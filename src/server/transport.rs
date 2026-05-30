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

/// Hyper server configuration
///
/// Configuration for HTTP/SSE/Hybrid MCP servers using the Builder pattern.
///
/// # Example
///
/// ```rust
/// use crates_docs::server::transport::HyperServerConfig;
///
/// let http_config = HyperServerConfig::http();
/// let sse_config = HyperServerConfig::sse();
/// let hybrid_config = HyperServerConfig::hybrid();
/// ```
#[derive(Debug, Clone)]
pub struct HyperServerConfig {
    /// Protocol name for logging (e.g., "HTTP", "SSE", "Hybrid")
    protocol_name: String,
    /// Whether SSE support is enabled
    sse_support: bool,
}

impl HyperServerConfig {
    /// Create HTTP server configuration
    ///
    /// HTTP mode supports Streamable HTTP protocol for stateless requests.
    #[must_use]
    pub fn http() -> Self {
        Self {
            protocol_name: "HTTP".to_string(),
            sse_support: false,
        }
    }

    /// Create SSE server configuration
    ///
    /// SSE mode supports Server-Sent Events for server push capabilities.
    #[must_use]
    pub fn sse() -> Self {
        Self {
            protocol_name: "SSE".to_string(),
            sse_support: true,
        }
    }

    /// Create Hybrid server configuration
    ///
    /// Hybrid mode supports both HTTP and SSE protocols.
    #[must_use]
    pub fn hybrid() -> Self {
        Self {
            protocol_name: "Hybrid".to_string(),
            sse_support: true,
        }
    }

    /// Get protocol name
    #[must_use]
    pub fn protocol_name(&self) -> &str {
        &self.protocol_name
    }

    /// Check if SSE support is enabled
    #[must_use]
    pub fn sse_support(&self) -> bool {
        self.sse_support
    }
}

/// Emit a prominent warning when authentication is configured but cannot be
/// enforced on the HTTP/SSE transport.
///
/// The in-tree `auth_middleware` is not wired into the SDK's Hyper request
/// pipeline, so API key / OAuth settings do NOT protect HTTP endpoints today.
/// Failing silently here would give operators a dangerous false sense of
/// security, so we log a loud warning instead.
fn warn_if_auth_configured_but_unenforced(server_config: &crate::config::AppConfig) {
    let mut configured = Vec::new();

    #[cfg(feature = "api-key")]
    if server_config.auth.api_key.enabled {
        configured.push("API key");
    }
    if server_config.auth.oauth.enabled || server_config.oauth.enabled {
        configured.push("OAuth");
    }

    if !configured.is_empty() {
        tracing::warn!(
            "{auth} authentication is enabled in configuration but is NOT enforced on the HTTP/SSE transport: requests to this server are currently unauthenticated. Do not expose this server on an untrusted network. Restrict access via allowed_hosts/allowed_origins, a reverse proxy, or run in stdio mode.",
            auth = configured.join(" + ")
        );
    }
}

/// Warn when Prometheus metrics are requested in configuration but the server
/// neither collects nor exposes them.
///
/// The metrics subsystem (`ServerMetrics`, `performance.metrics_port`) is not
/// currently wired into the request pipeline and no metrics endpoint is served,
/// so `enable_metrics = true` has no observable effect. Surfacing this avoids
/// misleading operators into believing a scrape target exists.
fn warn_if_metrics_configured_but_unavailable(server_config: &crate::config::AppConfig) {
    if server_config.performance.enable_metrics {
        tracing::warn!(
            metrics_port = server_config.performance.metrics_port,
            "performance.enable_metrics is set, but this server does not yet collect or expose \
             Prometheus metrics: no metrics endpoint is served and no request metrics are recorded. \
             This setting currently has no effect."
        );
    }
}

/// Warn when server resource limits are configured but not enforced.
///
/// `request_timeout_secs`, `response_timeout_secs`, and `max_connections` are
/// accepted in configuration, but the underlying SDK `HyperServerOptions` does
/// not expose request/response timeouts or a connection cap, so these values
/// are never applied. Warning when an operator sets a non-default value avoids
/// a false sense that the server enforces limits it does not.
fn unenforced_server_limits(server_config: &crate::config::AppConfig) -> Vec<&'static str> {
    let defaults = crate::config::ServerConfig::default();
    let mut unenforced = Vec::new();
    if server_config.server.request_timeout_secs != defaults.request_timeout_secs {
        unenforced.push("request_timeout_secs");
    }
    if server_config.server.response_timeout_secs != defaults.response_timeout_secs {
        unenforced.push("response_timeout_secs");
    }
    if server_config.server.max_connections != defaults.max_connections {
        unenforced.push("max_connections");
    }
    unenforced
}

fn warn_if_unenforced_server_limits_configured(server_config: &crate::config::AppConfig) {
    let unenforced = unenforced_server_limits(server_config);
    if !unenforced.is_empty() {
        tracing::warn!(
            fields = unenforced.join(", "),
            "These server limit settings are configured with non-default values but are NOT \
             enforced: the HTTP transport applies neither request/response timeouts nor a maximum \
             connection cap. These settings currently have no effect."
        );
    }
}

/// Run a Hyper-based MCP server with the given configuration.
///
/// This function handles HTTP, SSE, and Hybrid servers based on the configuration.
///
/// # Arguments
///
/// * `server` - `CratesDocsServer` instance
/// * `config` - `HyperServerConfig` instance
///
/// # Errors
///
/// Returns error if server startup fails
///
/// # Example
///
/// ```rust,no_run
/// use crates_docs::server::transport::{run_hyper_server, HyperServerConfig};
/// use crates_docs::{AppConfig, CratesDocsServer};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = AppConfig::default();
///     let server = CratesDocsServer::new(config)?;
///     let http_config = HyperServerConfig::http();
///     run_hyper_server(&server, http_config).await?;
///     Ok(())
/// }
/// ```
pub async fn run_hyper_server(server: &CratesDocsServer, config: HyperServerConfig) -> Result<()> {
    let server_config = server.config();
    let server_info = server.server_info();
    let handler = CratesDocsHandler::new(Arc::new(server.clone()));

    tracing::info!(
        "Starting {} MCP server on {}:{}...",
        config.protocol_name(),
        server_config.server.host,
        server_config.server.port
    );

    warn_if_auth_configured_but_unenforced(server_config);
    warn_if_metrics_configured_but_unavailable(server_config);
    warn_if_unenforced_server_limits_configured(server_config);

    // Create Hyper server options with security settings from config
    let options = HyperServerOptions {
        host: server_config.server.host.clone(),
        port: server_config.server.port,
        transport_options: Arc::new(TransportOptions::default()),
        sse_support: config.sse_support(),
        event_store: Some(Arc::new(event_store::InMemoryEventStore::default())),
        task_store: None,
        client_task_store: None,
        allowed_hosts: Some(server_config.server.allowed_hosts.clone()),
        allowed_origins: Some(server_config.server.allowed_origins.clone()),
        // Without this flag the SDK never installs the DnsRebindProtector, so
        // the allowlists above would be silently ignored. Honor the operator's
        // explicit opt-in instead.
        dns_rebinding_protection: server_config.server.dns_rebinding_protection,
        health_endpoint: Some("/health".to_string()),
        ..Default::default()
    };

    if server_config.server.dns_rebinding_protection
        && server_config.server.allowed_hosts.is_empty()
        && server_config.server.allowed_origins.is_empty()
    {
        tracing::warn!(
            "dns_rebinding_protection is enabled but both allowed_hosts and              allowed_origins are empty; no Host/Origin validation will occur"
        );
    }

    // Create HTTP/SSE/Hybrid server
    let mcp_server =
        hyper_server::create_server(server_info, handler.to_mcp_server_handler(), options);

    // Build the started message based on the protocol
    let started_msg = if config.sse_support() && config.protocol_name() != "SSE" {
        // Hybrid mode
        format!(
            "{} MCP server started, listening on {}:{} (HTTP + SSE)",
            config.protocol_name(),
            server_config.server.host,
            server_config.server.port
        )
    } else {
        format!(
            "{} MCP server started, listening on {}:{}",
            config.protocol_name(),
            server_config.server.host,
            server_config.server.port
        )
    };
    tracing::info!("{}", started_msg);

    mcp_server
        .start()
        .await
        .map_err(|e: McpSdkError| crate::error::Error::mcp("server_start", e.to_string()))?;

    Ok(())
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

impl TransportMode {
    /// Convert to `HyperServerConfig`
    #[must_use]
    pub fn to_hyper_config(&self) -> Option<HyperServerConfig> {
        match self {
            TransportMode::Stdio => None,
            TransportMode::Http => Some(HyperServerConfig::http()),
            TransportMode::Sse => Some(HyperServerConfig::sse()),
            TransportMode::Hybrid => Some(HyperServerConfig::hybrid()),
        }
    }
}

/// Run server with the specified transport mode
pub async fn run_server_with_mode(server: &CratesDocsServer, mode: TransportMode) -> Result<()> {
    match mode {
        TransportMode::Stdio => run_stdio_server(server).await,
        TransportMode::Http | TransportMode::Sse | TransportMode::Hybrid => {
            let config = mode
                .to_hyper_config()
                .expect("Hyper config should exist for HTTP/SSE/Hybrid");
            run_hyper_server(server, config).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::unenforced_server_limits;
    use crate::config::AppConfig;

    #[test]
    fn test_unenforced_limits_empty_for_defaults() {
        let config = AppConfig::default();
        assert!(unenforced_server_limits(&config).is_empty());
    }

    #[test]
    fn test_unenforced_limits_flags_changed_fields() {
        let mut config = AppConfig::default();
        config.server.request_timeout_secs += 1;
        config.server.max_connections += 1;
        let flagged = unenforced_server_limits(&config);
        assert!(flagged.contains(&"request_timeout_secs"));
        assert!(flagged.contains(&"max_connections"));
        assert!(!flagged.contains(&"response_timeout_secs"));
    }
}
