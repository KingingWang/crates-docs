//! MCP 工具模块
//!
//! 提供用于 Rust crate 文档查询的 MCP 工具。
//!
//! # 工具列表
//!
//! - `docs::lookup_crate::LookupCrateToolImpl`: 查找 crate 文档
//! - `docs::search::SearchCratesToolImpl`: 搜索 crate
//! - `docs::lookup_item::LookupItemToolImpl`: 查找特定项目
//! - `health::HealthCheckToolImpl`: 健康检查
//!
//! # 示例
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

/// 工具 trait
///
/// 定义 MCP 工具的基本接口，包括获取工具定义和执行工具。
///
/// # 实现
///
/// 所有工具都需要实现此 trait 才能注册到 [`ToolRegistry`]。
#[async_trait]
pub trait Tool: Send + Sync {
    /// 获取工具定义
    ///
    /// 返回工具的元数据，包括名称、描述、参数等。
    fn definition(&self) -> McpTool;

    /// 执行工具
    ///
    /// # 参数
    ///
    /// * `arguments` - 工具参数（JSON 格式）
    ///
    /// # 返回值
    ///
    /// 返回工具执行结果或错误
    async fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> std::result::Result<CallToolResult, CallToolError>;
}

/// 工具注册表
///
/// 使用 `HashMap` 实现 O(1) 查找的工具注册表。
///
/// # 字段
///
/// - `tools`: 存储工具的字典，键为工具名称
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    /// 创建新的工具注册表
    ///
    /// # 示例
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

    /// 注册工具
    ///
    /// # 参数
    ///
    /// * `tool` - 要实现 [`Tool`] trait 的工具实例
    ///
    /// # 返回值
    ///
    /// 返回自身以支持链式调用
    ///
    /// # 示例
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

    /// 获取所有工具定义
    ///
    /// # 返回值
    ///
    /// 返回所有注册工具的元数据列表
    #[must_use]
    pub fn get_tools(&self) -> Vec<McpTool> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// 按名称执行工具
    ///
    /// # 参数
    ///
    /// * `name` - 工具名称
    /// * `arguments` - 工具参数（JSON 格式）
    ///
    /// # 返回值
    ///
    /// 返回工具执行结果，如果工具不存在返回错误
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

    /// 检查工具是否存在
    ///
    /// # 参数
    ///
    /// * `name` - 工具名称
    ///
    /// # 返回值
    ///
    /// 如果工具存在返回 `true`，否则返回 `false`
    #[must_use]
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// 获取注册工具数量
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// 检查注册表是否为空
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

/// 创建默认工具注册表
///
/// 注册所有内置工具：
/// - `lookup_crate`: 查找 crate 文档
/// - `search_crates`: 搜索 crate
/// - `lookup_item`: 查找特定项目
/// - `health_check`: 健康检查
///
/// # 参数
///
/// * `service` - 文档服务实例
///
/// # 示例
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
