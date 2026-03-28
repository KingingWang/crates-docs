//! MCP request handler implementation
//!
//! Provides MCP protocol request handling logic, including tool listing, tool invocation, and resource lists.
//!
//! # Main Structs
//!
//! - `HandlerCore`: Shared core handling logic (internal use)
//! - `CratesDocsHandler`: Standard MCP handler
//! - `CratesDocsHandlerCore`: Core handler (provides more fine-grained control)
//!
//! # Design Pattern
//!
//! Uses composition pattern to eliminate code duplication:
//! - `HandlerCore` encapsulates all shared handling logic
//! - `CratesDocsHandler` and `CratesDocsHandlerCore` delegate to `HandlerCore`
//! - Supports config merging and optional metrics integration

use crate::metrics::ServerMetrics;
use crate::server::CratesDocsServer;
use crate::tools::ToolRegistry;
use async_trait::async_trait;
use rust_mcp_sdk::{
    mcp_server::{ServerHandler, ServerHandlerCore},
    schema::{
        CallToolError, CallToolRequestParams, CallToolResult, GetPromptRequestParams,
        GetPromptResult, ListPromptsResult, ListResourcesResult, ListToolsResult,
        NotificationFromClient, PaginatedRequestParams, ReadResourceRequestParams,
        ReadResourceResult, RequestFromClient, ResultFromServer, RpcError,
    },
    McpServer,
};
use std::sync::Arc;
use tracing::{info_span, Instrument};
use uuid::Uuid;

/// Tool execution result (supports different return type conversions)
#[derive(Debug)]
pub struct ToolExecutionResult {
    /// Tool name
    pub tool_name: String,
    /// Execution duration
    pub duration: std::time::Duration,
    /// Whether successful
    pub success: bool,
    /// Original result (for converting to different types)
    pub result: std::result::Result<CallToolResult, CallToolError>,
}

impl ToolExecutionResult {
    /// Convert to `CallToolResult` (for `ServerHandler`)
    pub fn into_call_tool_result(self) -> std::result::Result<CallToolResult, CallToolError> {
        self.result
    }

    /// Convert to `ResultFromServer` (for `ServerHandlerCore`)
    pub fn into_result_from_server(self) -> ResultFromServer {
        self.result.unwrap_or_else(CallToolResult::from).into()
    }
}

/// Handler configuration (supports merging)
///
/// Used to configure handler behavior, such as metrics integration, log level, etc.
#[derive(Debug, Clone, Default)]
pub struct HandlerConfig {
    /// Whether to enable verbose logging
    pub verbose_logging: bool,
    /// Whether to record metrics
    pub enable_metrics: bool,
}

impl HandlerConfig {
    /// Create new configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable verbose logging
    #[must_use]
    pub fn with_verbose_logging(self) -> Self {
        Self {
            verbose_logging: true,
            ..self
        }
    }

    /// Enable metrics
    #[must_use]
    pub fn with_metrics(self) -> Self {
        Self {
            enable_metrics: true,
            ..self
        }
    }

    /// Merge configuration (other takes precedence over self)
    #[must_use]
    pub fn merge(self, other: Option<Self>) -> Self {
        match other {
            Some(other) => Self {
                verbose_logging: other.verbose_logging || self.verbose_logging,
                enable_metrics: other.enable_metrics || self.enable_metrics,
            },
            None => self,
        }
    }
}

/// Shared core handling logic
///
/// Encapsulates all MCP request handling shared logic, eliminating duplication between `CratesDocsHandler` and
/// `CratesDocsHandlerCore`.
///
/// # Design
///
/// - Provides core methods for tool execution, list queries, etc.
/// - Supports optional metrics integration
/// - Supports configuration merging
pub struct HandlerCore {
    server: Arc<CratesDocsServer>,
    config: HandlerConfig,
    metrics: Option<Arc<ServerMetrics>>,
}

impl HandlerCore {
    /// Create new core handler
    ///
    /// # Arguments
    ///
    /// * `server` - Server instance
    #[must_use]
    pub fn new(server: Arc<CratesDocsServer>) -> Self {
        Self {
            server,
            config: HandlerConfig::default(),
            metrics: None,
        }
    }

    /// Create core handler with configuration
    #[must_use]
    pub fn with_config(server: Arc<CratesDocsServer>, config: HandlerConfig) -> Self {
        Self {
            server,
            config,
            metrics: None,
        }
    }

    /// Create core handler with merged configuration
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
        _runtime: std::sync::Arc<dyn McpServer>,
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
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        self.core.execute_tool(params).await.into_call_tool_result()
    }

    /// Handle list resources request
    async fn handle_list_resources_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourcesResult, RpcError> {
        Ok(self.core.list_resources())
    }

    /// Handle read resource request
    async fn handle_read_resource_request(
        &self,
        _params: ReadResourceRequestParams,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ReadResourceResult, RpcError> {
        Err(RpcError::invalid_request().with_message("Resource not found".to_string()))
    }

    /// Handle list prompts request
    async fn handle_list_prompts_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListPromptsResult, RpcError> {
        Ok(self.core.list_prompts())
    }

    /// Handle get prompt request
    async fn handle_get_prompt_request(
        &self,
        _params: GetPromptRequestParams,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<GetPromptResult, RpcError> {
        Err(RpcError::invalid_request().with_message("Prompt not found".to_string()))
    }
}

/// Core handler implementation (provides more control)
///
/// Implements more fine-grained MCP protocol handler interface.
/// Delegates all core logic to `HandlerCore`.
pub struct CratesDocsHandlerCore {
    core: HandlerCore,
}

impl CratesDocsHandlerCore {
    /// Create a new core handler
    #[must_use]
    pub fn new(server: Arc<CratesDocsServer>) -> Self {
        Self {
            core: HandlerCore::new(server),
        }
    }

    /// Create core handler with configuration
    #[must_use]
    pub fn with_config(server: Arc<CratesDocsServer>, config: HandlerConfig) -> Self {
        Self {
            core: HandlerCore::with_config(server, config),
        }
    }

    /// Create core handler with merged configuration
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
impl ServerHandlerCore for CratesDocsHandlerCore {
    /// Handle request
    async fn handle_request(
        &self,
        request: RequestFromClient,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ResultFromServer, RpcError> {
        match request {
            RequestFromClient::ListToolsRequest(_params) => Ok(self.core.list_tools().into()),
            RequestFromClient::CallToolRequest(params) => Ok(self
                .core
                .execute_tool(params)
                .await
                .into_result_from_server()),
            RequestFromClient::ListResourcesRequest(_params) => {
                Ok(self.core.list_resources().into())
            }
            RequestFromClient::ReadResourceRequest(_params) => {
                Err(RpcError::invalid_request().with_message("Resource not found".to_string()))
            }
            RequestFromClient::ListPromptsRequest(_params) => Ok(self.core.list_prompts().into()),
            RequestFromClient::GetPromptRequest(_params) => {
                Err(RpcError::invalid_request().with_message("Prompt not found".to_string()))
            }
            RequestFromClient::InitializeRequest(_params) => Err(RpcError::method_not_found()
                .with_message("Initialize request should be handled by runtime".to_string())),
            _ => {
                Err(RpcError::method_not_found()
                    .with_message("Unimplemented request type".to_string()))
            }
        }
    }

    /// Handle notification
    async fn handle_notification(
        &self,
        _notification: NotificationFromClient,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handle error
    async fn handle_error(
        &self,
        error: &RpcError,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        tracing::error!("MCP error: {:?}", error);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{CratesDocsHandler, CratesDocsHandlerCore, HandlerConfig, HandlerCore};
    use crate::metrics::ServerMetrics;
    use crate::server::CratesDocsServer;
    use rust_mcp_sdk::schema::{CallToolRequestParams, CallToolResult, ContentBlock};
    use std::sync::Arc;

    #[test]
    fn test_handler_config_merge() {
        // Test with override case
        let base = HandlerConfig::default();
        let override_config = HandlerConfig::new().with_verbose_logging().with_metrics();

        let merged = base.merge(Some(override_config));
        assert!(merged.verbose_logging);
        assert!(merged.enable_metrics);

        // Test empty override (use new base)
        let base2 = HandlerConfig::default();
        let merged_empty = base2.merge(None);
        assert!(!merged_empty.verbose_logging);
        assert!(!merged_empty.enable_metrics);
    }

    #[test]
    fn test_handler_config_chained() {
        let config = HandlerConfig::new().with_verbose_logging().with_metrics();

        assert!(config.verbose_logging);
        assert!(config.enable_metrics);
    }

    #[tokio::test]
    async fn test_handler_core_execute_tool() {
        let server = Arc::new(CratesDocsServer::new(crate::AppConfig::default()).unwrap());
        let core = HandlerCore::new(server);

        let result = core
            .execute_tool(CallToolRequestParams {
                arguments: Some(serde_json::Map::from_iter([(
                    "verbose".to_string(),
                    serde_json::Value::String("bad".to_string()),
                )])),
                meta: None,
                name: "health_check".to_string(),
                task: None,
            })
            .await;

        assert!(!result.success);
        assert_eq!(result.tool_name, "health_check");
    }

    #[tokio::test]
    async fn test_handler_core_with_metrics() {
        let server = Arc::new(CratesDocsServer::new(crate::AppConfig::default()).unwrap());
        let metrics = Arc::new(ServerMetrics::new());
        let core = HandlerCore::new(server).with_metrics(metrics.clone());

        let _result = core
            .execute_tool(CallToolRequestParams {
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
    async fn test_crates_docs_handler_delegation() {
        let server = Arc::new(CratesDocsServer::new(crate::AppConfig::default()).unwrap());
        let handler = CratesDocsHandler::new(server);

        let result = handler
            .core()
            .execute_tool(CallToolRequestParams {
                arguments: Some(serde_json::Map::new()),
                meta: None,
                name: "health_check".to_string(),
                task: None,
            })
            .await;

        assert!(result.success);
    }

    #[tokio::test]
    async fn test_crates_docs_handler_core_execute_tool_request() {
        let server = Arc::new(CratesDocsServer::new(crate::AppConfig::default()).unwrap());
        let handler = CratesDocsHandlerCore::new(server);

        let result = handler
            .core()
            .execute_tool(CallToolRequestParams {
                arguments: Some(serde_json::Map::from_iter([(
                    "verbose".to_string(),
                    serde_json::Value::String("bad".to_string()),
                )])),
                meta: None,
                name: "health_check".to_string(),
                task: None,
            })
            .await;

        let call_result = result.into_call_tool_result();
        assert!(call_result.is_err() || call_result.unwrap().is_error == Some(true));
    }

    #[tokio::test]
    async fn test_execute_tool_request_preserves_tool_errors() {
        let server = Arc::new(CratesDocsServer::new(crate::AppConfig::default()).unwrap());
        let handler = CratesDocsHandlerCore::new(server);

        let tool_result = handler
            .core()
            .execute_tool(CallToolRequestParams {
                arguments: Some(serde_json::Map::from_iter([(
                    "verbose".to_string(),
                    serde_json::Value::String("bad".to_string()),
                )])),
                meta: None,
                name: "health_check".to_string(),
                task: None,
            })
            .await;

        let result = CallToolResult::try_from(tool_result.into_result_from_server()).unwrap();
        assert_eq!(result.is_error, Some(true));

        let Some(ContentBlock::TextContent(text)) = result.content.first() else {
            panic!("expected first content block to be text");
        };

        assert!(text.text.contains("health_check"));
        assert!(text.text.contains("Parameter parsing failed"));
        assert!(!text.text.contains("Unknown tool"));
    }

    #[tokio::test]
    async fn test_handler_with_merged_config() {
        let server = Arc::new(CratesDocsServer::new(crate::AppConfig::default()).unwrap());
        let base_config = HandlerConfig::default();
        let override_config = HandlerConfig::new().with_verbose_logging();

        let handler =
            CratesDocsHandler::with_merged_config(server, base_config, Some(override_config));

        assert!(handler.core().config().verbose_logging);
        assert!(!handler.core().config().enable_metrics);
    }

    #[tokio::test]
    async fn test_handler_core_list_methods() {
        let server = Arc::new(CratesDocsServer::new(crate::AppConfig::default()).unwrap());
        let core = HandlerCore::new(server);

        let tools = core.list_tools();
        assert!(!tools.tools.is_empty());
        assert_eq!(tools.tools.len(), 4); // 4 default tools

        let resources = core.list_resources();
        assert!(resources.resources.is_empty());

        let prompts = core.list_prompts();
        assert!(prompts.prompts.is_empty());
    }
}
