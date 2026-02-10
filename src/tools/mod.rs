//! MCP 工具模块
//!
//! 提供 Rust crate 文档查询相关的 MCP 工具。

pub mod docs;
pub mod health;

use async_trait::async_trait;
use rust_mcp_sdk::schema::{CallToolError, CallToolResult, Tool as McpTool};
use std::sync::Arc;

/// 工具 trait
#[async_trait]
pub trait Tool: Send + Sync {
    /// 获取工具定义
    fn definition(&self) -> McpTool;

    /// 执行工具
    async fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> std::result::Result<CallToolResult, CallToolError>;
}

/// 工具注册器
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    /// 创建新的工具注册器
    #[must_use]
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// 注册工具
    #[must_use]
    pub fn register<T: Tool + 'static>(mut self, tool: T) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    /// 获取所有工具定义
    #[must_use]
    pub fn get_tools(&self) -> Vec<McpTool> {
        self.tools.iter().map(|t| t.definition()).collect()
    }

    /// 执行工具
    pub async fn execute_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        for tool in &self.tools {
            if tool.definition().name == name {
                return tool.execute(arguments).await;
            }
        }

        Err(CallToolError::unknown_tool(name.to_string()))
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 创建默认工具注册器
#[must_use]
pub fn create_default_registry(service: &Arc<docs::DocService>) -> ToolRegistry {
    ToolRegistry::new()
        .register(docs::lookup::LookupCrateToolImpl::new(service.clone()))
        .register(docs::search::SearchCratesToolImpl::new(service.clone()))
        .register(docs::lookup::LookupItemToolImpl::new(service.clone()))
        .register(health::HealthCheckToolImpl::new())
}
