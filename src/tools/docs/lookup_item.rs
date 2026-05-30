//! Lookup item documentation tool
//!
//! Provides functionality to retrieve documentation for a specific item
//! (function, struct, trait, module, etc.) from a Rust crate on docs.rs.
//! Supports search paths like `serde::Serialize`, `std::collections::HashMap`, etc.

#![allow(missing_docs)]

use crate::tools::docs::html;
use crate::tools::docs::DocService;
use crate::tools::Tool;
use async_trait::async_trait;
use rust_mcp_sdk::schema::CallToolError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const TOOL_NAME: &str = "lookup_item";

/// Lookup item documentation tool parameters
///
/// Used to specify which crate item to look up and in what format to return the documentation.
#[rust_mcp_sdk::macros::mcp_tool(
    name = "lookup_item",
    title = "Lookup Item Documentation",
    description = "Get documentation for a specific item (function, struct, trait, module, etc.) from a Rust crate on docs.rs. Supports search paths like serde::Serialize, std::collections::HashMap, etc.",
    destructive_hint = false,
    idempotent_hint = true,
    open_world_hint = false,
    read_only_hint = true,
    execution(task_support = "optional"),
    icons = [
        (src = "https://docs.rs/favicon.ico", mime_type = "image/x-icon", sizes = ["32x32"], theme = "light"),
        (src = "https://docs.rs/favicon.ico", mime_type = "image/x-icon", sizes = ["32x32"], theme = "dark")
    ]
)]
/// Parameters for the `lookup_item` tool
///
/// Defines the input parameters for retrieving item documentation within a crate,
/// including the crate name, item path, optional version, and output format.
#[derive(Debug, Clone, Deserialize, Serialize, rust_mcp_sdk::macros::JsonSchema)]
pub struct LookupItemTool {
    /// Crate name containing the item (e.g., "serde", "tokio", "std")
    #[json_schema(
        title = "Crate Name",
        description = "Crate name to lookup, e.g.: serde, tokio, std"
    )]
    pub crate_name: String,

    /// Item path within the crate (e.g., `"std::collections::HashMap"`)
    #[json_schema(
        title = "Item Path",
        description = "Item path in format 'module::submodule::item', e.g.: serde::Serialize, tokio::runtime::Runtime, std::collections::HashMap"
    )]
    pub item_path: String,

    /// Crate version (optional, defaults to latest)
    #[json_schema(
        title = "Version",
        description = "Crate version. Uses latest version if not specified"
    )]
    pub version: Option<String>,

    /// Output format: "markdown", "text", or "html" (defaults to "markdown")
    #[json_schema(
        title = "Output Format",
        description = "Output format: markdown (default), text (plain text), html",
        default = "markdown"
    )]
    pub format: Option<String>,
}

/// Implementation of the lookup item documentation tool
///
/// Handles the execution of item documentation lookups within crates,
/// including cache management, HTTP fetching from docs.rs, and result formatting.
pub struct LookupItemToolImpl {
    /// Shared document service for HTTP requests and caching
    service: Arc<DocService>,
}

/// Memoized result of fetching a crate's `all.html` index.
///
/// Distinguishes "not fetched yet" from "fetched, but the index does not
/// exist" so the underlying network request is issued at most once per item
/// resolution, even when both the full-path and parent-path attempts fall back
/// to the index.
enum AllHtmlMemo {
    Unfetched,
    Fetched(Option<String>),
}

impl LookupItemToolImpl {
    /// Create a new lookup item tool instance
    #[must_use]
    pub fn new(service: Arc<DocService>) -> Self {
        Self { service }
    }

    /// Build docs.rs search URL for item
    fn build_search_url(crate_name: &str, item_path: &str, version: Option<&str>) -> String {
        super::build_docs_item_url(crate_name, version, item_path)
    }

    async fn fetch_item_html(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        if let Some(cached) = self
            .service
            .doc_cache()
            .get_item_html(crate_name, item_path, version)
            .await
        {
            return Ok(cached.to_string());
        }

        let html = self
            .resolve_item_html(crate_name, item_path, version)
            .await?;

        self.service
            .doc_cache()
            .set_item_html(crate_name, item_path, version, html.clone())
            .await
            .map_err(|e| {
                CallToolError::from_message(format!("[{TOOL_NAME}] Cache set failed: {e}"))
            })?;

        Ok(html)
    }

    /// Resolve and fetch the HTML for a specific item.
    ///
    /// Probes the candidate rustdoc item URLs (`struct.`, `trait.`, `fn.`, ...)
    /// and returns the first that exists. docs.rs renders in-page search with
    /// client-side JavaScript, so the `?search=` URL only ever returns the
    /// crate landing page server-side; therefore, if no direct item page is
    /// found, it falls back to that crate page so the caller still gets useful
    /// context instead of a hard error.
    async fn resolve_item_html(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        // Reuse a single `all.html` fetch across the full-path and parent-path
        // resolution attempts. Both attempts consult the same crate-level
        // `all.html` index, so memoizing it here avoids a duplicate network
        // round trip when neither path resolves via a direct item page.
        let mut all_html_memo = AllHtmlMemo::Unfetched;
        if let Some(html) = self
            .try_resolve_item_path(crate_name, item_path, version, &mut all_html_memo)
            .await?
        {
            return Ok(html);
        }

        // Method / associated-item fallback: `Type::member` and trait methods
        // have no standalone rustdoc page; the member is documented on the
        // containing type's page. If the full path did not resolve, retry with
        // the parent path so the caller gets the focused type page (e.g. the
        // `Vec` struct for `Vec::push`) instead of the entire crate overview.
        if let Some((parent, _member)) = item_path.rsplit_once("::") {
            let parent = parent.trim();
            if !parent.is_empty() {
                if let Some(html) = self
                    .try_resolve_item_path(crate_name, parent, version, &mut all_html_memo)
                    .await?
                {
                    return Ok(html);
                }
            }
        }

        // Fallback: the crate page (legacy `?search=` behaviour).
        let url = Self::build_search_url(crate_name, item_path, version);
        self.service.fetch_html(&url, Some(TOOL_NAME)).await
    }

    /// Probe the candidate rustdoc item pages and the crate `all.html`
    /// re-export index for an exact item path. Returns the page HTML if found,
    /// or `None` if neither path resolves.
    async fn try_resolve_item_path(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
        all_html_memo: &mut AllHtmlMemo,
    ) -> std::result::Result<Option<String>, CallToolError> {
        let candidates = super::build_docs_item_url_candidates(crate_name, version, item_path);
        for url in candidates {
            if let Some(html) = self
                .service
                .fetch_html_optional(&url, Some(TOOL_NAME))
                .await?
            {
                return Ok(Some(html));
            }
        }

        // Re-export fallback: consult the crate's `all.html` index to resolve
        // items that have no stub page at the path implied by their name
        // (e.g. `tokio::spawn`, actually defined at `tokio::task::spawn`).
        let item_name = item_path.rsplit("::").next().unwrap_or(item_path).trim();
        if !item_name.is_empty() {
            // Lazily fetch `all.html` exactly once, memoizing the result (which
            // may itself be `None` when the index does not exist) so a second
            // resolution attempt for the parent path reuses it instead of
            // issuing a duplicate request.
            if matches!(all_html_memo, AllHtmlMemo::Unfetched) {
                let all_url = super::build_docs_all_items_url(crate_name, version);
                // Bind the fallible await to a `let` so the `?` temporary is
                // dropped at the statement boundary and not held across a later
                // await (which would make the future non-`Send`).
                let fetched = self
                    .service
                    .fetch_html_optional(&all_url, Some(TOOL_NAME))
                    .await?;
                *all_html_memo = AllHtmlMemo::Fetched(fetched);
            }
            // Compute the resolved URL in a scope that ends before the next
            // await so the borrow of `all_html_memo` is not held across it.
            let item_url = {
                let all_html = match &*all_html_memo {
                    AllHtmlMemo::Fetched(html) => html.as_deref(),
                    AllHtmlMemo::Unfetched => None,
                };
                all_html.and_then(|html| {
                    super::find_item_url_in_all_html(crate_name, version, html, item_name)
                })
            };
            if let Some(item_url) = item_url {
                let resolved = self
                    .service
                    .fetch_html_optional(&item_url, Some(TOOL_NAME))
                    .await?;
                if let Some(html) = resolved {
                    return Ok(Some(html));
                }
            }
        }

        Ok(None)
    }

    /// Get item documentation (markdown format)
    ///
    /// Returns `Arc<str>` to preserve shared ownership on cache hits,
    /// avoiding unnecessary cloning of large documentation strings.
    async fn fetch_item_docs(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
    ) -> std::result::Result<Arc<str>, CallToolError> {
        // Try cache first - returns Arc<str> directly without cloning
        if let Some(cached) = self
            .service
            .doc_cache()
            .get_item_docs(crate_name, item_path, version)
            .await
        {
            return Ok(cached);
        }

        let html = self.fetch_item_html(crate_name, item_path, version).await?;

        // Extract search results into Arc<str> for shared ownership
        let docs: Arc<str> =
            Arc::from(html::extract_search_results(&html, item_path).into_boxed_str());

        // Cache result - convert Arc<str> to String for the cache
        self.service
            .doc_cache()
            .set_item_docs(crate_name, item_path, version, docs.to_string())
            .await
            .map_err(|e| {
                CallToolError::from_message(format!("[{TOOL_NAME}] Cache set failed: {e}"))
            })?;

        Ok(docs)
    }

    /// Get item documentation as plain text
    async fn fetch_item_docs_as_text(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        let html = self.fetch_item_html(crate_name, item_path, version).await?;
        let body = html::extract_documentation_as_text(&html);
        // Mirror the markdown fallback note. `is_item_fallback_page` inspects
        // the page `<h1>` so it catches both the containing-type fallback
        // (e.g. the `Value` enum page for `Value::is_null`) and the crate
        // overview fallback, and stays correct on cache replays.
        let note = if html::is_item_fallback_page(&html, item_path) {
            format!(
                "No dedicated documentation page was found for '{item_path}'; showing the closest available page (its containing type or the crate overview) instead. It may be a method, associated item, or trait method, or it may not exist.\n\n"
            )
        } else {
            String::new()
        };
        Ok(format!("Documentation: {item_path}\n\n{note}{body}"))
    }

    /// Get item documentation as raw HTML
    async fn fetch_item_docs_as_html(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        let html = self.fetch_item_html(crate_name, item_path, version).await?;
        let body = html::extract_documentation_html(&html);
        // Mirror the markdown/text fallback note so all three formats are
        // consistent. `is_item_fallback_page` inspects the page `<h1>` to catch
        // both the containing-type fallback and the crate overview fallback,
        // and stays correct on cache replays.
        if html::is_item_fallback_page(&html, item_path) {
            // item_path is validated to [A-Za-z0-9_:-]; escape defensively
            // anyway since this is an HTML context.
            let safe_path = item_path
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            return Ok(format!(
                "<p><em>No dedicated documentation page was found for '{safe_path}'; showing the closest available page (its containing type or the crate overview) instead. It may be a method, associated item, or trait method, or it may not exist.</em></p>\n{body}"
            ));
        }
        Ok(body)
    }
}

#[async_trait]
impl Tool for LookupItemToolImpl {
    fn definition(&self) -> rust_mcp_sdk::schema::Tool {
        LookupItemTool::tool()
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> std::result::Result<
        rust_mcp_sdk::schema::CallToolResult,
        rust_mcp_sdk::schema::CallToolError,
    > {
        let mut params: LookupItemTool = serde_json::from_value(arguments).map_err(|e| {
            rust_mcp_sdk::schema::CallToolError::invalid_arguments(
                "lookup_item",
                Some(format!("Parameter parsing failed: {e}")),
            )
        })?;

        super::validate_crate_name(TOOL_NAME, &params.crate_name)?;
        super::validate_version(TOOL_NAME, params.version.as_deref())?;
        super::validate_item_path(TOOL_NAME, &params.item_path)?;
        // Normalise surrounding whitespace so it does not leak into headings or
        // candidate URL construction.
        params.crate_name = params.crate_name.trim().to_string();
        if let Some(version) = params.version.as_mut() {
            *version = version.trim().to_string();
        }
        params.item_path = params.item_path.trim().to_string();

        // Propagate the detailed parse error (e.g. "Invalid format 'xml'. Expected
        // one of: ...") rather than masking it with a generic message, so callers
        // get actionable feedback.
        let format = super::parse_format(TOOL_NAME, params.format.as_deref())?;
        let content = match format {
            super::Format::Text => {
                self.fetch_item_docs_as_text(
                    &params.crate_name,
                    &params.item_path,
                    params.version.as_deref(),
                )
                .await?
            }
            super::Format::Html => {
                self.fetch_item_docs_as_html(
                    &params.crate_name,
                    &params.item_path,
                    params.version.as_deref(),
                )
                .await?
            }
            super::Format::Json => {
                return Err(rust_mcp_sdk::schema::CallToolError::invalid_arguments(
                    "lookup_item",
                    Some(
                        "Invalid format 'json'. This tool supports: markdown, text, html"
                            .to_string(),
                    ),
                ))
            }
            super::Format::Markdown => self
                .fetch_item_docs(
                    &params.crate_name,
                    &params.item_path,
                    params.version.as_deref(),
                )
                .await
                .map(|arc| arc.to_string())?,
        };

        Ok(rust_mcp_sdk::schema::CallToolResult::text_content(vec![
            content.into(),
        ]))
    }
}

impl Default for LookupItemToolImpl {
    fn default() -> Self {
        Self::new(Arc::new(super::DocService::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_build_search_url_without_version() {
        std::env::set_var("CRATES_DOCS_DOCS_RS_URL", "https://docs.rs");
        let url = LookupItemToolImpl::build_search_url("serde", "Serialize", None);
        assert_eq!(url, "https://docs.rs/serde/?search=Serialize");
        std::env::remove_var("CRATES_DOCS_DOCS_RS_URL");
    }

    #[test]
    #[serial]
    fn test_build_search_url_with_version() {
        std::env::set_var("CRATES_DOCS_DOCS_RS_URL", "https://docs.rs");
        let url = LookupItemToolImpl::build_search_url("serde", "Serialize", Some("1.0.0"));
        assert_eq!(url, "https://docs.rs/serde/1.0.0/?search=Serialize");
        std::env::remove_var("CRATES_DOCS_DOCS_RS_URL");
    }

    #[test]
    #[serial]
    fn test_build_search_url_encodes_special_chars() {
        std::env::set_var("CRATES_DOCS_DOCS_RS_URL", "https://docs.rs");
        let url = LookupItemToolImpl::build_search_url("std", "collections::HashMap", None);
        assert!(url.contains("collections%3A%3AHashMap"));
        std::env::remove_var("CRATES_DOCS_DOCS_RS_URL");
    }
}
