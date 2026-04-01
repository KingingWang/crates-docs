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
use super::types::ToolExecutionResult;
use crate::metrics::ServerMetrics;
use crate::server::CratesDocsServer;
use crate::tools::ToolRegistry;

/// MCP server handler
///
/// Implements standard MCP protocol handler interface, handles client requests.
///
/// # Fields
///
/// - `server`: Server instance
/// - `config`: Handler configuration
/// - `metrics`: Optional metrics collector
pub struct CratesDocsHandler {
    server: Arc<CratesDocsServer>,
    config: HandlerConfig,
    metrics: Option<Arc<ServerMetrics>>,
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
            server,
            config: HandlerConfig::default(),
            metrics: None,
        }
    }

    /// Create handler with configuration
    #[must_use]
    pub fn with_config(server: Arc<CratesDocsServer>, config: HandlerConfig) -> Self {
        Self {
            server,
            config,
            metrics: None,
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
            server,
            config: base_config.merge(override_config),
            metrics: None,
        }
    }

    /// Set metrics
    #[must_use]
    pub fn with_metrics(self, metrics: Arc<ServerMetrics>) -> Self {
        Self {
            metrics: Some(metrics),
            ..self
        }
    }

    /// Get server reference
    #[must_use]
    pub fn server(&self) -> &Arc<CratesDocsServer> {
        &self.server
    }

    /// Get tool registry
    #[must_use]
    pub fn tool_registry(&self) -> &ToolRegistry {
        self.server.tool_registry()
    }

    /// Get configuration
    #[must_use]
    pub fn config(&self) -> &HandlerConfig {
        &self.config
    }

    /// Get metrics (optional)
    #[must_use]
    pub fn metrics(&self) -> Option<&Arc<ServerMetrics>> {
        self.metrics.as_ref()
    }

    /// Get all tools list
    #[must_use]
    pub fn list_tools(&self) -> ListToolsResult {
        ListToolsResult {
            tools: self.tool_registry().get_tools(),
            meta: None,
            next_cursor: None,
        }
    }

    /// Get empty resources list
    #[must_use]
    pub fn list_resources(&self) -> ListResourcesResult {
        ListResourcesResult {
            resources: vec![],
            meta: None,
            next_cursor: None,
        }
    }

    /// Get empty prompts list
    #[must_use]
    pub fn list_prompts(&self) -> ListPromptsResult {
        ListPromptsResult {
            prompts: vec![],
            meta: None,
            next_cursor: None,
        }
    }

    /// Execute tool call (core logic)
    ///
    /// This method encapsulates the complete tool execution flow:
    /// - tracing tracking
    /// - timing statistics
    /// - metrics recording (if enabled)
    ///
    /// # Returns
    ///
    /// Returns `ToolExecutionResult`, can be converted to different types to adapt to different traits
    pub async fn execute_tool(&self, params: CallToolRequestParams) -> ToolExecutionResult {
        let trace_id = Uuid::new_v4().to_string();
        let tool_name = params.name.clone();
        let span = info_span!(
            "execute_tool",
            trace_id = %trace_id,
            tool = %tool_name,
            verbose = self.config.verbose_logging,
        );

        async {
            tracing::info!("Executing tool: {}", tool_name);
            let start = std::time::Instant::now();

            let arguments = params
                .arguments
                .map_or_else(|| serde_json::Value::Null, serde_json::Value::Object);

            let result = self
                .tool_registry()
                .execute_tool(&tool_name, arguments)
                .await;

            let duration = start.elapsed();
            let success = result.is_ok();

            // Log results
            match &result {
                Ok(_) => {
                    tracing::info!("Tool {} executed successfully in {:?}", tool_name, duration);
                    if self.config.verbose_logging {
                        tracing::debug!("Verbose: Tool execution details available");
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Tool {} execution failed after {:?}: {:?}",
                        tool_name,
                        duration,
                        e
                    );
                }
            }

            // Record metrics (if enabled)
            if let Some(metrics) = &self.metrics {
                metrics.record_request(&tool_name, success, duration);
            }

            ToolExecutionResult {
                tool_name,
                duration,
                success,
                result,
            }
        }
        .instrument(span)
        .await
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
            let result = self.list_tools();
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
        self.execute_tool(params).await.into_call_tool_result()
    }

    /// Handle list resources request
    async fn handle_list_resources_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourcesResult, RpcError> {
        Ok(self.list_resources())
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
        Ok(self.list_prompts())
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
    async fn test_crates_docs_handler_execute_tool() {
        let server = Arc::new(CratesDocsServer::new(AppConfig::default()).unwrap());
        let handler = CratesDocsHandler::new(server);

        let result = handler
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

        assert!(handler.config().verbose_logging);
        assert!(!handler.config().enable_metrics);
    }

    #[tokio::test]
    async fn test_handler_with_metrics() {
        let server = Arc::new(CratesDocsServer::new(AppConfig::default()).unwrap());
        let metrics = Arc::new(ServerMetrics::new());
        let handler = CratesDocsHandler::new(server).with_metrics(metrics.clone());

        let _result = handler
            .execute_tool(rust_mcp_sdk::schema::CallToolRequestParams {
                arguments: None,
                meta: None,
                name: "health_check".to_string(),
                task: None,
            })
            .await;

        // Verify metrics are recorded
        let metrics_output = metrics.export().unwrap();
        assert!(metrics_output.contains("mcp_requests_total"));
    }

    #[tokio::test]
    async fn test_handler_list_methods() {
        let server = Arc::new(CratesDocsServer::new(AppConfig::default()).unwrap());
        let handler = CratesDocsHandler::new(server);

        let tools = handler.list_tools();
        assert!(!tools.tools.is_empty());
        assert_eq!(tools.tools.len(), 4); // 4 default tools

        let resources = handler.list_resources();
        assert!(resources.resources.is_empty());

        let prompts = handler.list_prompts();
        assert!(prompts.prompts.is_empty());
    }
}
