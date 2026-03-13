//! MCP tools module
//!
//! Provides MCP tools for Rust crate documentation queries.

pub mod docs;
pub mod health;

use async_trait::async_trait;
use rust_mcp_sdk::schema::{CallToolError, CallToolResult, Tool as McpTool};
use std::collections::HashMap;
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

/// Tool registry using `HashMap` for O(1) lookup
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Create a new tool registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register tool
    #[must_use]
    pub fn register<T: Tool + 'static>(mut self, tool: T) -> Self {
        let boxed_tool: Box<dyn Tool> = Box::new(tool);
        let name = boxed_tool.definition().name.clone();
        self.tools.insert(name, boxed_tool);
        self
    }

    /// Get all tool definitions
    #[must_use]
    pub fn get_tools(&self) -> Vec<McpTool> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Execute tool by name
    pub async fn execute_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        match self.tools.get(name) {
            Some(tool) => tool.execute(arguments).await,
            None => Err(CallToolError::unknown_tool(name.to_string())),
        }
    }

    /// Check if a tool exists
    #[must_use]
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get the number of registered tools
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
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
        .register(docs::lookup_crate::LookupCrateToolImpl::new(
            service.clone(),
        ))
        .register(docs::search::SearchCratesToolImpl::new(service.clone()))
        .register(docs::lookup_item::LookupItemToolImpl::new(service.clone()))
        .register(health::HealthCheckToolImpl::new())
}
