//! 服务器模块
//!
//! 提供 MCP 服务器的实现，支持多种传输协议。

pub mod auth;
pub mod handler;
pub mod transport;

use crate::cache::Cache;
use crate::error::Result;
use crate::tools::ToolRegistry;
use rust_mcp_sdk::schema::{
    Icon, IconTheme, Implementation, InitializeResult, ProtocolVersion, ServerCapabilities,
    ServerCapabilitiesTools,
};
use std::sync::Arc;

/// 服务器配置
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ServerConfig {
    /// 服务器名称
    pub name: String,

    /// 服务器版本
    pub version: String,

    /// 服务器描述
    pub description: Option<String>,

    /// 服务器图标
    pub icons: Vec<Icon>,

    /// 网站 URL
    pub website_url: Option<String>,

    /// 主机地址
    pub host: String,

    /// 端口
    pub port: u16,

    /// 传输模式
    pub transport_mode: String,

    /// 启用 SSE 支持
    pub enable_sse: bool,

    /// 启用 OAuth 认证
    pub enable_oauth: bool,

    /// 最大并发连接数
    pub max_connections: usize,

    /// 请求超时时间（秒）
    pub request_timeout_secs: u64,

    /// 响应超时时间（秒）
    pub response_timeout_secs: u64,

    /// 缓存配置
    pub cache: crate::cache::CacheConfig,

    /// OAuth 配置
    pub oauth: crate::server::auth::OAuthConfig,

    /// 日志配置
    pub logging: crate::config::LoggingConfig,

    /// 性能配置
    pub performance: crate::config::PerformanceConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "crates-docs".to_string(),
            version: crate::VERSION.to_string(),
            description: Some("高性能 Rust crate 文档查询 MCP 服务器".to_string()),
            icons: vec![
                Icon {
                    src: "https://docs.rs/static/favicon-32x32.png".to_string(),
                    mime_type: Some("image/png".to_string()),
                    sizes: vec!["32x32".to_string()],
                    theme: Some(IconTheme::Light),
                },
                Icon {
                    src: "https://docs.rs/static/favicon-32x32.png".to_string(),
                    mime_type: Some("image/png".to_string()),
                    sizes: vec!["32x32".to_string()],
                    theme: Some(IconTheme::Dark),
                },
            ],
            website_url: Some("https://github.com/KingingWang/crates-docs".to_string()),
            host: "127.0.0.1".to_string(),
            port: 8080,
            transport_mode: "hybrid".to_string(),
            enable_sse: true,
            enable_oauth: false,
            max_connections: 100,
            request_timeout_secs: 30,
            response_timeout_secs: 60,
            cache: crate::cache::CacheConfig::default(),
            oauth: crate::server::auth::OAuthConfig::default(),
            logging: crate::config::LoggingConfig::default(),
            performance: crate::config::PerformanceConfig::default(),
        }
    }
}

/// MCP 服务器
#[derive(Clone)]
pub struct CratesDocsServer {
    config: ServerConfig,
    tool_registry: Arc<ToolRegistry>,
    cache: Arc<dyn Cache>,
}

impl CratesDocsServer {
    /// 创建新的服务器实例（同步）
    ///
    /// 注意：此方法只支持内存缓存。如需使用 Redis，请使用 `new_async` 方法。
    pub fn new(config: ServerConfig) -> Result<Self> {
        let cache_box: Box<dyn Cache> = crate::cache::create_cache(&config.cache)?;
        let cache: Arc<dyn Cache> = Arc::from(cache_box);

        // 创建文档服务
        let doc_service = Arc::new(crate::tools::docs::DocService::new(cache.clone()));

        // 创建工具注册器
        let tool_registry = Arc::new(crate::tools::create_default_registry(&doc_service));

        Ok(Self {
            config,
            tool_registry,
            cache,
        })
    }

    /// 创建新的服务器实例（异步）
    ///
    /// 支持内存缓存和 Redis 缓存（需要 cache-redis feature）。
    #[allow(unused_variables)]
    pub async fn new_async(config: ServerConfig) -> Result<Self> {
        // 根据缓存类型和 feature 决定使用哪种创建方法
        #[cfg(feature = "cache-redis")]
        {
            let cache_box: Box<dyn Cache> =
                crate::cache::create_cache_async(&config.cache).await?;
            let cache: Arc<dyn Cache> = Arc::from(cache_box);

            // 创建文档服务
            let doc_service = Arc::new(crate::tools::docs::DocService::new(cache.clone()));

            // 创建工具注册器
            let tool_registry = Arc::new(crate::tools::create_default_registry(&doc_service));

            Ok(Self {
                config,
                tool_registry,
                cache,
            })
        }

        #[cfg(not(feature = "cache-redis"))]
        {
            // 没有 cache-redis feature，回退到同步创建
            let cache_box: Box<dyn Cache> = crate::cache::create_cache(&config.cache)?;
            let cache: Arc<dyn Cache> = Arc::from(cache_box);

            // 创建文档服务
            let doc_service = Arc::new(crate::tools::docs::DocService::new(cache.clone()));

            // 创建工具注册器
            let tool_registry = Arc::new(crate::tools::create_default_registry(&doc_service));

            Ok(Self {
                config,
                tool_registry,
                cache,
            })
        }
    }

    /// 获取服务器配置
    #[must_use]
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// 获取工具注册器
    #[must_use]
    pub fn tool_registry(&self) -> &Arc<ToolRegistry> {
        &self.tool_registry
    }

    /// 获取缓存
    #[must_use]
    pub fn cache(&self) -> &Arc<dyn Cache> {
        &self.cache
    }

    /// 获取服务器信息
    #[must_use]
    pub fn server_info(&self) -> InitializeResult {
        InitializeResult {
            server_info: Implementation {
                name: self.config.name.clone(),
                version: self.config.version.clone(),
                title: Some("Crates Docs MCP Server".to_string()),
                description: self.config.description.clone(),
                icons: self.config.icons.clone(),
                website_url: self.config.website_url.clone(),
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
                "使用此服务器查询 Rust crate 文档。支持查找 crate、搜索 crate 和健康检查。"
                    .to_string(),
            ),
            meta: None,
        }
    }

    /// 运行 Stdio 服务器
    pub async fn run_stdio(&self) -> Result<()> {
        transport::run_stdio_server(self).await
    }

    /// 运行 HTTP 服务器
    pub async fn run_http(&self) -> Result<()> {
        transport::run_http_server(self).await
    }

    /// 运行 SSE 服务器
    pub async fn run_sse(&self) -> Result<()> {
        transport::run_sse_server(self).await
    }
}
