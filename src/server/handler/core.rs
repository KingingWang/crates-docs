//! Shared core handling logic

use crate::metrics::ServerMetrics;
use crate::server::CratesDocsServer;
use crate::tools::ToolRegistry;
use rust_mcp_sdk::schema::{
    CallToolRequestParams, ListPromptsResult, ListResourcesResult, ListToolsResult,
};
use std::sync::Arc;
use tracing::{info_span, Instrument};
use uuid::Uuid;

use super::config::HandlerConfig;
use super::types::ToolExecutionResult;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppConfig;

    #[tokio::test]
    async fn test_handler_core_execute_tool() {
        let server = Arc::new(CratesDocsServer::new(AppConfig::default()).unwrap());
        let core = HandlerCore::new(server);

        let result = core
            .execute_tool(rust_mcp_sdk::schema::CallToolRequestParams {
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
        let server = Arc::new(CratesDocsServer::new(AppConfig::default()).unwrap());
        let metrics = Arc::new(ServerMetrics::new());
        let core = HandlerCore::new(server).with_metrics(metrics.clone());

        let _result = core
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
    async fn test_handler_core_list_methods() {
        let server = Arc::new(CratesDocsServer::new(AppConfig::default()).unwrap());
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
