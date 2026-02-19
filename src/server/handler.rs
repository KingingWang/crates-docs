//! MCP handler implementation

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

/// MCP server handler
pub struct CratesDocsHandler {
    server: Arc<CratesDocsServer>,
}

impl CratesDocsHandler {
    /// Create a new handler
    #[must_use]
    pub fn new(server: Arc<CratesDocsServer>) -> Self {
        Self { server }
    }

    /// Get the tool registry
    fn tool_registry(&self) -> &ToolRegistry {
        self.server.tool_registry()
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
        let tools = self.tool_registry().get_tools();

        Ok(ListToolsResult {
            tools,
            meta: None,
            next_cursor: None,
        })
    }

    /// Handle call tool request
    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        self.tool_registry()
            .execute_tool(
                &params.name,
                params
                    .arguments
                    .map_or_else(|| serde_json::Value::Null, serde_json::Value::Object),
            )
            .await
    }

    /// Handle list resources request
    async fn handle_list_resources_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourcesResult, RpcError> {
        // Resources are not currently provided
        Ok(ListResourcesResult {
            resources: vec![],
            meta: None,
            next_cursor: None,
        })
    }

    /// Handle read resource request
    async fn handle_read_resource_request(
        &self,
        _params: ReadResourceRequestParams,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ReadResourceResult, RpcError> {
        // Resources are not currently provided
        Err(RpcError::invalid_request().with_message("Resource not found".to_string()))
    }

    /// Handle list prompts request
    async fn handle_list_prompts_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListPromptsResult, RpcError> {
        // Prompts are not currently provided
        Ok(ListPromptsResult {
            prompts: vec![],
            meta: None,
            next_cursor: None,
        })
    }

    /// Handle get prompt request
    async fn handle_get_prompt_request(
        &self,
        _params: GetPromptRequestParams,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<GetPromptResult, RpcError> {
        // Prompts are not currently provided
        Err(RpcError::invalid_request().with_message("Prompt not found".to_string()))
    }
}

/// Core handler implementation (provides more control)
pub struct CratesDocsHandlerCore {
    server: Arc<CratesDocsServer>,
}

impl CratesDocsHandlerCore {
    /// Create a new core handler
    #[must_use]
    pub fn new(server: Arc<CratesDocsServer>) -> Self {
        Self { server }
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
            RequestFromClient::ListToolsRequest(_params) => {
                let tools = self.server.tool_registry().get_tools();
                Ok(ListToolsResult {
                    tools,
                    meta: None,
                    next_cursor: None,
                }
                .into())
            }
            RequestFromClient::CallToolRequest(params) => {
                let result = self
                    .server
                    .tool_registry()
                    .execute_tool(
                        &params.name,
                        params
                            .arguments
                            .map_or_else(|| serde_json::Value::Null, serde_json::Value::Object),
                    )
                    .await
                    .map_err(|_e| CallToolError::unknown_tool(params.name.clone()))?;
                Ok(result.into())
            }
            RequestFromClient::ListResourcesRequest(_params) => Ok(ListResourcesResult {
                resources: vec![],
                meta: None,
                next_cursor: None,
            }
            .into()),
            RequestFromClient::ReadResourceRequest(_params) => {
                Err(RpcError::invalid_request().with_message("Resource not found".to_string()))
            }
            RequestFromClient::ListPromptsRequest(_params) => Ok(ListPromptsResult {
                prompts: vec![],
                meta: None,
                next_cursor: None,
            }
            .into()),
            RequestFromClient::GetPromptRequest(_params) => {
                Err(RpcError::invalid_request().with_message("Prompt not found".to_string()))
            }
            RequestFromClient::InitializeRequest(_params) => {
                // Use default initialization handling
                Err(RpcError::method_not_found()
                    .with_message("Initialize request should be handled by runtime".to_string()))
            }
            _ => {
                // Other requests use default handling
                Err(RpcError::method_not_found().with_message("Unimplemented request type".to_string()))
            }
        }
    }

    /// Handle notification
    async fn handle_notification(
        &self,
        _notification: NotificationFromClient,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        // Notifications are not currently handled
        Ok(())
    }

    /// Handle error
    async fn handle_error(
        &self,
        _error: &RpcError,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        // Log error but don't interrupt
        tracing::error!("MCP error: {:?}", _error);
        Ok(())
    }
}
