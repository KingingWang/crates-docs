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

/// Whether in-process API-key enforcement is active for this build and config.
///
/// True only when the binary is compiled with **both** the `api-key` and `auth`
/// features (so [`crate::server::auth::ApiKeyAuthProvider`] exists and the SDK
/// can attach its `AuthMiddleware`) **and** `api_key.enabled` is set. When true,
/// the HTTP/SSE transport rejects any request lacking a valid
/// `Authorization: Bearer <key>` with 401; when false, no API-key check runs
/// in-process.
#[cfg(all(feature = "api-key", feature = "auth"))]
fn api_key_auth_enforced(server_config: &crate::config::AppConfig) -> bool {
    server_config.auth.api_key.enabled
}

/// Fallback for builds without both `api-key` and `auth`: enforcement is
/// impossible, so it is never active.
#[cfg(not(all(feature = "api-key", feature = "auth")))]
fn api_key_auth_enforced(_server_config: &crate::config::AppConfig) -> bool {
    false
}

/// Build the SDK auth provider for in-process API-key enforcement, if enabled.
///
/// Returns `Some(provider)` only when `api_key.enabled` is set; the SDK's
/// `HyperServer::new` then auto-attaches its `AuthMiddleware`. Returns `None`
/// otherwise, leaving the transport unauthenticated (the runtime on/off switch).
#[cfg(all(feature = "api-key", feature = "auth"))]
fn build_api_key_auth(
    server_config: &crate::config::AppConfig,
) -> Option<Arc<dyn rust_mcp_sdk::auth::AuthProvider>> {
    server_config.auth.api_key.enabled.then(|| {
        Arc::new(crate::server::auth::ApiKeyAuthProvider::new(
            server_config.auth.api_key.clone(),
        )) as Arc<dyn rust_mcp_sdk::auth::AuthProvider>
    })
}

/// Report how authentication settings map onto actual HTTP/SSE enforcement.
///
/// In-process API-key enforcement is active only when the binary is built with
/// both the `api-key` and `auth` features and `api_key.enabled` is set; the
/// SDK's `AuthMiddleware` then rejects requests without a valid
/// `Authorization: Bearer <key>`. OAuth, by contrast, is still **not** wired
/// into the request pipeline, so an enabled OAuth config protects nothing.
/// Reporting both accurately avoids giving operators a false sense of security
/// (and avoids hiding that protection is, in fact, on).
fn warn_if_auth_configured_but_unenforced(server_config: &crate::config::AppConfig) {
    #[cfg(feature = "api-key")]
    if server_config.auth.api_key.enabled {
        if api_key_auth_enforced(server_config) {
            tracing::info!(
                "API key authentication is ENFORCED on the HTTP/SSE transport: clients must send \
                 `Authorization: Bearer <key>` or receive 401 (the /health endpoint stays open). \
                 The in-process layer does not read the `X-API-Key` header directly — to keep \
                 using `X-API-Key` and to encrypt traffic with TLS, front the server with the \
                 bundled reverse proxy (docs/reverse-proxy/)."
            );
        } else {
            tracing::warn!(
                "API key authentication is enabled in configuration but is NOT enforced: this \
                 binary was built without the `auth` feature, so HTTP/SSE requests are \
                 unauthenticated. Rebuild with the `auth` feature (it is in the default set) or \
                 front the server with an authenticating reverse proxy. Do not expose this \
                 server on an untrusted network."
            );
        }
    }

    // OAuth is accepted in configuration but never attached to the pipeline.
    if server_config.auth.oauth.enabled || server_config.oauth.enabled {
        tracing::warn!(
            "OAuth authentication is enabled in configuration but is NOT enforced on the \
             HTTP/SSE transport: OAuth requests are not validated. Do not rely on it for access \
             control; use API-key authentication or an authenticating reverse proxy instead."
        );
    }
}

/// Warn when API-key header / query settings are set but ignored in-process.
///
/// The SDK middleware reads **only** `Authorization: Bearer <token>` — it cannot
/// honor a custom `header_name` or `allow_query_param`. Those settings take
/// effect solely at a fronting reverse proxy. The default `header_name`
/// (`X-API-Key`) is the documented proxy path and is covered by the info log
/// above, so this only fires for a genuinely non-default header or an enabled
/// query parameter, preventing the belief that the server reads them directly.
#[cfg(all(feature = "api-key", feature = "auth"))]
fn warn_if_api_key_header_settings_ignored(server_config: &crate::config::AppConfig) {
    if !api_key_auth_enforced(server_config) {
        return;
    }
    let non_default_header = !server_config
        .auth
        .api_key
        .header_name
        .eq_ignore_ascii_case("x-api-key");
    let query_allowed = server_config.auth.api_key.allow_query_param;
    if non_default_header || query_allowed {
        tracing::warn!(
            header_name = %server_config.auth.api_key.header_name,
            allow_query_param = query_allowed,
            "In-process API-key enforcement reads ONLY `Authorization: Bearer <key>`; the \
             configured `header_name` and `allow_query_param` are ignored by the server and take \
             effect only at a fronting reverse proxy. Translate your custom header / query param \
             into `Authorization: Bearer <key>` at the proxy — see docs/reverse-proxy/."
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

/// Whether the `server.enable_sse` setting contradicts the active transport.
///
/// SSE support is decided solely by the transport mode (`sse`/`hybrid` enable
/// it; `http` does not); the `enable_sse` config flag is never consulted. A
/// mismatch means the operator's `enable_sse` value is being ignored.
fn enable_sse_setting_ignored(configured_enable_sse: bool, sse_active: bool) -> bool {
    configured_enable_sse != sse_active
}

/// Warn when `server.enable_sse` does not match the transport-derived state.
fn warn_if_enable_sse_ignored(server_config: &crate::config::AppConfig, sse_active: bool) {
    if enable_sse_setting_ignored(server_config.server.enable_sse, sse_active) {
        tracing::warn!(
            configured_enable_sse = server_config.server.enable_sse,
            sse_active,
            "server.enable_sse does not match the active transport and is being ignored: SSE \
             support is determined solely by transport_mode (sse/hybrid serve SSE, http does not). \
             Set transport_mode to control SSE; the enable_sse flag has no effect."
        );
    }
}

/// Whether `host` is a loopback address (or `localhost`).
///
/// Used to decide whether binding exposes the server beyond the local machine.
/// Anything that is not an IP loopback address and not `localhost` is treated
/// as network-exposed (conservative: unknown hostnames warn).
fn host_is_loopback(host: &str) -> bool {
    match host.parse::<std::net::IpAddr>() {
        Ok(ip) => ip.is_loopback(),
        Err(_) => host.eq_ignore_ascii_case("localhost"),
    }
}

/// Warn when the server binds to a non-loopback address and is therefore
/// reachable from other hosts on the network.
///
/// If in-process API-key auth is enforced, the risk is plaintext exposure
/// (requests and keys travel unencrypted over HTTP); otherwise the risk is the
/// absence of any authentication. Both warrant a reverse proxy.
fn warn_if_network_exposed(server_config: &crate::config::AppConfig) {
    if host_is_loopback(&server_config.server.host) {
        return;
    }
    if api_key_auth_enforced(server_config) {
        tracing::warn!(
            host = %server_config.server.host,
            "Server is binding to a non-loopback address and is reachable from other hosts on \
             the network. API-key authentication IS enforced (requests need `Authorization: \
             Bearer <key>`), but traffic is sent UNENCRYPTED over plain HTTP: anyone who can \
             observe the network sees requests and keys in clear text. Terminate TLS with the \
             bundled reverse proxy (docs/reverse-proxy/) or restrict the network."
        );
    } else {
        tracing::warn!(
            host = %server_config.server.host,
            "Server is binding to a non-loopback address and is reachable from other hosts on \
             the network. The HTTP/SSE transport performs no authentication; put a reverse proxy \
             with authentication in front of it, restrict the network, or run in stdio mode."
        );
    }
}

/// Warn when DNS rebinding protection is disabled, so the configured
/// `allowed_hosts`/`allowed_origins` allowlists are not enforced.
///
/// With protection off (the default) the SDK installs no `Host`/`Origin`
/// validation, so a malicious web page loaded in a local browser can reach
/// this server via DNS rebinding. Surfacing this avoids a false sense of
/// security from the presence of the allowlist settings.
fn warn_if_dns_rebinding_protection_disabled(server_config: &crate::config::AppConfig) {
    if !server_config.server.dns_rebinding_protection {
        tracing::warn!(
            "dns_rebinding_protection is disabled: the allowed_hosts/allowed_origins allowlists \
             are NOT enforced, so a malicious local web page could reach this server via DNS \
             rebinding. Set server.dns_rebinding_protection = true (with exact host:port and \
             origin values) to enable Host/Origin validation."
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
    #[cfg(all(feature = "api-key", feature = "auth"))]
    warn_if_api_key_header_settings_ignored(server_config);
    warn_if_metrics_configured_but_unavailable(server_config);
    warn_if_unenforced_server_limits_configured(server_config);
    warn_if_enable_sse_ignored(server_config, config.sse_support());
    warn_if_network_exposed(server_config);
    warn_if_dns_rebinding_protection_disabled(server_config);

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
        // Runtime on/off switch for in-process auth: `Some` only when
        // `api_key.enabled` is set, which makes the SDK attach its
        // `AuthMiddleware`. Toggling the config flag + restart flips
        // enforcement without a rebuild. Cfg-gated as a field init (rather than
        // a `mut` mutation) so `options` stays immutable under `-D warnings`.
        #[cfg(all(feature = "api-key", feature = "auth"))]
        auth: build_api_key_auth(server_config),
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

    #[test]
    fn test_host_is_loopback() {
        assert!(super::host_is_loopback("127.0.0.1"));
        assert!(super::host_is_loopback("::1"));
        assert!(super::host_is_loopback("localhost"));
        assert!(super::host_is_loopback("LocalHost"));
        // Non-loopback / network-exposed binds.
        assert!(!super::host_is_loopback("0.0.0.0"));
        assert!(!super::host_is_loopback("::"));
        assert!(!super::host_is_loopback("192.168.1.5"));
        assert!(!super::host_is_loopback("example.com"));
    }

    #[test]
    fn test_enable_sse_setting_ignored() {
        // No contradiction: enable_sse matches the active SSE state.
        assert!(!super::enable_sse_setting_ignored(true, true));
        assert!(!super::enable_sse_setting_ignored(false, false));
        // Contradiction: setting is ignored.
        assert!(super::enable_sse_setting_ignored(false, true));
        assert!(super::enable_sse_setting_ignored(true, false));
    }

    #[cfg(all(feature = "api-key", feature = "auth"))]
    #[test]
    fn test_api_key_auth_enforced_tracks_enabled_flag() {
        let mut config = AppConfig::default();
        // Disabled by default → no in-process enforcement.
        assert!(!super::api_key_auth_enforced(&config));
        // Flipping the runtime flag turns enforcement on (no rebuild needed).
        config.auth.api_key.enabled = true;
        assert!(super::api_key_auth_enforced(&config));
    }

    #[cfg(all(feature = "api-key", feature = "auth"))]
    #[test]
    fn test_build_api_key_auth_follows_enabled_flag() {
        let mut config = AppConfig::default();
        assert!(super::build_api_key_auth(&config).is_none());
        config.auth.api_key.enabled = true;
        assert!(super::build_api_key_auth(&config).is_some());
    }
}
