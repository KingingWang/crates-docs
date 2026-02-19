//! MCP tools module
//!
//! Provides MCP tools for Rust crate documentation queries.

pub mod docs;
pub mod health;

use async_trait::async_trait;
use rust_mcp_sdk::schema::{CallToolError, CallToolResult, Tool as McpTool};
use std::sync::Arc;

/// Tool trait
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get tool definition
    fn definition(&self) -> McpTool;

    /// Execute tool
    async fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> std::result::Result<CallToolResult, CallToolError>;
}

/// Tool registry
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Create a new tool registry
    #[must_use]
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Register tool
    #[must_use]
    pub fn register<T: Tool + 'static>(mut self, tool: T) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    /// Get all tool definitions
    #[must_use]
    pub fn get_tools(&self) -> Vec<McpTool> {
        self.tools.iter().map(|t| t.definition()).collect()
    }

    /// Execute tool
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

/// Create default tool registry
#[must_use]
pub fn create_default_registry(service: &Arc<docs::DocService>) -> ToolRegistry {
    ToolRegistry::new()
        .register(docs::lookup::LookupCrateToolImpl::new(service.clone()))
        .register(docs::search::SearchCratesToolImpl::new(service.clone()))
        .register(docs::lookup::LookupItemToolImpl::new(service.clone()))
        .register(health::HealthCheckToolImpl::new())
}
