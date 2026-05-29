//! Lookup crate documentation tool
//!
//! Provides functionality to retrieve complete documentation for a Rust crate
//! from docs.rs. Returns the main documentation page content including modules,
//! structs, functions, etc.

#![allow(missing_docs)]

//! Tool parameters for looking up crate documentation from docs.rs
//!
//! This struct defines the parameters needed to retrieve documentation
//! for a specific Rust crate, including the crate name, optional version,
//! and desired output format.

use crate::tools::docs::html;
use crate::tools::docs::DocService;
use crate::tools::Tool;
use async_trait::async_trait;
use rust_mcp_sdk::schema::CallToolError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const TOOL_NAME: &str = "lookup_crate";
///
/// Used to specify which crate to look up and in what format to return the documentation.
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
/// Parameters for the `lookup_crate` tool
///
/// Defines the input parameters for retrieving crate documentation,
/// including the crate name, optional version specification, and output format.
#[derive(Debug, Clone, Deserialize, Serialize, rust_mcp_sdk::macros::JsonSchema)]
pub struct LookupCrateTool {
    /// Crate name to lookup (e.g., "serde", "tokio", "reqwest")
    #[json_schema(
        title = "Crate Name",
        description = "Crate name to lookup, e.g.: serde, tokio, reqwest"
    )]
    pub crate_name: String,

    /// Crate version (optional, defaults to latest)
    #[json_schema(
        title = "Version",
        description = "Crate version, e.g.: 1.0.0. Uses latest version if not specified"
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

/// Implementation of the lookup crate documentation tool
///
/// Handles the execution of crate documentation lookups, including
/// cache management, HTTP fetching from docs.rs, and result formatting.
pub struct LookupCrateToolImpl {
    /// Shared document service for HTTP requests and caching
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
        super::build_docs_url(crate_name, version)
    }

    async fn fetch_crate_html(
        &self,
        crate_name: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        if let Some(cached) = self
            .service
            .doc_cache()
            .get_crate_html(crate_name, version)
            .await
        {
            return Ok(cached.to_string());
        }

        let url = Self::build_url(crate_name, version);
        let html = self.service.fetch_html(&url, Some(TOOL_NAME)).await?;

        self.service
            .doc_cache()
            .set_crate_html(crate_name, version, html.clone())
            .await
            .map_err(|e| {
                CallToolError::from_message(format!("[{TOOL_NAME}] Cache set failed: {e}"))
            })?;

        Ok(html)
    }

    /// Get crate documentation (markdown format)
    ///
    /// Returns `Arc<str>` to preserve shared ownership on cache hits,
    /// avoiding unnecessary cloning of large documentation strings.
    async fn fetch_crate_docs(
        &self,
        crate_name: &str,
        version: Option<&str>,
    ) -> std::result::Result<Arc<str>, CallToolError> {
        // Try cache first - returns Arc<str> directly without cloning
        if let Some(cached) = self
            .service
            .doc_cache()
            .get_crate_docs(crate_name, version)
            .await
        {
            return Ok(cached);
        }

        let html = self.fetch_crate_html(crate_name, version).await?;

        // Extract documentation into Arc<str> for shared ownership
        let docs: Arc<str> = Arc::from(html::extract_documentation(&html).into_boxed_str());

        // Cache result - convert Arc<str> to String for the cache
        self.service
            .doc_cache()
            .set_crate_docs(crate_name, version, docs.to_string())
            .await
            .map_err(|e| {
                CallToolError::from_message(format!("[{TOOL_NAME}] Cache set failed: {e}"))
            })?;

        Ok(docs)
    }

    /// Get crate documentation as plain text
    async fn fetch_crate_docs_as_text(
        &self,
        crate_name: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        let html = self.fetch_crate_html(crate_name, version).await?;
        Ok(html::extract_documentation_as_text(&html))
    }

    /// Get crate documentation as raw HTML
    async fn fetch_crate_docs_as_html(
        &self,
        crate_name: &str,
        version: Option<&str>,
    ) -> std::result::Result<String, CallToolError> {
        let html = self.fetch_crate_html(crate_name, version).await?;
        Ok(html::extract_documentation_html(&html))
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

        // Propagate the detailed parse error (e.g. "Invalid format 'xml'. Expected
        // one of: ...") rather than masking it with a generic message, so callers
        // get actionable feedback.
        super::validate_crate_name(&params.crate_name)?;
        super::validate_version(params.version.as_deref())?;

        let format = super::parse_format(params.format.as_deref())?;
        let content = match format {
            super::Format::Text => {
                self.fetch_crate_docs_as_text(&params.crate_name, params.version.as_deref())
                    .await?
            }
            super::Format::Html => {
                self.fetch_crate_docs_as_html(&params.crate_name, params.version.as_deref())
                    .await?
            }
            super::Format::Json => {
                return Err(rust_mcp_sdk::schema::CallToolError::invalid_arguments(
                    "lookup_crate",
                    Some("JSON format is not supported by this tool".to_string()),
                ))
            }
            super::Format::Markdown => self
                .fetch_crate_docs(&params.crate_name, params.version.as_deref())
                .await
                .map(|arc| arc.to_string())?,
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
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_build_url_without_version() {
        std::env::set_var("CRATES_DOCS_DOCS_RS_URL", "https://docs.rs");
        let url = LookupCrateToolImpl::build_url("serde", None);
        assert_eq!(url, "https://docs.rs/serde/");
        std::env::remove_var("CRATES_DOCS_DOCS_RS_URL");
    }

    #[test]
    #[serial]
    fn test_build_url_with_version() {
        std::env::set_var("CRATES_DOCS_DOCS_RS_URL", "https://docs.rs");
        let url = LookupCrateToolImpl::build_url("serde", Some("1.0.0"));
        assert_eq!(url, "https://docs.rs/serde/1.0.0/");
        std::env::remove_var("CRATES_DOCS_DOCS_RS_URL");
    }

    #[test]
    #[serial]
    fn test_build_url_with_custom_base() {
        std::env::set_var("CRATES_DOCS_DOCS_RS_URL", "http://mock-server");
        let url = LookupCrateToolImpl::build_url("serde", None);
        assert_eq!(url, "http://mock-server/serde/");
        std::env::remove_var("CRATES_DOCS_DOCS_RS_URL");
    }
}
