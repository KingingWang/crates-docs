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
        description = "Output format: markdown (default), text (plain text), json (structured JSON: name, version, downloads, recent_downloads, description, repository, documentation, docs_rs)",
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
    /// Highest non-yanked version (crates.io). Preferred over `max_version`
    /// (which can be a yanked release users cannot install).
    #[serde(default)]
    max_stable_version: Option<String>,
    #[serde(default)]
    downloads: u64,
    /// Downloads in the last 90 days (crates.io `recent_downloads`). Drives the
    /// `recent-downloads` sort, so it is surfaced alongside the total.
    #[serde(default)]
    recent_downloads: Option<u64>,
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
            return serde_json::from_str(&cached).map_err(|e| {
                CallToolError::from_message(format!("[search_crates] Cache parsing failed: {e}"))
            });
        }

        // Build URL using helper function
        let url = super::build_crates_io_search_url(query, Some(sort), Some(limit as usize));

        let response = self
            .service
            .client()
            .get(&url)
            .header("User-Agent", crate::user_agent())
            .send()
            .await
            .map_err(|e| {
                CallToolError::from_message(format!("[search_crates] HTTP request failed: {e}"))
            })?;

        if !response.status().is_success() {
            // Surface crates.io diagnostics (e.g. rate-limit explanations) from
            // the response body instead of returning a bare status code. HTML
            // error pages are suppressed to avoid dumping noise.
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let trimmed = body.trim();
            let detail = if trimmed.is_empty()
                || trimmed.starts_with('<')
                || trimmed.to_ascii_lowercase().contains("<html")
            {
                String::new()
            } else {
                let snippet: String = trimmed.chars().take(200).collect();
                format!(" - {snippet}")
            };
            return Err(CallToolError::from_message(format!(
                "[search_crates] crates.io search failed: HTTP {status}{detail}"
            )));
        }

        // Use typed deserialization instead of serde_json::Value
        let search_response: SearchCratesResponse = response.json().await.map_err(|e| {
            CallToolError::from_message(format!("[search_crates] JSON parsing failed: {e}"))
        })?;

        let crates = parse_crates_response(search_response, limit as usize);

        let cache_value = serde_json::to_string(&crates).map_err(|e| {
            CallToolError::from_message(format!("[search_crates] Serialization failed: {e}"))
        })?;

        // Cache the results. A cache write failure (e.g. a Redis outage) must
        // not fail the user's request: the search succeeded, so log and
        // continue returning the results uncached.
        if let Err(e) = self
            .service
            .doc_cache()
            .set_search_results(query, limit, Some(sort), cache_value)
            .await
        {
            tracing::warn!(
                "[search_crates] failed to cache search results (continuing uncached): {e}"
            );
        }

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
    /// Recent downloads (last 90 days), when reported by crates.io. Shown next
    /// to the total so `recent-downloads`-sorted results are not confusing.
    #[serde(default)]
    recent_downloads: Option<u64>,
    /// Repository URL
    repository: Option<String>,
    /// Documentation URL (as provided by crates.io, if any)
    documentation: Option<String>,
    /// Canonical docs.rs URL for the crate (always present on fresh results).
    /// Tolerate cache entries written by older binaries that predate this
    /// field so a stale cache hit degrades to an empty value instead of a
    /// fatal "Cache parsing failed" error.
    #[serde(default)]
    docs_rs: String,
}

#[inline]
fn parse_crates_response(response: SearchCratesResponse, limit: usize) -> Vec<CrateInfo> {
    response
        .crates
        .into_iter()
        .take(limit)
        .map(|crate_record| {
            let docs_rs = format!("https://docs.rs/{}/", crate_record.name);
            CrateInfo {
                name: crate_record.name,
                description: crate_record.description,
                // Prefer the highest stable (non-yanked) version so results do
                // not advertise a version users cannot `cargo add`. Fall back to
                // max_version when a crate has no stable release.
                version: crate_record
                    .max_stable_version
                    .unwrap_or(crate_record.max_version),
                downloads: crate_record.downloads,
                recent_downloads: crate_record.recent_downloads,
                repository: crate_record.repository,
                documentation: crate_record.documentation,
                docs_rs,
            }
        })
        .collect()
}

#[inline]
fn format_search_results(crates: &[CrateInfo], format: super::Format) -> String {
    match format {
        // Machine-readable: an empty array is the correct, parseable result for
        // a no-match search, so it is left as-is.
        super::Format::Json => {
            serde_json::to_string_pretty(crates).unwrap_or_else(|_| "[]".to_string())
        }
        // Human-readable formats must not return a blank (text) or header-only
        // (markdown) body when nothing matched: that looks like a failure. Emit
        // an explicit "no crates found" message instead.
        super::Format::Text => {
            if crates.is_empty() {
                "No crates found matching the query.".to_string()
            } else {
                format_text_results(crates)
            }
        }
        // `html` is rejected before formatting (see `execute`); list both
        // variants explicitly so adding a new `Format` variant becomes a
        // compile error here rather than a silent fall-through to markdown.
        super::Format::Markdown | super::Format::Html => {
            if crates.is_empty() {
                "# Search Results\n\nNo crates found matching the query.".to_string()
            } else {
                format_markdown_results(crates)
            }
        }
    }
}

/// Escape characters that would let upstream-controlled text (e.g. a crate
/// description set by its publisher) inject markdown links, inline HTML, or
/// code spans into the rendered output. Only structural characters are escaped
/// so ordinary prose renders unchanged.
fn escape_markdown_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '[' => out.push_str("\\["),
            ']' => out.push_str("\\]"),
            '`' => out.push_str("\\`"),
            '<' => out.push_str("&lt;"),
            _ => out.push(c),
        }
    }
    out
}

/// Render a publisher-supplied URL as a markdown link only when it is a plain
/// `http(s)` URL free of characters that would break the link target or smuggle
/// in extra markdown. Anything else is shown as inert text so a crafted
/// `repository`/`documentation` field cannot inject an active or misleading
/// link (including non-`http` schemes such as `javascript:`).
fn render_markdown_url(label: &str, url: &str) -> String {
    let is_http = url.starts_with("http://") || url.starts_with("https://");
    let is_clean = !url.chars().any(|c| {
        c.is_whitespace()
            || c.is_control()
            || matches!(c, '(' | ')' | '<' | '>' | '[' | ']' | '"' | '\\')
    });
    if is_http && is_clean {
        format!("[{label}]({url})")
    } else {
        // Not a safe http(s) URL: show inert in a code span so it is neither a
        // clickable link nor able to inject further markdown.
        let inert: String = url
            .chars()
            .map(|c| if c == '`' || c.is_control() { ' ' } else { c })
            .collect();
        format!("`{inert}`")
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
        if let Some(recent) = crate_info.recent_downloads {
            writeln!(output, "**Recent downloads**: {recent}").unwrap();
        }

        if let Some(desc) = &crate_info.description {
            writeln!(output, "**Description**: {}", escape_markdown_text(desc)).unwrap();
        }

        if let Some(repo) = &crate_info.repository {
            writeln!(
                output,
                "**Repository**: {}",
                render_markdown_url("Link", repo)
            )
            .unwrap();
        }

        if let Some(docs) = &crate_info.documentation {
            writeln!(
                output,
                "**Documentation**: {}",
                render_markdown_url("Link", docs)
            )
            .unwrap();
        }

        writeln!(
            output,
            "**Docs.rs**: {}\n",
            render_markdown_url(&crate_info.docs_rs, &crate_info.docs_rs)
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
        if let Some(recent) = crate_info.recent_downloads {
            writeln!(output, "   Recent downloads: {recent}").unwrap();
        }

        if let Some(desc) = &crate_info.description {
            writeln!(output, "   Description: {desc}").unwrap();
        }

        // Mirror the markdown format so the text format does not silently drop
        // the repository/documentation links when crates.io provides them.
        if let Some(repo) = &crate_info.repository {
            writeln!(output, "   Repository: {repo}").unwrap();
        }

        if let Some(docs) = &crate_info.documentation {
            writeln!(output, "   Documentation: {docs}").unwrap();
        }

        writeln!(output, "   Docs.rs: {}", crate_info.docs_rs).unwrap();
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
        super::validate_search_query("search_crates", &params.query)?;
        // Clamp to the documented range [1, 100]. A lower bound of 0 (or a
        // value above 100) would otherwise silently produce an empty/odd
        // result set and a `per_page=0` upstream request.
        let limit = params.limit.unwrap_or(DEFAULT_SEARCH_LIMIT).clamp(1, 100);
        let sort = normalize_search_sort(params.sort.as_deref())?;
        // `parse_format` validates against SEARCH_FORMATS, so an unsupported
        // (e.g. `html`) or unknown format is rejected here with an error that
        // lists only the formats search actually accepts.
        let format = super::parse_format(
            "search_crates",
            params.format.as_deref(),
            super::SEARCH_FORMATS,
        )?;

        // Trim the query before fetching so the upstream crates.io request
        // matches the normalized (trimmed + lowercased) cache key. Otherwise a
        // query like "  tokio  " is sent verbatim to crates.io (poorer results)
        // yet cached/looked-up under the trimmed key, letting a whitespace-laden
        // first request poison the cache for every later "tokio" caller.
        let crates = self
            .search_crates(params.query.trim(), limit, &sort)
            .await?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_search_results_empty_emits_message() {
        use crate::tools::docs::Format;
        let text = format_search_results(&[], Format::Text);
        assert!(
            text.contains("No crates found"),
            "text empty should explain no matches: {text:?}"
        );
        let md = format_search_results(&[], Format::Markdown);
        assert!(
            md.contains("No crates found"),
            "markdown empty should explain no matches: {md:?}"
        );
        // JSON stays machine-parseable: an empty array, not a prose message.
        let json = format_search_results(&[], Format::Json);
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_recent_downloads_parsed_and_rendered() {
        use crate::tools::docs::Format;
        let json = r#"{"crates":[
            {"name":"a","max_stable_version":"1.0.0","downloads":1000,"recent_downloads":42}
        ]}"#;
        let resp: SearchCratesResponse = serde_json::from_str(json).unwrap();
        let crates = parse_crates_response(resp, 10);
        assert_eq!(crates[0].recent_downloads, Some(42));
        let md = format_search_results(&crates, Format::Markdown);
        assert!(md.contains("**Recent downloads**: 42"), "markdown: {md}");
        let text = format_search_results(&crates, Format::Text);
        assert!(text.contains("Recent downloads: 42"), "text: {text}");
    }

    #[test]
    fn test_parse_crates_response_prefers_stable_version() {
        // crates.io returns both max_version (may be yanked) and
        // max_stable_version; the stable one must win so results do not
        // advertise an uninstallable version.
        let json = r#"{"crates":[
            {"name":"a","max_version":"2.0.0-yanked","max_stable_version":"1.9.0","downloads":1},
            {"name":"b","max_version":"0.3.0","downloads":2}
        ]}"#;
        let resp: SearchCratesResponse = serde_json::from_str(json).unwrap();
        let crates = parse_crates_response(resp, 10);
        assert_eq!(crates[0].version, "1.9.0");
        // No max_stable_version -> fall back to max_version.
        assert_eq!(crates[1].version, "0.3.0");
    }

    #[test]
    fn test_format_text_results_includes_repository_and_documentation() {
        let crates = vec![CrateInfo {
            name: "demo".to_string(),
            description: Some("A demo crate".to_string()),
            version: "1.0.0".to_string(),
            downloads: 42,
            recent_downloads: None,
            repository: Some("https://github.com/x/demo".to_string()),
            documentation: Some("https://docs.rs/demo".to_string()),
            docs_rs: "https://docs.rs/demo/".to_string(),
        }];
        let out = format_text_results(&crates);
        assert!(
            out.contains("Repository: https://github.com/x/demo"),
            "{out}"
        );
        assert!(out.contains("Documentation: https://docs.rs/demo"), "{out}");
        assert!(out.contains("Docs.rs: https://docs.rs/demo/"), "{out}");
    }
}
