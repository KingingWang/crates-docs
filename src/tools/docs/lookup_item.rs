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
            return Ok(cached.as_ref().clone());
        }

        let url = Self::build_search_url(crate_name, item_path, version);
        let html = self.service.fetch_html(&url, Some(TOOL_NAME)).await?;

        self.service
            .doc_cache()
            .set_item_html(crate_name, item_path, version, html.clone())
            .await
            .map_err(|e| {
                CallToolError::from_message(format!("[{TOOL_NAME}] Cache set failed: {e}"))
            })?;

        Ok(html)
    }

    /// Get item documentation (markdown format)
    ///
    /// Returns `Arc<String>` to preserve shared ownership on cache hits,
    /// avoiding unnecessary cloning of large documentation strings.
    async fn fetch_item_docs(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
    ) -> std::result::Result<Arc<String>, CallToolError> {
        // Try cache first - returns Arc<String> directly without cloning
        if let Some(cached) = self
            .service
            .doc_cache()
            .get_item_docs(crate_name, item_path, version)
            .await
        {
            return Ok(cached);
        }

        let html = self.fetch_item_html(crate_name, item_path, version).await?;

        // Extract search results into Arc<String> for shared ownership
        let docs: Arc<String> = Arc::new(html::extract_search_results(&html, item_path));

        // Cache result - clone the Arc's inner String for the cache
        self.service
            .doc_cache()
            .set_item_docs(crate_name, item_path, version, (*docs).clone())
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
        Ok(format!(
            "Search results: {}\n\n{}",
            item_path,
            html::html_to_text(&html)
        ))
    }

    /// Get item documentation as raw HTML
    async fn fetch_item_docs_as_html(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        self.fetch_item_html(crate_name, item_path, version).await
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
        let params: LookupItemTool = serde_json::from_value(arguments).map_err(|e| {
            rust_mcp_sdk::schema::CallToolError::invalid_arguments(
                "lookup_item",
                Some(format!("Parameter parsing failed: {e}")),
            )
        })?;

        let format = super::parse_format(params.format.as_deref()).map_err(|_| {
            rust_mcp_sdk::schema::CallToolError::invalid_arguments(
                "lookup_item",
                Some("Invalid format".to_string()),
            )
        })?;
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
                    Some("JSON format is not supported by this tool".to_string()),
                ))
            }
            super::Format::Markdown => self
                .fetch_item_docs(
                    &params.crate_name,
                    &params.item_path,
                    params.version.as_deref(),
                )
                .await
                .map(|arc| (*arc).clone())?,
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
