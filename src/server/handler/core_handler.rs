//! Core handler implementation

use async_trait::async_trait;
use rust_mcp_sdk::{
    mcp_server::ServerHandlerCore,
    schema::{NotificationFromClient, RequestFromClient, ResultFromServer, RpcError},
    McpServer,
};
use std::sync::Arc;

use super::config::HandlerConfig;
use super::core::HandlerCore;
use crate::metrics::ServerMetrics;
use crate::server::CratesDocsServer;

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
        _runtime: Arc<dyn McpServer>,
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
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handle error
    async fn handle_error(
        &self,
        error: &RpcError,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        tracing::error!("MCP error: {:?}", error);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppConfig;
    use rust_mcp_sdk::schema::{CallToolResult, ContentBlock};

    #[tokio::test]
    async fn test_crates_docs_handler_core_execute_tool_request() {
        let server = Arc::new(CratesDocsServer::new(AppConfig::default()).unwrap());
        let handler = CratesDocsHandlerCore::new(server);

        let result = handler
            .core()
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

        let call_result = result.into_call_tool_result();
        assert!(call_result.is_err() || call_result.unwrap().is_error == Some(true));
    }

    #[tokio::test]
    async fn test_execute_tool_request_preserves_tool_errors() {
        let server = Arc::new(CratesDocsServer::new(AppConfig::default()).unwrap());
        let handler = CratesDocsHandlerCore::new(server);

        let tool_result = handler
            .core()
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

        let result = CallToolResult::try_from(tool_result.into_result_from_server()).unwrap();
        assert_eq!(result.is_error, Some(true));

        let Some(ContentBlock::TextContent(text)) = result.content.first() else {
            panic!("expected first content block to be text");
        };

        assert!(text.text.contains("health_check"));
        assert!(text.text.contains("Parameter parsing failed"));
        assert!(!text.text.contains("Unknown tool"));
    }
}
