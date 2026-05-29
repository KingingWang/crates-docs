//! Search crates tool
//!
//! Provides functionality to search for Rust crates from crates.io.
//! Returns a list of matching crates with metadata like name, description,
//! version, downloads, etc.

#![allow(missing_docs)]

use crate::tools::Tool;
use async_trait::async_trait;
use rust_mcp_sdk::macros;
use rust_mcp_sdk::schema::CallToolError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const DEFAULT_SEARCH_LIMIT: u32 = 10;
const ESTIMATED_MARKDOWN_ENTRY_SIZE: usize = 200;
const ESTIMATED_TEXT_ENTRY_SIZE: usize = 100;

/// Search crates tool parameters
///
/// Used to specify search criteria for finding Rust crates on crates.io.
#[macros::mcp_tool(
    name = "search_crates",
    title = "Search Crates",
    description = "Search for Rust crates from crates.io. Returns a list of matching crates, including name, description, version, downloads, etc. Suitable for discovering and comparing available Rust libraries.",
    destructive_hint = false,
    idempotent_hint = true,
    open_world_hint = false,
    read_only_hint = true,
    execution(task_support = "optional"),
    icons = [
        (src = "https://crates.io/favicon.ico", mime_type = "image/x-icon", sizes = ["32x32"], theme = "light"),
        (src = "https://crates.io/favicon.ico", mime_type = "image/x-icon", sizes = ["32x32"], theme = "dark")
    ]
)]
/// Parameters for the `search_crates` tool
///
/// Defines the input parameters for searching Rust crates on crates.io,
/// including the search query, result limit, sort order, and output format.
#[derive(Debug, Clone, Deserialize, Serialize, macros::JsonSchema)]
pub struct SearchCratesTool {
    /// Search keywords (e.g., "web framework", "async", "http client")
    #[json_schema(
        title = "Search Query",
        description = "Search keywords, e.g.: web framework, async, http client, serialization"
    )]
    pub query: String,

    /// Maximum number of results to return (range 1-100, defaults to 10)
    #[json_schema(
        title = "Result Limit",
        description = "Maximum number of results to return, range 1-100",
        minimum = 1,
        maximum = 100,
        default = 10
    )]
    pub limit: Option<u32>,

    /// Sort order: "relevance", "downloads", "recent-downloads", "recent-updates", "new"
    #[json_schema(
        title = "Sort Order",
        description = "Sort order: relevance (default), downloads, recent-downloads, recent-updates, new",
        default = "relevance"
    )]
    pub sort: Option<String>,

    /// Output format: "markdown", "text", or "json" (defaults to "markdown")
    #[json_schema(
        title = "Output Format",
        description = "Output format: markdown (default), text (plain text), json (raw JSON)",
        default = "markdown"
    )]
    pub format: Option<String>,
}

const DEFAULT_SEARCH_SORT: &str = "relevance";
const VALID_SEARCH_SORTS: &[&str] = &[
    DEFAULT_SEARCH_SORT,
    "downloads",
    "recent-downloads",
    "recent-updates",
    "new",
];

/// Crates.io search response (typed deserialization)
#[derive(Debug, Deserialize)]
struct SearchCratesResponse {
    crates: Vec<SearchCrateRecord>,
}

/// Individual crate record from crates.io search
#[derive(Debug, Deserialize)]
struct SearchCrateRecord {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default = "default_max_version")]
    max_version: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    repository: Option<String>,
    #[serde(default)]
    documentation: Option<String>,
}

fn default_max_version() -> String {
    "0.0.0".to_string()
}

/// Implementation of the search crates tool
///
/// Handles the execution of crate searches on crates.io, including
/// cache management, HTTP requests, and result formatting.
pub struct SearchCratesToolImpl {
    /// Shared document service for HTTP requests and caching
    service: Arc<super::DocService>,
}

fn normalize_search_sort(sort: Option<&str>) -> std::result::Result<String, CallToolError> {
    match sort {
        Some(raw) => {
            // Normalize like `parse_format`: trim surrounding whitespace and
            // compare case-insensitively so e.g. "Downloads" or " downloads "
            // are accepted. This also matches the cache-key normalization.
            let normalized = raw.trim().to_lowercase();
            if VALID_SEARCH_SORTS.contains(&normalized.as_str()) {
                Ok(normalized)
            } else {
                Err(CallToolError::invalid_arguments(
                    "search_crates",
                    Some(format!(
                        "Invalid sort option '{raw}', expected one of: {}",
                        VALID_SEARCH_SORTS.join(", ")
                    )),
                ))
            }
        }
        None => Ok(DEFAULT_SEARCH_SORT.to_string()),
    }
}

impl SearchCratesToolImpl {
    /// Create a new tool instance
    #[must_use]
    pub fn new(service: Arc<super::DocService>) -> Self {
        Self { service }
    }

    /// Search crates
    async fn search_crates(
        &self,
        query: &str,
        limit: u32,
        sort: &str,
    ) -> std::result::Result<Vec<CrateInfo>, CallToolError> {
        // Check cache using DocCache API
        if let Some(cached) = self
            .service
            .doc_cache()
            .get_search_results(query, limit, Some(sort))
            .await
        {
            return serde_json::from_str(&cached)
                .map_err(|e| CallToolError::from_message(format!("Cache parsing failed: {e}")));
        }

        // Build URL using helper function
        let url = super::build_crates_io_search_url(query, Some(sort), Some(limit as usize));

        let response = self
            .service
            .client()
            .get(&url)
            .header("User-Agent", format!("CratesDocsMCP/{}", crate::VERSION))
            .send()
            .await
            .map_err(|e| CallToolError::from_message(format!("HTTP request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(CallToolError::from_message(format!(
                "Search failed, status code: {}",
                response.status()
            )));
        }

        // Use typed deserialization instead of serde_json::Value
        let search_response: SearchCratesResponse = response
            .json()
            .await
            .map_err(|e| CallToolError::from_message(format!("JSON parsing failed: {e}")))?;

        let crates = parse_crates_response(search_response, limit as usize);

        let cache_value = serde_json::to_string(&crates)
            .map_err(|e| CallToolError::from_message(format!("Serialization failed: {e}")))?;

        // Set cache using DocCache API
        self.service
            .doc_cache()
            .set_search_results(query, limit, Some(sort), cache_value)
            .await
            .map_err(|e| CallToolError::from_message(format!("Cache set failed: {e}")))?;

        Ok(crates)
    }
}

/// Crate information from search results
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CrateInfo {
    /// Crate name
    name: String,
    /// Crate description
    description: Option<String>,
    /// Latest version
    version: String,
    /// Total downloads
    downloads: u64,
    /// Repository URL
    repository: Option<String>,
    /// Documentation URL
    documentation: Option<String>,
}

#[inline]
fn parse_crates_response(response: SearchCratesResponse, limit: usize) -> Vec<CrateInfo> {
    response
        .crates
        .into_iter()
        .take(limit)
        .map(|crate_record| CrateInfo {
            name: crate_record.name,
            description: crate_record.description,
            version: crate_record.max_version,
            downloads: crate_record.downloads,
            repository: crate_record.repository,
            documentation: crate_record.documentation,
        })
        .collect()
}

#[inline]
fn format_search_results(crates: &[CrateInfo], format: super::Format) -> String {
    match format {
        super::Format::Json => {
            serde_json::to_string_pretty(crates).unwrap_or_else(|_| "[]".to_string())
        }
        super::Format::Text => format_text_results(crates),
        _ => format_markdown_results(crates),
    }
}

fn format_markdown_results(crates: &[CrateInfo]) -> String {
    // SAFETY: writeln! to String never fails (writes to memory buffer). unwrap() is safe here.
    use std::fmt::Write;
    let estimated_size = crates.len().saturating_mul(ESTIMATED_MARKDOWN_ENTRY_SIZE) + 20;
    let mut output = String::with_capacity(estimated_size);
    output.push_str("# Search Results\n\n");

    for (i, crate_info) in crates.iter().enumerate() {
        writeln!(output, "## {}. {}", i + 1, crate_info.name).unwrap();
        writeln!(output, "**Version**: {}", crate_info.version).unwrap();
        writeln!(output, "**Downloads**: {}", crate_info.downloads).unwrap();

        if let Some(desc) = &crate_info.description {
            writeln!(output, "**Description**: {desc}").unwrap();
        }

        if let Some(repo) = &crate_info.repository {
            writeln!(output, "**Repository**: [Link]({repo})").unwrap();
        }

        if let Some(docs) = &crate_info.documentation {
            writeln!(output, "**Documentation**: [Link]({docs})").unwrap();
        }

        writeln!(
            output,
            "**Docs.rs**: [https://docs.rs/{}/](https://docs.rs/{}/)\n",
            crate_info.name, crate_info.name
        )
        .unwrap();
    }

    output
}

fn format_text_results(crates: &[CrateInfo]) -> String {
    // SAFETY: writeln! to String never fails (writes to memory buffer). unwrap() is safe here.
    use std::fmt::Write;
    let estimated_size = crates.len().saturating_mul(ESTIMATED_TEXT_ENTRY_SIZE);
    let mut output = String::with_capacity(estimated_size);

    for (i, crate_info) in crates.iter().enumerate() {
        writeln!(output, "{}. {}", i + 1, crate_info.name).unwrap();
        writeln!(output, "   Version: {}", crate_info.version).unwrap();
        writeln!(output, "   Downloads: {}", crate_info.downloads).unwrap();

        if let Some(desc) = &crate_info.description {
            writeln!(output, "   Description: {desc}").unwrap();
        }

        writeln!(output, "   Docs.rs: https://docs.rs/{}/", crate_info.name).unwrap();
        writeln!(output).unwrap();
    }

    output
}

#[async_trait]
impl Tool for SearchCratesToolImpl {
    fn definition(&self) -> rust_mcp_sdk::schema::Tool {
        SearchCratesTool::tool()
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> std::result::Result<
        rust_mcp_sdk::schema::CallToolResult,
        rust_mcp_sdk::schema::CallToolError,
    > {
        let params: SearchCratesTool = serde_json::from_value(arguments).map_err(|e| {
            rust_mcp_sdk::schema::CallToolError::invalid_arguments(
                "search_crates",
                Some(format!("Parameter parsing failed: {e}")),
            )
        })?;

        // Validate all input parameters up front (fail-fast) before making any
        // network requests. This avoids wasted crates.io calls on invalid input
        // and keeps input-validation errors deterministic regardless of network
        // availability.
        super::validate_search_query(&params.query)?;
        // Clamp to the documented range [1, 100]. A lower bound of 0 (or a
        // value above 100) would otherwise silently produce an empty/odd
        // result set and a `per_page=0` upstream request.
        let limit = params.limit.unwrap_or(DEFAULT_SEARCH_LIMIT).clamp(1, 100);
        let sort = normalize_search_sort(params.sort.as_deref())?;
        let format = super::parse_format(params.format.as_deref())?;
        // search_crates only supports markdown/text/json. Reject `html`
        // explicitly with an actionable error instead of silently returning
        // markdown (the tool schema does not advertise html for search).
        if matches!(format, super::Format::Html) {
            return Err(rust_mcp_sdk::schema::CallToolError::invalid_arguments(
                "search_crates",
                Some(
                    "Invalid format 'html' for search_crates. Expected one of: markdown, text, json"
                        .to_string(),
                ),
            ));
        }

        let crates = self.search_crates(&params.query, limit, &sort).await?;
        let content = format_search_results(&crates, format);

        Ok(rust_mcp_sdk::schema::CallToolResult::text_content(vec![
            content.into(),
        ]))
    }
}

impl Default for SearchCratesToolImpl {
    fn default() -> Self {
        Self::new(Arc::new(super::DocService::default()))
    }
}
