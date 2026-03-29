//! 传输模块
//!
//! 提供 Stdio、HTTP 和 SSE 传输协议支持。
//!
//! # 支持的传输模式
//!
//! - **Stdio**: 标准输入输出，适合与 MCP 客户端集成
//! - **HTTP**: Streamable HTTP，支持无状态请求
//! - **SSE**: Server-Sent Events，支持服务器推送
//! - **Hybrid**: 混合模式，同时支持 HTTP 和 SSE
//!
//! # 示例
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
//!     // 运行 Stdio 服务器
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

/// 运行 Stdio 服务器
///
/// 通过标准输入输出与 MCP 客户端通信。
///
/// # 参数
///
/// * `server` - `CratesDocsServer` 实例
///
/// # 错误
///
/// 如果服务器启动失败，返回错误
///
/// # 示例
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
/// * `server` - `CratesDocsServer` 实例
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

/// 运行 HTTP 服务器（Streamable HTTP）
///
/// 启动支持 Streamable HTTP 协议的 MCP 服务器。
///
/// # 参数
///
/// * `server` - `CratesDocsServer` 实例
///
/// # 错误
///
/// 如果服务器启动失败，返回错误
///
/// # 示例
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

/// 运行 SSE 服务器（Server-Sent Events）
///
/// 启动支持 Server-Sent Events 协议的 MCP 服务器。
///
/// # 参数
///
/// * `server` - `CratesDocsServer` 实例
///
/// # 错误
///
/// 如果服务器启动失败，返回错误
///
/// # 示例
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

/// 运行混合服务器（同时支持 HTTP 和 SSE）
///
/// 启动同时支持 HTTP 和 SSE 协议的 MCP 服务器。
///
/// # 参数
///
/// * `server` - `CratesDocsServer` 实例
///
/// # 错误
///
/// 如果服务器启动失败，返回错误
///
/// # 示例
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

/// 传输模式
///
/// 定义 MCP 服务器支持的传输协议类型。
///
/// # 变体
///
/// - `Stdio`: 标准输入输出，适合与 MCP 客户端集成
/// - `Http`: Streamable HTTP，支持无状态请求
/// - `Sse`: Server-Sent Events，支持服务器推送
/// - `Hybrid`: 混合模式，同时支持 HTTP 和 SSE
///
/// # 示例
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
