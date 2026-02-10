//! MCP 处理器实现

use crate::server::CratesDocsServer;
use crate::tools::ToolRegistry;
use async_trait::async_trait;
use rust_mcp_sdk::{
    mcp_server::{ServerHandler, ServerHandlerCore},
    schema::{
        CallToolError, CallToolResult, CallToolRequestParams, GetPromptRequestParams,
        GetPromptResult, ListPromptsResult, ListResourcesResult, ListToolsResult,
        NotificationFromClient, PaginatedRequestParams, ReadResourceRequestParams,
        ReadResourceResult, RequestFromClient, ResultFromServer, RpcError,
    },
    McpServer,
};
use std::sync::Arc;

/// MCP 服务器处理器
pub struct CratesDocsHandler {
    server: Arc<CratesDocsServer>,
}

impl CratesDocsHandler {
    /// 创建新的处理器
    #[must_use] 
    pub fn new(server: Arc<CratesDocsServer>) -> Self {
        Self { server }
    }
    
    /// 获取工具注册器
    fn tool_registry(&self) -> &ToolRegistry {
        self.server.tool_registry()
    }
}

#[async_trait]
impl ServerHandler for CratesDocsHandler {
    /// 处理列出工具请求
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
    
    /// 处理调用工具请求
    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        self.tool_registry()
            .execute_tool(&params.name, params.arguments.map_or_else(|| serde_json::Value::Null, serde_json::Value::Object))
            .await
    }
    
    /// 处理列出资源请求
    async fn handle_list_resources_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourcesResult, RpcError> {
        // 当前不提供资源
        Ok(ListResourcesResult {
            resources: vec![],
            meta: None,
            next_cursor: None,
        })
    }
    
    /// 处理读取资源请求
    async fn handle_read_resource_request(
        &self,
        _params: ReadResourceRequestParams,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ReadResourceResult, RpcError> {
        // 当前不提供资源
        Err(RpcError::invalid_request()
            .with_message("资源未找到".to_string()))
    }
    
    /// 处理列出提示请求
    async fn handle_list_prompts_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListPromptsResult, RpcError> {
        // 当前不提供提示
        Ok(ListPromptsResult {
            prompts: vec![],
            meta: None,
            next_cursor: None,
        })
    }
    
    /// 处理获取提示请求
    async fn handle_get_prompt_request(
        &self,
        _params: GetPromptRequestParams,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<GetPromptResult, RpcError> {
        // 当前不提供提示
        Err(RpcError::invalid_request()
            .with_message("提示未找到".to_string()))
    }
}

// /// 将处理器转换为 MCP 服务器处理器
// impl From<CratesDocsHandler> for rust_mcp_sdk::mcp_server::McpServerHandler {
//     fn from(handler: CratesDocsHandler) -> Self {
//         handler.to_mcp_server_handler()
//     }
// }

/// 核心处理器实现（提供更多控制）
pub struct CratesDocsHandlerCore {
    server: Arc<CratesDocsServer>,
}

impl CratesDocsHandlerCore {
    /// 创建新的核心处理器
    #[must_use] 
    pub fn new(server: Arc<CratesDocsServer>) -> Self {
        Self { server }
    }
}

#[async_trait]
impl ServerHandlerCore for CratesDocsHandlerCore {
    /// 处理请求
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
                let result = self.server.tool_registry()
                    .execute_tool(&params.name, params.arguments.map_or_else(|| serde_json::Value::Null, serde_json::Value::Object))
                    .await
                    .map_err(|_e| CallToolError::unknown_tool(params.name.clone()))?;
                Ok(result.into())
            }
            RequestFromClient::ListResourcesRequest(_params) => {
                Ok(ListResourcesResult {
                    resources: vec![],
                    meta: None,
                    next_cursor: None,
                }
                .into())
            }
            RequestFromClient::ReadResourceRequest(_params) => {
                Err(RpcError::invalid_request()
                    .with_message("资源未找到".to_string()))
            }
            RequestFromClient::ListPromptsRequest(_params) => {
                Ok(ListPromptsResult {
                    prompts: vec![],
                    meta: None,
                    next_cursor: None,
                }
                .into())
            }
            RequestFromClient::GetPromptRequest(_params) => {
                Err(RpcError::invalid_request()
                    .with_message("提示未找到".to_string()))
            }
            RequestFromClient::InitializeRequest(_params) => {
                // 使用默认初始化处理
                Err(RpcError::method_not_found()
                    .with_message("初始化请求应由运行时处理".to_string()))
            }
            _ => {
                // 其他请求使用默认处理
                Err(RpcError::method_not_found()
                    .with_message("未实现的请求类型".to_string()))
            }
        }
    }
    
    /// 处理通知
    async fn handle_notification(
        &self,
        _notification: NotificationFromClient,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        // 当前不处理通知
        Ok(())
    }
    
    /// 处理错误
    async fn handle_error(
        &self,
        _error: &RpcError,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        // 记录错误但不中断
        tracing::error!("MCP 错误: {:?}", _error);
        Ok(())
    }
}