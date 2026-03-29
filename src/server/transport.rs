//! Transport module
//!
//! Provides Stdio, HTTP and SSE transport protocol support.
//!
//! # Supported Transport Modes
//!
//! - **Stdio**: Standard input/output, suitable for MCP client integration
//! - **HTTP**: Streamable HTTP, supports stateless requests
//! - **SSE**: Server-Sent Events, supports server push
//! - **Hybrid**: Hybrid mode, supports both HTTP and SSE
//!
//! # Example
//!
//! ```rust,no_run
//! use crates_docs::server::transport::{run_stdio_server, TransportMode};
//! use crates_docs::{AppConfig, CratesDocsServer};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = AppConfig::default();
//!     let server = CratesDocsServer::new(config)?;
//!
//!     // Run Stdio server
//!     run_stdio_server(&server).await?;
//!
//!     Ok(())
//! }
//! ```

use crate::error::Result;
use crate::server::handler::CratesDocsHandler;
use crate::server::CratesDocsServer;
use rust_mcp_sdk::{
    error::McpSdkError,
    event_store,
    mcp_server::{hyper_server, server_runtime, HyperServerOptions, McpServerOptions},
    McpServer, StdioTransport, ToMcpServerHandler, TransportOptions,
};
use std::sync::Arc;

/// Run Stdio server
///
/// Communicates with MCP clients via standard input/output.
///
/// # Arguments
///
/// * `server` - `CratesDocsServer` instance
///
/// # Errors
///
/// Returns error if server startup fails
///
/// # Example
///
/// ```rust,no_run
/// use crates_docs::server::transport::run_stdio_server;
/// use crates_docs::{AppConfig, CratesDocsServer};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = AppConfig::default();
///     let server = CratesDocsServer::new(config)?;
///     run_stdio_server(&server).await?;
///     Ok(())
/// }
/// ```
pub async fn run_stdio_server(server: &CratesDocsServer) -> Result<()> {
    tracing::info!("Starting Stdio MCP server...");

    let server_info = server.server_info();
    let handler = CratesDocsHandler::new(Arc::new(server.clone()));

    // Create Stdio transport
    let transport = StdioTransport::new(TransportOptions::default())
        .map_err(|e| crate::error::Error::mcp("transport", e.to_string()))?;

    // Create MCP server
    let mcp_server: Arc<rust_mcp_sdk::mcp_server::ServerRuntime> =
        server_runtime::create_server(McpServerOptions {
            server_details: server_info,
            transport,
            handler: handler.to_mcp_server_handler(),
            task_store: None,
            client_task_store: None,
            message_observer: None,
        });

    tracing::info!("Stdio MCP server started, waiting for connections...");
    mcp_server
        .start()
        .await
        .map_err(|e: McpSdkError| crate::error::Error::mcp("server_start", e.to_string()))?;

    Ok(())
}

/// Internal helper to run a Hyper-based MCP server with the given configuration.
///
/// This function consolidates the common logic for HTTP, SSE, and Hybrid servers,
/// which only differ in their SSE support and log messages.
///
/// # Arguments
///
/// * `server` - `CratesDocsServer` instance
/// * `protocol_name` - Protocol name for logging (e.g., "HTTP", "SSE", "Hybrid")
/// * `sse_support` - Whether SSE support is enabled
async fn run_hyper_server(
    server: &CratesDocsServer,
    protocol_name: &str,
    sse_support: bool,
) -> Result<()> {
    let config = server.config();
    let server_info = server.server_info();
    let handler = CratesDocsHandler::new(Arc::new(server.clone()));

    tracing::info!(
        "Starting {} MCP server on {}:{}...",
        protocol_name,
        config.server.host,
        config.server.port
    );

    // Create Hyper server options with security settings from config
    let options = HyperServerOptions {
        host: config.server.host.clone(),
        port: config.server.port,
        transport_options: Arc::new(TransportOptions::default()),
        sse_support,
        event_store: Some(Arc::new(event_store::InMemoryEventStore::default())),
        task_store: None,
        client_task_store: None,
        allowed_hosts: Some(config.server.allowed_hosts.clone()),
        allowed_origins: Some(config.server.allowed_origins.clone()),
        health_endpoint: Some("/health".to_string()),
        ..Default::default()
    };

    // Create HTTP/SSE/Hybrid server
    let mcp_server =
        hyper_server::create_server(server_info, handler.to_mcp_server_handler(), options);

    // Build the started message based on the protocol
    let started_msg = if sse_support && protocol_name != "SSE" {
        // Hybrid mode
        format!(
            "{} MCP server started, listening on {}:{} (HTTP + SSE)",
            protocol_name, config.server.host, config.server.port
        )
    } else {
        format!(
            "{} MCP server started, listening on {}:{}",
            protocol_name, config.server.host, config.server.port
        )
    };
    tracing::info!("{}", started_msg);

    mcp_server
        .start()
        .await
        .map_err(|e: McpSdkError| crate::error::Error::mcp("server_start", e.to_string()))?;

    Ok(())
}

/// Run HTTP server (Streamable HTTP)
///
/// Starts MCP server supporting Streamable HTTP protocol.
///
/// # Arguments
///
/// * `server` - `CratesDocsServer` instance
///
/// # Errors
///
/// Returns error if server startup fails
///
/// # Example
///
/// ```rust,no_run
/// use crates_docs::server::transport::run_http_server;
/// use crates_docs::{AppConfig, CratesDocsServer};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = AppConfig::default();
///     let server = CratesDocsServer::new(config)?;
///     run_http_server(&server).await?;
///     Ok(())
/// }
/// ```
pub async fn run_http_server(server: &CratesDocsServer) -> Result<()> {
    run_hyper_server(server, "HTTP", false).await
}

/// Run SSE server (Server-Sent Events)
///
/// Starts MCP server supporting Server-Sent Events protocol.
///
/// # Arguments
///
/// * `server` - `CratesDocsServer` instance
///
/// # Errors
///
/// Returns error if server startup fails
///
/// # Example
///
/// ```rust,no_run
/// use crates_docs::server::transport::run_sse_server;
/// use crates_docs::{AppConfig, CratesDocsServer};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = AppConfig::default();
///     let server = CratesDocsServer::new(config)?;
///     run_sse_server(&server).await?;
///     Ok(())
/// }
/// ```
pub async fn run_sse_server(server: &CratesDocsServer) -> Result<()> {
    run_hyper_server(server, "SSE", true).await
}

/// Run Hybrid server (supports both HTTP and SSE)
///
/// Starts MCP server supporting both HTTP and SSE protocols.
///
/// # Arguments
///
/// * `server` - `CratesDocsServer` instance
///
/// # Errors
///
/// Returns error if server startup fails
///
/// # Example
///
/// ```rust,no_run
/// use crates_docs::server::transport::run_hybrid_server;
/// use crates_docs::{AppConfig, CratesDocsServer};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = AppConfig::default();
///     let server = CratesDocsServer::new(config)?;
///     run_hybrid_server(&server).await?;
///     Ok(())
/// }
/// ```
pub async fn run_hybrid_server(server: &CratesDocsServer) -> Result<()> {
    run_hyper_server(server, "Hybrid", true).await
}

/// Transport mode
///
/// Defines the transport protocol types supported by MCP server.
///
/// # Variants
///
/// - `Stdio`: Standard input/output, suitable for MCP client integration
/// - `Http`: Streamable HTTP, supports stateless requests
/// - `Sse`: Server-Sent Events, supports server push
/// - `Hybrid`: Hybrid mode, supports both HTTP and SSE
///
/// # Example
///
/// ```rust
/// use crates_docs::server::transport::TransportMode;
/// use std::str::FromStr;
///
/// let mode = TransportMode::from_str("http").unwrap();
/// assert_eq!(mode, TransportMode::Http);
/// assert_eq!(mode.to_string(), "http");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum TransportMode {
    /// Stdio transport (for CLI integration)
    Stdio,
    /// HTTP transport (Streamable HTTP)
    Http,
    /// SSE transport (Server-Sent Events)
    Sse,
    /// Hybrid mode (supports both HTTP and SSE)
    Hybrid,
}

impl std::str::FromStr for TransportMode {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stdio" => Ok(TransportMode::Stdio),
            "http" => Ok(TransportMode::Http),
            "sse" => Ok(TransportMode::Sse),
            "hybrid" => Ok(TransportMode::Hybrid),
            _ => Err(format!("Unknown transport mode: {s}")),
        }
    }
}

impl std::fmt::Display for TransportMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportMode::Stdio => write!(f, "stdio"),
            TransportMode::Http => write!(f, "http"),
            TransportMode::Sse => write!(f, "sse"),
            TransportMode::Hybrid => write!(f, "hybrid"),
        }
    }
}

/// Run server with the specified transport mode
pub async fn run_server_with_mode(server: &CratesDocsServer, mode: TransportMode) -> Result<()> {
    match mode {
        TransportMode::Stdio => run_stdio_server(server).await,
        TransportMode::Http => run_http_server(server).await,
        TransportMode::Sse => run_sse_server(server).await,
        TransportMode::Hybrid => run_hybrid_server(server).await,
    }
}
