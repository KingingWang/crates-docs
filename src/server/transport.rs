//! Transport module
//!
//! Provides Stdio, HTTP, and SSE transport support.

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
pub async fn run_stdio_server(server: &CratesDocsServer) -> Result<()> {
    tracing::info!("Starting Stdio MCP server...");

    let server_info = server.server_info();
    let handler = CratesDocsHandler::new(Arc::new(server.clone()));

    // Create Stdio transport
    let transport = StdioTransport::new(TransportOptions::default())
        .map_err(|e| crate::error::Error::Mcp(e.to_string()))?;

    // Create MCP server
    let mcp_server: Arc<rust_mcp_sdk::mcp_server::ServerRuntime> =
        server_runtime::create_server(McpServerOptions {
            server_details: server_info,
            transport,
            handler: handler.to_mcp_server_handler(),
            task_store: None,
            client_task_store: None,
        });

    tracing::info!("Stdio MCP server started, waiting for connections...");
    mcp_server
        .start()
        .await
        .map_err(|e: McpSdkError| crate::error::Error::Mcp(e.to_string()))?;

    Ok(())
}

/// Run HTTP server (Streamable HTTP)
pub async fn run_http_server(server: &CratesDocsServer) -> Result<()> {
    let config = server.config();
    tracing::info!(
        "Starting HTTP MCP server on {}:{}...",
        config.host,
        config.port
    );

    let server_info = server.server_info();
    let handler = CratesDocsHandler::new(Arc::new(server.clone()));

    // Create Hyper server options
    let options = HyperServerOptions {
        host: config.host.clone(),
        port: config.port,
        transport_options: Arc::new(TransportOptions::default()),
        sse_support: false, // Pure HTTP mode
        event_store: Some(Arc::new(event_store::InMemoryEventStore::default())),
        task_store: None,
        client_task_store: None,
        allowed_hosts: Some(vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "0.0.0.0".to_string(),
        ]),
        allowed_origins: Some(vec!["*".to_string()]),
        ..Default::default()
    };

    // Create HTTP server
    let mcp_server =
        hyper_server::create_server(server_info, handler.to_mcp_server_handler(), options);

    tracing::info!(
        "HTTP MCP server started, listening on {}:{}",
        config.host,
        config.port
    );
    mcp_server
        .start()
        .await
        .map_err(|e: McpSdkError| crate::error::Error::Mcp(e.to_string()))?;

    Ok(())
}

/// Run SSE server (Server-Sent Events)
pub async fn run_sse_server(server: &CratesDocsServer) -> Result<()> {
    let config = server.config();
    tracing::info!(
        "Starting SSE MCP server on {}:{}...",
        config.host,
        config.port
    );

    let server_info = server.server_info();
    let handler = CratesDocsHandler::new(Arc::new(server.clone()));

    // Create Hyper server options with SSE support enabled
    let options = HyperServerOptions {
        host: config.host.clone(),
        port: config.port,
        transport_options: Arc::new(TransportOptions::default()),
        sse_support: true, // Enable SSE support
        event_store: Some(Arc::new(event_store::InMemoryEventStore::default())),
        task_store: None,
        client_task_store: None,
        allowed_hosts: Some(vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "0.0.0.0".to_string(),
        ]),
        allowed_origins: Some(vec!["*".to_string()]),
        ..Default::default()
    };

    // Create SSE server
    let mcp_server =
        hyper_server::create_server(server_info, handler.to_mcp_server_handler(), options);

    tracing::info!(
        "SSE MCP server started, listening on {}:{}",
        config.host,
        config.port
    );
    mcp_server
        .start()
        .await
        .map_err(|e: McpSdkError| crate::error::Error::Mcp(e.to_string()))?;

    Ok(())
}

/// Run hybrid server (supports both HTTP and SSE)
pub async fn run_hybrid_server(server: &CratesDocsServer) -> Result<()> {
    let config = server.config();
    tracing::info!(
        "Starting hybrid MCP server on {}:{}...",
        config.host,
        config.port
    );

    let server_info = server.server_info();
    let handler = CratesDocsHandler::new(Arc::new(server.clone()));

    // Create Hyper server options with SSE support enabled
    let options = HyperServerOptions {
        host: config.host.clone(),
        port: config.port,
        transport_options: Arc::new(TransportOptions::default()),
        sse_support: true, // Enable SSE support
        event_store: Some(Arc::new(event_store::InMemoryEventStore::default())),
        task_store: None,
        client_task_store: None,
        allowed_hosts: Some(vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "0.0.0.0".to_string(),
        ]),
        allowed_origins: Some(vec!["*".to_string()]),
        ..Default::default()
    };

    // Create hybrid server (HTTP + SSE)
    let mcp_server =
        hyper_server::create_server(server_info, handler.to_mcp_server_handler(), options);

    tracing::info!(
        "Hybrid MCP server started, listening on {}:{} (HTTP + SSE)",
        config.host,
        config.port
    );
    mcp_server
        .start()
        .await
        .map_err(|e: McpSdkError| crate::error::Error::Mcp(e.to_string()))?;

    Ok(())
}

/// Transport mode
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
