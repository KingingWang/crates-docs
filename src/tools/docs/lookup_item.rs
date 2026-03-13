//! Lookup item documentation tool
#![allow(missing_docs)]

use crate::tools::docs::html;
use crate::tools::docs::DocService;
use crate::tools::Tool;
use async_trait::async_trait;
use rust_mcp_sdk::schema::CallToolError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Lookup item documentation tool
#[rust_mcp_sdk::macros::mcp_tool(
    name = "lookup_item",
    title = "查找 Crate 项目文档",
    description = "从 docs.rs 获取 Rust crate 中特定项目（函数、结构体、trait、模块等）的文档。适用于查找特定 API 的详细用法和签名。支持搜索路径如 serde::Serialize、std::collections::HashMap 等。",
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
        title = "Crate 名称",
        description = "要查找的 Crate name，例如：serde、tokio、std"
    )]
    pub crate_name: String,

    /// Item path (e.g., `std::collections::HashMap`)
    #[json_schema(
        title = "项目路径",
        description = "要查找的项目路径，格式为 '模块::子模块::项目名'。例如：serde::Serialize、tokio::runtime::Runtime、std::collections::HashMap"
    )]
    pub item_path: String,

    /// Version (optional, defaults to latest)
    #[json_schema(
        title = "版本号",
        description = "指定 crate 版本号。不指定则使用最新版本"
    )]
    pub version: Option<String>,

    /// Output format: markdown, text, or html
    #[json_schema(
        title = "输出格式",
        description = "Documentation output format: markdown (default), text (plain text), html",
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
        let encoded_path = urlencoding::encode(item_path);
        match version {
            Some(ver) => format!("https://docs.rs/{crate_name}/{ver}/?search={encoded_path}"),
            None => format!("https://docs.rs/{crate_name}/?search={encoded_path}"),
        }
    }

    /// Fetch HTML from docs.rs
    async fn fetch_html(&self, url: &str) -> std::result::Result<String, CallToolError> {
        let response = self
            .service
            .client()
            .get(url)
            .send()
            .await
            .map_err(|e| CallToolError::from_message(format!("HTTP request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(CallToolError::from_message(format!(
                "Failed to get item documentation: HTTP {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        response
            .text()
            .await
            .map_err(|e| CallToolError::from_message(format!("Failed to read response: {e}")))
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
        let html = self.fetch_html(&url).await?;

        // Extract search results
        let docs = html::extract_search_results(&html, item_path);

        // Cache result
        self.service
            .doc_cache()
            .set_item_docs(crate_name, item_path, version, docs.clone())
            .await
            .map_err(|e| CallToolError::from_message(format!("Cache set failed: {e}")))?;

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
        let html = self.fetch_html(&url).await?;
        Ok(format!(
            "搜索结果: {}\n\n{}",
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
        self.fetch_html(&url).await
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

    #[test]
    fn test_build_search_url_without_version() {
        let url = LookupItemToolImpl::build_search_url("serde", "Serialize", None);
        assert_eq!(url, "https://docs.rs/serde/?search=Serialize");
    }

    #[test]
    fn test_build_search_url_with_version() {
        let url = LookupItemToolImpl::build_search_url("serde", "Serialize", Some("1.0.0"));
        assert_eq!(url, "https://docs.rs/serde/1.0.0/?search=Serialize");
    }

    #[test]
    fn test_build_search_url_encodes_special_chars() {
        let url = LookupItemToolImpl::build_search_url("std", "collections::HashMap", None);
        assert!(url.contains("collections%3A%3AHashMap"));
    }
}
