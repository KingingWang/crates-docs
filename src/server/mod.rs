//! 服务器模块
//!
//! 提供 MCP 服务器实现，支持多种传输协议（stdio、HTTP、SSE、Hybrid）。
//!
//! # 主要组件
//!
//! - `CratesDocsServer`: 主服务器结构体
//! - `handler`: MCP 请求处理
//! - `transport`: 传输层实现
//! - `auth`: OAuth 认证支持
//!
//! # Handler 设计模式
//!
//! 使用组合模式消除代码重复：
//! - `HandlerCore`: 封装共享核心处理逻辑
//! - `CratesDocsHandler`: 标准 MCP 处理器（委托给 `HandlerCore`）
//! - `CratesDocsHandlerCore`: 核心处理器（委托给 `HandlerCore`）
//! - `HandlerConfig`: 配置类，支持 merge 操作
//!
//! # 示例
//!
//! ```rust,no_run
//! use crates_docs::{AppConfig, CratesDocsServer};
//! use crates_docs::server::handler::{CratesDocsHandler, HandlerConfig};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = AppConfig::default();
//!     let server = Arc::new(CratesDocsServer::new(config)?);
//!     
//!     // 使用 merge 配置创建 handler
//!     let base_config = HandlerConfig::default();
//!     let override_config = HandlerConfig::new().with_verbose_logging();
//!     let handler = CratesDocsHandler::with_merged_config(
//!         server,
//!         base_config,
//!         Some(override_config)
//!     );
//!     
//!     // 运行 HTTP 服务器
//!     crates_docs::server::transport::run_http_server(&handler.server()).await?;
//!
//!     Ok(())
//! }
//! ```

pub mod auth;
pub mod auth_middleware;
pub mod handler;
pub mod transport;

use crate::cache::Cache;
use crate::config::AppConfig;
use crate::error::Result;
use crate::tools::ToolRegistry;
use rust_mcp_sdk::schema::{
    Implementation, InitializeResult, ProtocolVersion, ServerCapabilities, ServerCapabilitiesTools,
};
use std::sync::Arc;

/// 从配置模块重新导出 `ServerConfig` 以保持向后兼容
pub use crate::config::ServerConfig;

/// 从 handler 模块重新导出 `CratesDocsHandler`
pub use handler::CratesDocsHandler;

/// Crates Docs MCP 服务器
///
/// 主服务器结构体，管理配置、工具注册表和缓存。
/// 支持多种传输协议：stdio、HTTP、SSE、Hybrid。
///
/// # 字段
///
/// - `config`: 应用配置
/// - `tool_registry`: 工具注册表
/// - `cache`: 缓存实例
#[derive(Clone)]
pub struct CratesDocsServer {
    config: AppConfig,
    tool_registry: Arc<ToolRegistry>,
    cache: Arc<dyn Cache>,
}

impl CratesDocsServer {
    /// 从组件创建服务器（内部初始化逻辑）
    ///
    /// # 参数
    ///
    /// * `config` - 应用配置
    /// * `cache` - 缓存实例
    ///
    /// # 错误
    ///
    /// 如果文档服务创建失败，返回错误
    fn from_parts(config: AppConfig, cache: Arc<dyn Cache>) -> crate::error::Result<Self> {
        // Create document service with cache configuration
        let doc_service = Arc::new(crate::tools::docs::DocService::with_config(
            cache.clone(),
            &config.cache,
        )?);

        // Create tool registry
        let tool_registry = Arc::new(crate::tools::create_default_registry(&doc_service));

        Ok(Self {
            config,
            tool_registry,
            cache,
        })
    }

    /// 创建新的服务器实例（同步）
    ///
    /// # 参数
    ///
    /// * `config` - 应用配置
    ///
    /// # 错误
    ///
    /// 如果缓存创建失败，返回错误
    ///
    /// # 注意
    ///
    /// 此方法仅支持内存缓存。如需使用 Redis，请使用 [`new_async`](Self::new_async) 方法。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use crates_docs::{AppConfig, CratesDocsServer};
    ///
    /// let config = AppConfig::default();
    /// let server = CratesDocsServer::new(config).expect("Failed to create server");
    /// ```
    pub fn new(config: AppConfig) -> Result<Self> {
        let cache_box: Box<dyn Cache> = crate::cache::create_cache(&config.cache)?;
        let cache: Arc<dyn Cache> = Arc::from(cache_box);
        Self::from_parts(config, cache)
    }

    /// 创建新的服务器实例（异步）
    ///
    /// # 参数
    ///
    /// * `config` - 应用配置
    ///
    /// # 错误
    ///
    /// 如果缓存创建失败，返回错误
    ///
    /// # 注意
    ///
    /// 支持内存缓存和 Redis 缓存（需要启用 `cache-redis` feature）。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use crates_docs::{AppConfig, CratesDocsServer};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = AppConfig::default();
    ///     let server = CratesDocsServer::new_async(config).await?;
    ///     Ok(())
    /// }
    /// ```
    #[allow(unused_variables)]
    #[allow(clippy::unused_async)]
    pub async fn new_async(config: AppConfig) -> Result<Self> {
        // Decide which creation method to use based on cache type and feature
        #[cfg(feature = "cache-redis")]
        {
            let cache_box: Box<dyn Cache> = crate::cache::create_cache_async(&config.cache).await?;
            let cache: Arc<dyn Cache> = Arc::from(cache_box);
            Self::from_parts(config, cache)
        }

        #[cfg(not(feature = "cache-redis"))]
        {
            // No cache-redis feature, fall back to synchronous creation
            let cache_box: Box<dyn Cache> = crate::cache::create_cache(&config.cache)?;
            let cache: Arc<dyn Cache> = Arc::from(cache_box);
            Self::from_parts(config, cache)
        }
    }

    /// 获取服务器配置
    #[must_use]
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// 获取工具注册表
    #[must_use]
    pub fn tool_registry(&self) -> &Arc<ToolRegistry> {
        &self.tool_registry
    }

    /// 获取缓存实例
    #[must_use]
    pub fn cache(&self) -> &Arc<dyn Cache> {
        &self.cache
    }

    /// 获取服务器信息
    ///
    /// 返回 MCP 初始化结果，包含服务器元数据和能力信息
    #[must_use]
    pub fn server_info(&self) -> InitializeResult {
        InitializeResult {
            server_info: Implementation {
                name: self.config.server.name.clone(),
                version: self.config.server.version.clone(),
                title: Some("Crates Docs MCP Server".to_string()),
                description: self.config.server.description.clone(),
                icons: self.config.server.icons.clone(),
                website_url: self.config.server.website_url.clone(),
            },
            capabilities: ServerCapabilities {
                tools: Some(ServerCapabilitiesTools { list_changed: None }),
                resources: None,
                prompts: None,
                experimental: None,
                completions: None,
                logging: None,
                tasks: None,
            },
            protocol_version: ProtocolVersion::V2025_11_25.into(),
            instructions: Some(
                "Use this server to query Rust crate documentation. Supports crate lookup, crate search, and health check."
                    .to_string(),
            ),
            meta: None,
        }
    }

    /// 运行 Stdio 服务器
    ///
    /// # 错误
    ///
    /// 如果服务器启动失败，返回错误
    pub async fn run_stdio(&self) -> Result<()> {
        transport::run_stdio_server(self).await
    }

    /// 运行 HTTP 服务器
    ///
    /// # 错误
    ///
    /// 如果服务器启动失败，返回错误
    pub async fn run_http(&self) -> Result<()> {
        transport::run_http_server(self).await
    }

    /// 运行 SSE 服务器
    ///
    /// # 错误
    ///
    /// 如果服务器启动失败，返回错误
    pub async fn run_sse(&self) -> Result<()> {
        transport::run_sse_server(self).await
    }
}
