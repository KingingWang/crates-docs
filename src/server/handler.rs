//! MCP 请求处理器实现
//!
//! 提供 MCP 协议请求的处理逻辑，包括工具列表、工具调用、资源列表等。
//!
//! # 主要结构体
//!
//! - `HandlerCore`: 共享核心处理逻辑（内部使用）
//! - `CratesDocsHandler`: 标准 MCP 处理器
//! - `CratesDocsHandlerCore`: 核心处理器（提供更细粒度的控制）
//!
//! # 设计模式
//!
//! 使用组合模式消除代码重复：
//! - `HandlerCore` 封装所有共享的处理逻辑
//! - `CratesDocsHandler` 和 `CratesDocsHandlerCore` 委托给 `HandlerCore`
//! - 支持配置合并（merge）和可选的 metrics 集成

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

/// 工具执行结果（支持不同返回类型转换）
#[derive(Debug)]
pub struct ToolExecutionResult {
    /// 工具名称
    pub tool_name: String,
    /// 执行耗时
    pub duration: std::time::Duration,
    /// 是否成功
    pub success: bool,
    /// 原始结果（用于转换为不同类型）
    pub result: std::result::Result<CallToolResult, CallToolError>,
}

impl ToolExecutionResult {
    /// 转换为 CallToolResult（用于 `ServerHandler`）
    pub fn into_call_tool_result(self) -> std::result::Result<CallToolResult, CallToolError> {
        self.result
    }

    /// 转换为 ResultFromServer（用于 `ServerHandlerCore`）
    pub fn into_result_from_server(self) -> ResultFromServer {
        self.result.unwrap_or_else(CallToolResult::from).into()
    }
}

/// Handler 配置（支持合并）
///
/// 用于配置 handler 的行为，如 metrics 集成、日志级别等。
#[derive(Debug, Clone, Default)]
pub struct HandlerConfig {
    /// 是否启用详细日志
    pub verbose_logging: bool,
    /// 是否记录 metrics
    pub enable_metrics: bool,
}

impl HandlerConfig {
    /// 创建新的配置
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 启用详细日志
    #[must_use]
    pub fn with_verbose_logging(self) -> Self {
        Self {
            verbose_logging: true,
            ..self
        }
    }

    /// 启用 metrics
    #[must_use]
    pub fn with_metrics(self) -> Self {
        Self {
            enable_metrics: true,
            ..self
        }
    }

    /// 合并配置（other 优先覆盖 self）
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

/// 共享核心处理逻辑
///
/// 封装所有 MCP 请求处理的共享逻辑，消除 `CratesDocsHandler` 和
/// `CratesDocsHandlerCore` 之间的代码重复。
///
/// # 设计
///
/// - 提供工具执行、列表查询等核心方法
/// - 支持可选的 metrics 集成
/// - 支持配置合并
pub struct HandlerCore {
    server: Arc<CratesDocsServer>,
    config: HandlerConfig,
    metrics: Option<Arc<ServerMetrics>>,
}

impl HandlerCore {
    /// 创建新的核心处理器
    ///
    /// # 参数
    ///
    /// * `server` - 服务器实例
    #[must_use]
    pub fn new(server: Arc<CratesDocsServer>) -> Self {
        Self {
            server,
            config: HandlerConfig::default(),
            metrics: None,
        }
    }

    /// 使用配置创建核心处理器
    #[must_use]
    pub fn with_config(server: Arc<CratesDocsServer>, config: HandlerConfig) -> Self {
        Self {
            server,
            config,
            metrics: None,
        }
    }

    /// 合并配置创建核心处理器
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

    /// 设置 metrics
    #[must_use]
    pub fn with_metrics(self, metrics: Arc<ServerMetrics>) -> Self {
        Self {
            metrics: Some(metrics),
            ..self
        }
    }

    /// 获取服务器引用
    #[must_use]
    pub fn server(&self) -> &Arc<CratesDocsServer> {
        &self.server
    }

    /// 获取工具注册表
    #[must_use]
    pub fn tool_registry(&self) -> &ToolRegistry {
        self.server.tool_registry()
    }

    /// 获取配置
    #[must_use]
    pub fn config(&self) -> &HandlerConfig {
        &self.config
    }

    /// 获取 metrics（可选）
    #[must_use]
    pub fn metrics(&self) -> Option<&Arc<ServerMetrics>> {
        self.metrics.as_ref()
    }

    /// 获取所有工具列表
    #[must_use]
    pub fn list_tools(&self) -> ListToolsResult {
        ListToolsResult {
            tools: self.tool_registry().get_tools(),
            meta: None,
            next_cursor: None,
        }
    }

    /// 获取空资源列表
    #[must_use]
    pub fn list_resources(&self) -> ListResourcesResult {
        ListResourcesResult {
            resources: vec![],
            meta: None,
            next_cursor: None,
        }
    }

    /// 获取空提示列表
    #[must_use]
    pub fn list_prompts(&self) -> ListPromptsResult {
        ListPromptsResult {
            prompts: vec![],
            meta: None,
            next_cursor: None,
        }
    }

    /// 执行工具调用（核心逻辑）
    ///
    /// 此方法封装了工具执行的完整流程：
    /// - tracing 追踪
    /// - 计时统计
    /// - metrics 记录（如果启用）
    ///
    /// # 返回
    ///
    /// 返回 `ToolExecutionResult`，可转换为不同类型以适配不同的 trait
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

            // 记录日志
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

            // 记录 metrics（如果启用）
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

/// MCP 服务器处理器
///
/// 实现标准 MCP 协议处理器接口，处理客户端请求。
/// 委托所有核心逻辑给 `HandlerCore`。
///
/// # 字段
///
/// - `core`: 共享核心处理逻辑
pub struct CratesDocsHandler {
    core: HandlerCore,
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
        Self {
            core: HandlerCore::new(server),
        }
    }

    /// 使用配置创建处理器
    #[must_use]
    pub fn with_config(server: Arc<CratesDocsServer>, config: HandlerConfig) -> Self {
        Self {
            core: HandlerCore::with_config(server, config),
        }
    }

    /// 合并配置创建处理器
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

    /// 设置 metrics
    #[must_use]
    pub fn with_metrics(self, metrics: Arc<ServerMetrics>) -> Self {
        Self {
            core: self.core.with_metrics(metrics),
        }
    }

    /// 获取核心处理器
    #[must_use]
    pub fn core(&self) -> &HandlerCore {
        &self.core
    }

    /// 获取服务器引用
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
/// 实现更细粒度的 MCP 协议处理器接口。
/// 委托所有核心逻辑给 `HandlerCore`。
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

    /// 使用配置创建核心处理器
    #[must_use]
    pub fn with_config(server: Arc<CratesDocsServer>, config: HandlerConfig) -> Self {
        Self {
            core: HandlerCore::with_config(server, config),
        }
    }

    /// 合并配置创建核心处理器
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

    /// 设置 metrics
    #[must_use]
    pub fn with_metrics(self, metrics: Arc<ServerMetrics>) -> Self {
        Self {
            core: self.core.with_metrics(metrics),
        }
    }

    /// 获取核心处理器
    #[must_use]
    pub fn core(&self) -> &HandlerCore {
        &self.core
    }

    /// 获取服务器引用
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
        // 测试有 override 的情况
        let base = HandlerConfig::default();
        let override_config = HandlerConfig::new().with_verbose_logging().with_metrics();

        let merged = base.merge(Some(override_config));
        assert!(merged.verbose_logging);
        assert!(merged.enable_metrics);

        // 测试空 override（使用新的 base）
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

        // 验证 metrics 被记录
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
        assert_eq!(tools.tools.len(), 4); // 4 个默认工具

        let resources = core.list_resources();
        assert!(resources.resources.is_empty());

        let prompts = core.list_prompts();
        assert!(prompts.prompts.is_empty());
    }
}
