//! Server module
//!
//! Provides MCP server implementation with support for multiple transport protocols.

pub mod auth;
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

/// Re-export `ServerConfig` from config module for backward compatibility
pub use crate::config::ServerConfig;

/// MCP server
#[derive(Clone)]
pub struct CratesDocsServer {
    config: AppConfig,
    tool_registry: Arc<ToolRegistry>,
    cache: Arc<dyn Cache>,
}

impl CratesDocsServer {
    /// Create a new server instance (synchronous)
    ///
    /// Note: This method only supports memory cache. For Redis, use the `new_async` method.
    pub fn new(config: AppConfig) -> Result<Self> {
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
    pub async fn new_async(config: AppConfig) -> Result<Self> {
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
    pub fn config(&self) -> &AppConfig {
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
