//! 传输模块
//!
//! 提供 Stdio、HTTP 和 SSE 传输支持。

use crate::error::Result;
use crate::server::CratesDocsServer;
use crate::server::handler::CratesDocsHandler;
use rust_mcp_sdk::{
    McpServer, StdioTransport, ToMcpServerHandler, TransportOptions,
    error::McpSdkError,
    event_store,
    mcp_server::{HyperServerOptions, McpServerOptions, hyper_server, server_runtime},
};
use std::sync::Arc;

/// 运行 Stdio 服务器
pub async fn run_stdio_server(server: &CratesDocsServer) -> Result<()> {
    tracing::info!("启动 Stdio MCP 服务器...");

    let server_info = server.server_info();
    let handler = CratesDocsHandler::new(Arc::new(server.clone()));

    // 创建 Stdio 传输
    let transport = StdioTransport::new(TransportOptions::default())
        .map_err(|e| crate::error::Error::Mcp(e.to_string()))?;

    // 创建 MCP 服务器
    let mcp_server: Arc<rust_mcp_sdk::mcp_server::ServerRuntime> =
        server_runtime::create_server(McpServerOptions {
            server_details: server_info,
            transport,
            handler: handler.to_mcp_server_handler(),
            task_store: None,
            client_task_store: None,
        });

    tracing::info!("Stdio MCP 服务器已启动，等待连接...");
    mcp_server
        .start()
        .await
        .map_err(|e: McpSdkError| crate::error::Error::Mcp(e.to_string()))?;

    Ok(())
}

/// 运行 HTTP 服务器（Streamable HTTP）
pub async fn run_http_server(server: &CratesDocsServer) -> Result<()> {
    let config = server.config();
    tracing::info!("启动 HTTP MCP 服务器在 {}:{}...", config.host, config.port);

    let server_info = server.server_info();
    let handler = CratesDocsHandler::new(Arc::new(server.clone()));

    // 创建 Hyper 服务器选项
    let options = HyperServerOptions {
        host: config.host.clone(),
        port: config.port,
        transport_options: Arc::new(TransportOptions::default()),
        sse_support: false, // 纯 HTTP 模式
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

    // 创建 HTTP 服务器
    let mcp_server =
        hyper_server::create_server(server_info, handler.to_mcp_server_handler(), options);

    tracing::info!(
        "HTTP MCP 服务器已启动，监听 {}:{}",
        config.host,
        config.port
    );
    mcp_server
        .start()
        .await
        .map_err(|e: McpSdkError| crate::error::Error::Mcp(e.to_string()))?;

    Ok(())
}

/// 运行 SSE 服务器（Server-Sent Events）
pub async fn run_sse_server(server: &CratesDocsServer) -> Result<()> {
    let config = server.config();
    tracing::info!("启动 SSE MCP 服务器在 {}:{}...", config.host, config.port);

    let server_info = server.server_info();
    let handler = CratesDocsHandler::new(Arc::new(server.clone()));

    // 创建 Hyper 服务器选项，启用 SSE 支持
    let options = HyperServerOptions {
        host: config.host.clone(),
        port: config.port,
        transport_options: Arc::new(TransportOptions::default()),
        sse_support: true, // 启用 SSE 支持
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

    // 创建 SSE 服务器
    let mcp_server =
        hyper_server::create_server(server_info, handler.to_mcp_server_handler(), options);

    tracing::info!("SSE MCP 服务器已启动，监听 {}:{}", config.host, config.port);
    mcp_server
        .start()
        .await
        .map_err(|e: McpSdkError| crate::error::Error::Mcp(e.to_string()))?;

    Ok(())
}

/// 运行混合服务器（同时支持 HTTP 和 SSE）
pub async fn run_hybrid_server(server: &CratesDocsServer) -> Result<()> {
    let config = server.config();
    tracing::info!("启动混合 MCP 服务器在 {}:{}...", config.host, config.port);

    let server_info = server.server_info();
    let handler = CratesDocsHandler::new(Arc::new(server.clone()));

    // 创建 Hyper 服务器选项，启用 SSE 支持
    let options = HyperServerOptions {
        host: config.host.clone(),
        port: config.port,
        transport_options: Arc::new(TransportOptions::default()),
        sse_support: true, // 启用 SSE 支持
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

    // 创建混合服务器（HTTP + SSE）
    let mcp_server =
        hyper_server::create_server(server_info, handler.to_mcp_server_handler(), options);

    tracing::info!(
        "混合 MCP 服务器已启动，监听 {}:{} (HTTP + SSE)",
        config.host,
        config.port
    );
    mcp_server
        .start()
        .await
        .map_err(|e: McpSdkError| crate::error::Error::Mcp(e.to_string()))?;

    Ok(())
}

/// 传输模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum TransportMode {
    /// Stdio 传输（用于 CLI 集成）
    Stdio,
    /// HTTP 传输（Streamable HTTP）
    Http,
    /// SSE 传输（Server-Sent Events）
    Sse,
    /// 混合模式（同时支持 HTTP 和 SSE）
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
            _ => Err(format!("未知的传输模式: {s}")),
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

/// 根据传输模式运行服务器
pub async fn run_server_with_mode(server: &CratesDocsServer, mode: TransportMode) -> Result<()> {
    match mode {
        TransportMode::Stdio => run_stdio_server(server).await,
        TransportMode::Http => run_http_server(server).await,
        TransportMode::Sse => run_sse_server(server).await,
        TransportMode::Hybrid => run_hybrid_server(server).await,
    }
}
