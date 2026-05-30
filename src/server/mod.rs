//! Server module
//!
//! Provides MCP server implementation with multiple transport protocols (stdio, HTTP, SSE, Hybrid).
//!
//! # Main Components
//!
//! - `CratesDocsServer`: Main server struct
//! - `handler`: MCP request handling
//! - `transport`: Transport layer implementation
//! - `auth`: OAuth authentication support
//!
//! # Handler Design
//!
//! Single-layer architecture with all handling logic directly in `CratesDocsHandler`:
//! - `CratesDocsHandler`: Implements MCP protocol handler interface
//! - `HandlerConfig`: Configuration class, supports merge operation
//!
//! # Example
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
//!     // Create handler with merged config
//!     let base_config = HandlerConfig::default();
//!     let override_config = HandlerConfig::new().with_verbose_logging();
//!     let handler = CratesDocsHandler::with_merged_config(
//!         server,
//!         base_config,
//!         Some(override_config)
//!     );
//!
//!     // Run HTTP server
//!     let http_config = crates_docs::server::transport::HyperServerConfig::http();
//!     crates_docs::server::transport::run_hyper_server(&handler.server(), http_config).await?;
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

/// Re-export `ServerConfig` from config module for backward compatibility
pub use crate::config::ServerConfig;

/// Re-export `CratesDocsHandler` from handler module
pub use handler::CratesDocsHandler;

/// Re-export `HyperServerConfig` from transport module
pub use transport::HyperServerConfig;

/// Crates Docs MCP Server
///
/// Main server struct, managing configuration, tool registry, and cache.
/// Supports multiple transport protocols: stdio, HTTP, SSE, Hybrid.
///
/// # Fields
///
/// - `config`: Application configuration
/// - `tool_registry`: Tool registry
/// - `cache`: Cache instance
#[derive(Clone)]
pub struct CratesDocsServer {
    config: AppConfig,
    tool_registry: Arc<ToolRegistry>,
    cache: Arc<dyn Cache>,
}

impl CratesDocsServer {
    /// Create server from components (internal initialization logic)
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration
    /// * `cache` - Cache instance
    ///
    /// # Errors
    ///
    /// Returns error if document service creation fails
    fn from_parts(config: AppConfig, cache: Arc<dyn Cache>) -> crate::error::Result<Self> {
        // Initialize global HTTP client with performance config for connection pool reuse
        // This ensures all HTTP requests share the same connection pool
        // Note: init_global_http_client will fail if already initialized, which is fine
        let _ = crate::utils::init_global_http_client(&config.performance);

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

    /// Create new server instance (synchronous)
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration
    ///
    /// # Errors
    ///
    /// Returns error if cache creation fails
    ///
    /// # Note
    ///
    /// This method only supports memory cache. For Redis, use [`new_async`](Self::new_async).
    ///
    /// # Example
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

    /// Create new server instance (async)
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration
    ///
    /// # Errors
    ///
    /// Returns error if cache creation fails
    ///
    /// # Note
    ///
    /// Supports memory cache and Redis cache (requires `cache-redis` feature).
    ///
    /// # Example
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

    /// Get cache instance
    #[must_use]
    pub fn cache(&self) -> &Arc<dyn Cache> {
        &self.cache
    }

    /// Get server info
    ///
    /// Returns MCP initialization result with server metadata and capabilities
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
                "Use this server to query Rust crate documentation. Supports crate lookup, item lookup (functions, structs, traits, etc.), crate search, and health check."
                .to_string(),
            ),
            meta: None,
        }
    }

    /// Run Stdio server
    ///
    /// # Errors
    ///
    /// Returns error if server startup fails
    pub async fn run_stdio(&self) -> Result<()> {
        transport::run_stdio_server(self).await
    }

    /// Run HTTP server
    ///
    /// # Errors
    ///
    /// Returns error if server startup fails
    pub async fn run_http(&self) -> Result<()> {
        transport::run_hyper_server(self, transport::HyperServerConfig::http()).await
    }

    /// Run SSE server
    ///
    /// # Errors
    ///
    /// Returns error if server startup fails
    pub async fn run_sse(&self) -> Result<()> {
        transport::run_hyper_server(self, transport::HyperServerConfig::sse()).await
    }
}
