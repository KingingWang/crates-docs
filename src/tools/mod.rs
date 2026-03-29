//! MCP tool module
//!
//! Provides MCP tools for Rust crate documentation lookup.
//!
//! # Tool List
//!
//! - `docs::lookup_crate::LookupCrateToolImpl`: Lookup crate documentation
//! - `docs::search::SearchCratesToolImpl`: Search crates
//! - `docs::lookup_item::LookupItemToolImpl`: Lookup specific items
//! - `health::HealthCheckToolImpl`: Health check
//!
//! # Examples
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use crates_docs::tools::{ToolRegistry, create_default_registry};
//! use crates_docs::tools::docs::DocService;
//! use crates_docs::cache::memory::MemoryCache;
//!
//! let cache = Arc::new(MemoryCache::new(1000));
//! let doc_service = Arc::new(DocService::new(cache).unwrap());
//! let registry = create_default_registry(&doc_service);
//! ```

pub mod docs;
pub mod health;

use async_trait::async_trait;
use rust_mcp_sdk::schema::{CallToolError, CallToolResult, Tool as McpTool};
use std::collections::HashMap;
use std::sync::Arc;

/// Tool trait
///
/// Defines the basic interface for MCP tools, including getting tool definition and executing the tool.
///
/// # Implementations
///
/// All tools need to implement this trait to be registered with [`ToolRegistry`].
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get tool definition
    ///
    /// Returns the tool's metadata, including name, description, parameters, etc.
    fn definition(&self) -> McpTool;

    /// Execute tool
    ///
    /// # Arguments
    ///
    /// * `arguments` - Tool arguments (JSON format)
    ///
    /// # Returns
    ///
    /// Returns tool execution result or error
    async fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> std::result::Result<CallToolResult, CallToolError>;
}

/// Tool registry
///
/// A tool registry using `HashMap` for O(1) lookup.
///
/// # Fields
///
/// - `tools`: Dictionary storing tools, keyed by tool name
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Create a new tool registry
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crates_docs::tools::ToolRegistry;
    ///
    /// let registry = ToolRegistry::new();
    /// assert!(registry.is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool
    ///
    /// # Arguments
    ///
    /// * `tool` - Tool instance implementing [`Tool`] trait
    ///
    /// # Returns
    ///
    /// Returns self for chaining
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use crates_docs::tools::ToolRegistry;
    /// use crates_docs::tools::health::HealthCheckToolImpl;
    ///
    /// let registry = ToolRegistry::new()
    ///     .register(HealthCheckToolImpl::new());
    /// ```
    #[must_use]
    pub fn register<T: Tool + 'static>(mut self, tool: T) -> Self {
        let boxed_tool: Box<dyn Tool> = Box::new(tool);
        let name = boxed_tool.definition().name.clone();
        self.tools.insert(name, boxed_tool);
        self
    }

    /// Get all tool definitions
    ///
    /// # Returns
    ///
    /// Returns a list of metadata for all registered tools
    #[must_use]
    pub fn get_tools(&self) -> Vec<McpTool> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Execute tool by name
    ///
    /// # Arguments
    ///
    /// * `name` - Tool name
    /// * `arguments` - Tool arguments (JSON format)
    ///
    /// # Returns
    ///
    /// Returns tool execution result, or error if tool not found
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

    /// Check if tool exists
    ///
    /// # Arguments
    ///
    /// * `name` - Tool name
    ///
    /// # Returns
    ///
    /// Returns `true` if tool exists, `false` otherwise
    #[must_use]
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get number of registered tools
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if registry is empty
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
///
/// Registers all built-in tools:
/// - `lookup_crate`: Lookup crate documentation
/// - `search_crates`: Search crates
/// - `lookup_item`: Lookup specific items
/// - `health_check`: Health check
///
/// # Arguments
///
/// * `service` - Document service instance
///
/// # Examples
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use crates_docs::tools::create_default_registry;
/// use crates_docs::tools::docs::DocService;
/// use crates_docs::cache::memory::MemoryCache;
///
/// let cache = Arc::new(MemoryCache::new(1000));
/// let doc_service = Arc::new(DocService::new(cache).unwrap());
/// let registry = create_default_registry(&doc_service);
/// ```
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
