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
const DEFAULT_SEARCH_CACHE_TTL_SECS: u64 = 300;
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
        Some(sort) if VALID_SEARCH_SORTS.contains(&sort) => Ok(sort.to_string()),
        Some(sort) => Err(CallToolError::invalid_arguments(
            "search_crates",
            Some(format!(
                "Invalid sort option '{sort}', expected one of: {}",
                VALID_SEARCH_SORTS.join(", ")
            )),
        )),
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
        let cache_key = format!("search:{query}:{sort}:{limit}");

        if let Some(cached) = self.service.cache().get(&cache_key).await {
            return serde_json::from_str(&cached)
                .map_err(|e| CallToolError::from_message(format!("Cache parsing failed: {e}")));
        }

        let url = format!(
            "{}/api/v1/crates?q={}&per_page={}&sort={}",
            super::crates_io_base_url(),
            urlencoding::encode(query),
            limit,
            urlencoding::encode(sort)
        );

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

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| CallToolError::from_message(format!("JSON parsing failed: {e}")))?;

        let crates = parse_crates_response(&json, limit as usize);

        let cache_value = serde_json::to_string(&crates)
            .map_err(|e| CallToolError::from_message(format!("Serialization failed: {e}")))?;

        self.service
            .cache()
            .set(
                cache_key,
                cache_value,
                Some(std::time::Duration::from_secs(
                    DEFAULT_SEARCH_CACHE_TTL_SECS,
                )),
            )
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
fn parse_crates_response(json: &serde_json::Value, limit: usize) -> Vec<CrateInfo> {
    let Some(crates_array) = json.get("crates").and_then(|c| c.as_array()) else {
        return Vec::new();
    };

    crates_array
        .iter()
        .take(limit)
        .map(|crate_item| CrateInfo {
            name: crate_item
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("Unknown")
                .to_string(),
            description: crate_item
                .get("description")
                .and_then(|d| d.as_str())
                .map(std::string::ToString::to_string),
            version: crate_item
                .get("max_version")
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0")
                .to_string(),
            downloads: crate_item
                .get("downloads")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            repository: crate_item
                .get("repository")
                .and_then(|r| r.as_str())
                .map(std::string::ToString::to_string),
            documentation: crate_item
                .get("documentation")
                .and_then(|d| d.as_str())
                .map(std::string::ToString::to_string),
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

        let limit = params.limit.unwrap_or(DEFAULT_SEARCH_LIMIT).min(100);
        let sort = normalize_search_sort(params.sort.as_deref())?;
        let crates = self.search_crates(&params.query, limit, &sort).await?;

        let format = super::parse_format(params.format.as_deref()).map_err(|_| {
            rust_mcp_sdk::schema::CallToolError::invalid_arguments(
                "search_crates",
                Some("Invalid format".to_string()),
            )
        })?;
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
