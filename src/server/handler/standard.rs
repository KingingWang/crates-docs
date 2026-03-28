//! MCP server handler implementation

use async_trait::async_trait;
use rust_mcp_sdk::{
    mcp_server::ServerHandler,
    schema::{
        CallToolError, CallToolRequestParams, CallToolResult, GetPromptRequestParams,
        GetPromptResult, ListPromptsResult, ListResourcesResult, ListToolsResult,
        PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResult, RpcError,
    },
    McpServer,
};
use std::sync::Arc;
use tracing::{info_span, Instrument};
use uuid::Uuid;

use super::config::HandlerConfig;
use super::core::HandlerCore;
use crate::metrics::ServerMetrics;
use crate::server::CratesDocsServer;

/// MCP server handler
///
/// Implements standard MCP protocol handler interface, handles client requests.
/// Delegates all core logic to `HandlerCore`.
///
/// # Fields
///
/// - `core`: Shared core handling logic
pub struct CratesDocsHandler {
    core: HandlerCore,
}

impl CratesDocsHandler {
    /// Create new handler
    ///
    /// # Arguments
    ///
    /// * `server` - Server instance
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use crates_docs::server::{CratesDocsServer, CratesDocsHandler};
    /// use crates_docs::AppConfig;
    ///
    /// let config = AppConfig::default();
    /// let server = Arc::new(CratesDocsServer::new(config).unwrap());
    /// let handler = CratesDocsHandler::new(server);
    /// ```
    #[must_use]
    pub fn new(server: Arc<CratesDocsServer>) -> Self {
        Self {
            core: HandlerCore::new(server),
        }
    }

    /// Create handler with configuration
    #[must_use]
    pub fn with_config(server: Arc<CratesDocsServer>, config: HandlerConfig) -> Self {
        Self {
            core: HandlerCore::with_config(server, config),
        }
    }

    /// Create handler with merged configuration
    #[must_use]
    pub fn with_merged_config(
        server: Arc<CratesDocsServer>,
        base_config: HandlerConfig,
        override_config: Option<HandlerConfig>,
    ) -> Self {
        Self {
            core: HandlerCore::with_merged_config(server, base_config, override_config),
        }
    }

    /// Set metrics
    #[must_use]
    pub fn with_metrics(self, metrics: Arc<ServerMetrics>) -> Self {
        Self {
            core: self.core.with_metrics(metrics),
        }
    }

    /// Get core handler
    #[must_use]
    pub fn core(&self) -> &HandlerCore {
        &self.core
    }

    /// Get server reference
    #[must_use]
    pub fn server(&self) -> &Arc<CratesDocsServer> {
        self.core.server()
    }
}

#[async_trait]
impl ServerHandler for CratesDocsHandler {
    /// Handle list tools request
    async fn handle_list_tools_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        let trace_id = Uuid::new_v4().to_string();
        let span = info_span!("list_tools", trace_id = %trace_id);

        async {
            tracing::debug!("Listing available tools");
            let result = self.core.list_tools();
            tracing::debug!("Found {} tools", result.tools.len());
            Ok(result)
        }
        .instrument(span)
        .await
    }

    /// Handle call tool request
    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        self.core.execute_tool(params).await.into_call_tool_result()
    }

    /// Handle list resources request
    async fn handle_list_resources_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourcesResult, RpcError> {
        Ok(self.core.list_resources())
    }

    /// Handle read resource request
    async fn handle_read_resource_request(
        &self,
        _params: ReadResourceRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ReadResourceResult, RpcError> {
        Err(RpcError::invalid_request().with_message("Resource not found".to_string()))
    }

    /// Handle list prompts request
    async fn handle_list_prompts_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListPromptsResult, RpcError> {
        Ok(self.core.list_prompts())
    }

    /// Handle get prompt request
    async fn handle_get_prompt_request(
        &self,
        _params: GetPromptRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<GetPromptResult, RpcError> {
        Err(RpcError::invalid_request().with_message("Prompt not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppConfig;

    #[tokio::test]
    async fn test_crates_docs_handler_delegation() {
        let server = Arc::new(CratesDocsServer::new(AppConfig::default()).unwrap());
        let handler = CratesDocsHandler::new(server);

        let result = handler
            .core()
            .execute_tool(rust_mcp_sdk::schema::CallToolRequestParams {
                arguments: Some(serde_json::Map::new()),
                meta: None,
                name: "health_check".to_string(),
                task: None,
            })
            .await;

        assert!(result.success);
    }

    #[tokio::test]
    async fn test_handler_with_merged_config() {
        let server = Arc::new(CratesDocsServer::new(AppConfig::default()).unwrap());
        let base_config = HandlerConfig::default();
        let override_config = HandlerConfig::new().with_verbose_logging();

        let handler =
            CratesDocsHandler::with_merged_config(server, base_config, Some(override_config));

        assert!(handler.core().config().verbose_logging);
        assert!(!handler.core().config().enable_metrics);
    }
}
