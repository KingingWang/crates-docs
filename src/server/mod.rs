//! Server module
//!
//! Provides MCP server implementation with support for multiple transport protocols.

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

/// Server configuration
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ServerConfig {
    /// Server name
    pub name: String,

    /// Server version
    pub version: String,

    /// Server description
    pub description: Option<String>,

    /// Server icons
    pub icons: Vec<Icon>,

    /// Website URL
    pub website_url: Option<String>,

    /// Host address
    pub host: String,

    /// Port
    pub port: u16,

    /// Transport mode
    pub transport_mode: String,

    /// Enable SSE support
    pub enable_sse: bool,

    /// Enable OAuth authentication
    pub enable_oauth: bool,

    /// Maximum concurrent connections
    pub max_connections: usize,

    /// Request timeout (seconds)
    pub request_timeout_secs: u64,

    /// Response timeout (seconds)
    pub response_timeout_secs: u64,

    /// Cache configuration
    pub cache: crate::cache::CacheConfig,

    /// OAuth configuration
    pub oauth: crate::server::auth::OAuthConfig,

    /// Logging configuration
    pub logging: crate::config::LoggingConfig,

    /// Performance configuration
    pub performance: crate::config::PerformanceConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "crates-docs".to_string(),
            version: crate::VERSION.to_string(),
            description: Some("High-performance Rust crate documentation query MCP server".to_string()),
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

/// MCP server
#[derive(Clone)]
pub struct CratesDocsServer {
    config: ServerConfig,
    tool_registry: Arc<ToolRegistry>,
    cache: Arc<dyn Cache>,
}

impl CratesDocsServer {
    /// Create a new server instance (synchronous)
    ///
    /// Note: This method only supports memory cache. For Redis, use the `new_async` method.
    pub fn new(config: ServerConfig) -> Result<Self> {
        let cache_box: Box<dyn Cache> = crate::cache::create_cache(&config.cache)?;
        let cache: Arc<dyn Cache> = Arc::from(cache_box);

        // Create document service
        let doc_service = Arc::new(crate::tools::docs::DocService::new(cache.clone()));

        // Create tool registry
        let tool_registry = Arc::new(crate::tools::create_default_registry(&doc_service));

        Ok(Self {
            config,
            tool_registry,
            cache,
        })
    }

    /// Create a new server instance (asynchronous)
    ///
    /// Supports memory cache and Redis cache (requires cache-redis feature).
    #[allow(unused_variables)]
    #[allow(clippy::unused_async)]
    pub async fn new_async(config: ServerConfig) -> Result<Self> {
        // Decide which creation method to use based on cache type and feature
        #[cfg(feature = "cache-redis")]
        {
            let cache_box: Box<dyn Cache> = crate::cache::create_cache_async(&config.cache).await?;
            let cache: Arc<dyn Cache> = Arc::from(cache_box);

            // Create document service
            let doc_service = Arc::new(crate::tools::docs::DocService::new(cache.clone()));

            // Create tool registry
            let tool_registry = Arc::new(crate::tools::create_default_registry(&doc_service));

            Ok(Self {
                config,
                tool_registry,
                cache,
            })
        }

        #[cfg(not(feature = "cache-redis"))]
        {
            // No cache-redis feature, fall back to synchronous creation
            let cache_box: Box<dyn Cache> = crate::cache::create_cache(&config.cache)?;
            let cache: Arc<dyn Cache> = Arc::from(cache_box);

            // Create document service
            let doc_service = Arc::new(crate::tools::docs::DocService::new(cache.clone()));

            // Create tool registry
            let tool_registry = Arc::new(crate::tools::create_default_registry(&doc_service));

            Ok(Self {
                config,
                tool_registry,
                cache,
            })
        }
    }

    /// Get server configuration
    #[must_use]
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Get tool registry
    #[must_use]
    pub fn tool_registry(&self) -> &Arc<ToolRegistry> {
        &self.tool_registry
    }

    /// Get cache
    #[must_use]
    pub fn cache(&self) -> &Arc<dyn Cache> {
        &self.cache
    }

    /// Get server information
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
                "Use this server to query Rust crate documentation. Supports crate lookup, crate search, and health check."
                    .to_string(),
            ),
            meta: None,
        }
    }

    /// Run Stdio server
    pub async fn run_stdio(&self) -> Result<()> {
        transport::run_stdio_server(self).await
    }

    /// Run HTTP server
    pub async fn run_http(&self) -> Result<()> {
        transport::run_http_server(self).await
    }

    /// Run SSE server
    pub async fn run_sse(&self) -> Result<()> {
        transport::run_sse_server(self).await
    }
}
