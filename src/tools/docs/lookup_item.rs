//! Lookup item documentation tool
#![allow(missing_docs)]

use crate::tools::docs::html;
use crate::tools::docs::DocService;
use crate::tools::Tool;
use async_trait::async_trait;
use rust_mcp_sdk::schema::CallToolError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const TOOL_NAME: &str = "lookup_item";

/// Lookup item documentation tool
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
#[allow(missing_docs)]
#[derive(Debug, Clone, Deserialize, Serialize, rust_mcp_sdk::macros::JsonSchema)]
pub struct LookupItemTool {
    /// Crate name
    #[json_schema(
        title = "Crate Name",
        description = "Crate name to lookup, e.g.: serde, tokio, std"
    )]
    pub crate_name: String,

    /// Item path (e.g., `std::collections::HashMap`)
    #[json_schema(
        title = "Item Path",
        description = "Item path in format 'module::submodule::item', e.g.: serde::Serialize, tokio::runtime::Runtime, std::collections::HashMap"
    )]
    pub item_path: String,

    /// Version (optional, defaults to latest)
    #[json_schema(
        title = "Version",
        description = "Crate version. Uses latest version if not specified"
    )]
    pub version: Option<String>,

    /// Output format: markdown, text, or html
    #[json_schema(
        title = "Output Format",
        description = "Output format: markdown (default), text (plain text), html",
        default = "markdown"
    )]
    pub format: Option<String>,
}

/// Lookup item documentation tool implementation
pub struct LookupItemToolImpl {
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

    /// Get item documentation (markdown format)
    async fn fetch_item_docs(
        &self,
        crate_name: &str,
        item_path: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        // Try cache first
        if let Some(cached) = self
            .service
            .doc_cache()
            .get_item_docs(crate_name, item_path, version)
            .await
        {
            return Ok(cached);
        }

        // Fetch from docs.rs
        let url = Self::build_search_url(crate_name, item_path, version);
        let html = self.service.fetch_html(&url, Some(TOOL_NAME)).await?;

        // Extract search results
        let docs = html::extract_search_results(&html, item_path);

        // Cache result
        self.service
            .doc_cache()
            .set_item_docs(crate_name, item_path, version, docs.clone())
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
        let url = Self::build_search_url(crate_name, item_path, version);
        let html = self.service.fetch_html(&url, Some(TOOL_NAME)).await?;
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
        let url = Self::build_search_url(crate_name, item_path, version);
        self.service.fetch_html(&url, Some(TOOL_NAME)).await
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

        let format = params.format.as_deref().unwrap_or("markdown");
        let content = match format {
            "text" => {
                self.fetch_item_docs_as_text(
                    &params.crate_name,
                    &params.item_path,
                    params.version.as_deref(),
                )
                .await?
            }
            "html" => {
                self.fetch_item_docs_as_html(
                    &params.crate_name,
                    &params.item_path,
                    params.version.as_deref(),
                )
                .await?
            }
            _ => {
                // "markdown" and other formats default to markdown
                self.fetch_item_docs(
                    &params.crate_name,
                    &params.item_path,
                    params.version.as_deref(),
                )
                .await?
            }
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
