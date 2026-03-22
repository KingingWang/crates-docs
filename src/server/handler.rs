//! MCP 请求处理器实现
//!
//! 提供 MCP 协议请求的处理逻辑，包括工具列表、工具调用、资源列表等。
//!
//! # 主要结构体
//!
//! - `CratesDocsHandler`: 标准 MCP 处理器
//! - `CratesDocsHandlerCore`: 核心处理器（提供更细粒度的控制）

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

/// MCP 服务器处理器
///
/// 实现标准 MCP 协议处理器接口，处理客户端请求。
///
/// # 字段
///
/// - `server`: 服务器实例的 Arc 引用
pub struct CratesDocsHandler {
    server: Arc<CratesDocsServer>,
}

impl CratesDocsHandler {
    /// 创建新的处理器
    ///
    /// # 参数
    ///
    /// * `server` - 服务器实例
    ///
    /// # 示例
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
        Self { server }
    }

    /// 获取工具注册表
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
        let trace_id = Uuid::new_v4().to_string();
        let span = info_span!(
            "list_tools",
            trace_id = %trace_id,
        );

        async {
            tracing::debug!("Listing available tools");
            let tools = self.tool_registry().get_tools();
            tracing::debug!("Found {} tools", tools.len());

            Ok(ListToolsResult {
                tools,
                meta: None,
                next_cursor: None,
            })
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
        let trace_id = Uuid::new_v4().to_string();
        let tool_name = params.name.clone();
        let span = info_span!(
            "call_tool",
            trace_id = %trace_id,
            tool = %tool_name,
        );

        async {
            tracing::info!("Executing tool: {}", tool_name);
            let start = std::time::Instant::now();

            let result = self
                .tool_registry()
                .execute_tool(
                    &tool_name,
                    params
                        .arguments
                        .map_or_else(|| serde_json::Value::Null, serde_json::Value::Object),
                )
                .await;

            let duration = start.elapsed();
            match &result {
                Ok(_) => {
                    tracing::info!("Tool {} executed successfully in {:?}", tool_name, duration);
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

            result
        }
        .instrument(span)
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

    async fn execute_tool_request(&self, params: CallToolRequestParams) -> ResultFromServer {
        let trace_id = Uuid::new_v4().to_string();
        let tool_name = params.name.clone();
        let span = info_span!(
            "execute_tool_core",
            trace_id = %trace_id,
            tool = %tool_name,
        );

        async {
            tracing::info!("Executing tool request: {}", tool_name);
            let start = std::time::Instant::now();

            let result = self
                .server
                .tool_registry()
                .execute_tool(
                    &tool_name,
                    params
                        .arguments
                        .map_or_else(|| serde_json::Value::Null, serde_json::Value::Object),
                )
                .await;

            let duration = start.elapsed();
            match &result {
                Ok(_) => {
                    tracing::info!("Tool {} executed successfully in {:?}", tool_name, duration);
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

            result.unwrap_or_else(CallToolResult::from).into()
        }
        .instrument(span)
        .await
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
                Ok(self.execute_tool_request(params).await)
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

#[cfg(test)]
mod tests {
    use super::CratesDocsHandlerCore;
    use crate::server::CratesDocsServer;
    use rust_mcp_sdk::schema::{CallToolRequestParams, CallToolResult, ContentBlock};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_execute_tool_request_preserves_tool_errors() {
        let server = Arc::new(CratesDocsServer::new(crate::AppConfig::default()).unwrap());
        let handler = CratesDocsHandlerCore::new(server);
        let result = handler
            .execute_tool_request(CallToolRequestParams {
                arguments: Some(serde_json::Map::from_iter([(
                    "verbose".to_string(),
                    serde_json::Value::String("bad".to_string()),
                )])),
                meta: None,
                name: "health_check".to_string(),
                task: None,
            })
            .await;

        let result = CallToolResult::try_from(result).unwrap();
        assert_eq!(result.is_error, Some(true));

        let Some(ContentBlock::TextContent(text)) = result.content.first() else {
            panic!("expected first content block to be text");
        };

        assert!(text.text.contains("health_check"));
        assert!(text.text.contains("Parameter parsing failed"));
        assert!(!text.text.contains("Unknown tool"));
    }
}
