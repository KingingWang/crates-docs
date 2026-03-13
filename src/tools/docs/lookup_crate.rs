//! Lookup crate documentation tool
#![allow(missing_docs)]

use crate::tools::docs::html;
use crate::tools::docs::DocService;
use crate::tools::Tool;
use async_trait::async_trait;
use rust_mcp_sdk::schema::CallToolError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Lookup crate documentation tool
#[rust_mcp_sdk::macros::mcp_tool(
    name = "lookup_crate",
    title = "Lookup Crate Documentation",
    description = "Get complete documentation for a Rust crate from docs.rs. Returns the main documentation page content, including modules, structs, functions, etc. Suitable for understanding the overall functionality and usage of a crate.",
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
pub struct LookupCrateTool {
    /// Crate name
    #[json_schema(
        title = "Crate 名称",
        description = "要查找的 Crate name，例如：serde、tokio、reqwest"
    )]
    pub crate_name: String,

    /// Version (optional, defaults to latest)
    #[json_schema(
        title = "版本号",
        description = "Specify crate version, e.g.: 1.0.0. Uses latest version if not specified"
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

/// Lookup crate documentation tool implementation
pub struct LookupCrateToolImpl {
    service: Arc<DocService>,
}

impl LookupCrateToolImpl {
    /// Create a new lookup tool instance
    #[must_use]
    pub fn new(service: Arc<DocService>) -> Self {
        Self { service }
    }

    /// Build docs.rs URL for crate
    fn build_url(crate_name: &str, version: Option<&str>) -> String {
        match version {
            Some(ver) => format!("https://docs.rs/{crate_name}/{ver}/"),
            None => format!("https://docs.rs/{crate_name}/"),
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
                "Failed to get documentation: HTTP {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        response
            .text()
            .await
            .map_err(|e| CallToolError::from_message(format!("Failed to read response: {e}")))
    }

    /// Get crate documentation (markdown format)
    async fn fetch_crate_docs(
        &self,
        crate_name: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        // Try cache first
        if let Some(cached) = self
            .service
            .doc_cache()
            .get_crate_docs(crate_name, version)
            .await
        {
            return Ok(cached);
        }

        // Fetch from docs.rs
        let url = Self::build_url(crate_name, version);
        let html = self.fetch_html(&url).await?;

        // Extract documentation
        let docs = html::extract_documentation(&html);

        // Cache result
        self.service
            .doc_cache()
            .set_crate_docs(crate_name, version, docs.clone())
            .await
            .map_err(|e| CallToolError::from_message(format!("Cache set failed: {e}")))?;

        Ok(docs)
    }

    /// Get crate documentation as plain text
    async fn fetch_crate_docs_as_text(
        &self,
        crate_name: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        let url = Self::build_url(crate_name, version);
        let html = self.fetch_html(&url).await?;
        Ok(html::html_to_text(&html))
    }

    /// Get crate documentation as raw HTML
    async fn fetch_crate_docs_as_html(
        &self,
        crate_name: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        let url = Self::build_url(crate_name, version);
        self.fetch_html(&url).await
    }
}

#[async_trait]
impl Tool for LookupCrateToolImpl {
    fn definition(&self) -> rust_mcp_sdk::schema::Tool {
        LookupCrateTool::tool()
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> std::result::Result<
        rust_mcp_sdk::schema::CallToolResult,
        rust_mcp_sdk::schema::CallToolError,
    > {
        let params: LookupCrateTool = serde_json::from_value(arguments).map_err(|e| {
            rust_mcp_sdk::schema::CallToolError::invalid_arguments(
                "lookup_crate",
                Some(format!("Parameter parsing failed: {e}")),
            )
        })?;

        let format = params.format.as_deref().unwrap_or("markdown");
        let content = match format {
            "text" => {
                self.fetch_crate_docs_as_text(&params.crate_name, params.version.as_deref())
                    .await?
            }
            "html" => {
                self.fetch_crate_docs_as_html(&params.crate_name, params.version.as_deref())
                    .await?
            }
            _ => {
                // "markdown" and other formats default to markdown
                self.fetch_crate_docs(&params.crate_name, params.version.as_deref())
                    .await?
            }
        };

        Ok(rust_mcp_sdk::schema::CallToolResult::text_content(vec![
            content.into(),
        ]))
    }
}

impl Default for LookupCrateToolImpl {
    fn default() -> Self {
        Self::new(Arc::new(super::DocService::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_url_without_version() {
        let url = LookupCrateToolImpl::build_url("serde", None);
        assert_eq!(url, "https://docs.rs/serde/");
    }

    #[test]
    fn test_build_url_with_version() {
        let url = LookupCrateToolImpl::build_url("serde", Some("1.0.0"));
        assert_eq!(url, "https://docs.rs/serde/1.0.0/");
    }
}
