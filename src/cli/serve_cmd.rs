//! Serve command implementation

use crate::server::transport;
use crate::CratesDocsServer;
use std::path::PathBuf;

/// Start server command
#[allow(clippy::too_many_arguments)]
pub async fn run_serve_command(
    config_path: &PathBuf,
    debug: bool,
    mode: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    enable_oauth: Option<bool>,
    oauth_client_id: Option<String>,
    oauth_client_secret: Option<String>,
    oauth_redirect_uri: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = load_config(
        config_path,
        host,
        port,
        mode,
        enable_oauth,
        oauth_client_id,
        oauth_client_secret,
        oauth_redirect_uri,
    )?;

    // Get the actual transport mode (for logging and startup)
    let transport_mode = config.server.transport_mode.clone();

    // Initialize logging system (prefer config file, debug mode uses debug level)
    if debug {
        // In debug mode, override log level from config file
        let mut debug_config = config.logging.clone();
        debug_config.level = "debug".to_string();
        crate::init_logging_with_config(&debug_config)
            .map_err(|e| format!("Failed to initialize logging system: {e}"))?;
    } else {
        crate::init_logging_with_config(&config.logging)
            .map_err(|e| format!("Failed to initialize logging system: {e}"))?;
    }

    tracing::info!(
        "Starting Crates Docs MCP Server v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Create server (async to support Redis)
    let server: CratesDocsServer = CratesDocsServer::new_async(config)
        .await
        .map_err(|e| format!("Failed to create server: {e}"))?;

    // Start server based on mode
    match transport_mode.to_lowercase().as_str() {
        "stdio" => {
            tracing::info!("Using Stdio transport mode");
            transport::run_stdio_server(&server)
                .await
                .map_err(|e| format!("Failed to start Stdio server: {e}"))?;
        }
        "http" => {
            tracing::info!(
                "Using HTTP transport mode, listening on {}:{}",
                server.config().server.host,
                server.config().server.port
            );
            transport::run_http_server(&server)
                .await
                .map_err(|e| format!("Failed to start HTTP server: {e}"))?;
        }
        "sse" => {
            tracing::info!(
                "Using SSE transport mode, listening on {}:{}",
                server.config().server.host,
                server.config().server.port
            );
            transport::run_sse_server(&server)
                .await
                .map_err(|e| format!("Failed to start SSE server: {e}"))?;
        }
        "hybrid" => {
            tracing::info!(
                "Using hybrid transport mode (HTTP + SSE), listening on {}:{}",
                server.config().server.host,
                server.config().server.port
            );
            transport::run_hybrid_server(&server)
                .await
                .map_err(|e| format!("Failed to start hybrid server: {e}"))?;
        }
        _ => {
            return Err(format!("Unknown transport mode: {transport_mode}").into());
        }
    }

    Ok(())
}

/// Load configuration
#[allow(clippy::too_many_arguments)]
fn load_config(
    config_path: &PathBuf,
    host: Option<String>,
    port: Option<u16>,
    mode: Option<String>,
    enable_oauth: Option<bool>,
    oauth_client_id: Option<String>,
    oauth_client_secret: Option<String>,
    oauth_redirect_uri: Option<String>,
) -> Result<crate::config::AppConfig, Box<dyn std::error::Error>> {
    let mut config = if config_path.exists() {
        tracing::info!("Loading configuration from file: {}", config_path.display());
        crate::config::AppConfig::from_file(config_path)
            .map_err(|e| format!("Failed to load config file: {e}"))?
    } else {
        tracing::warn!(
            "Config file does not exist, using default config: {}",
            config_path.display()
        );
        crate::config::AppConfig::default()
    };

    // Only override config file when command line arguments are explicitly provided
    if let Some(h) = host {
        config.server.host = h;
        tracing::info!(
            "Command line argument overrides host: {}",
            config.server.host
        );
    }
    if let Some(p) = port {
        config.server.port = p;
        tracing::info!(
            "Command line argument overrides port: {}",
            config.server.port
        );
    }
    if let Some(m) = mode {
        config.server.transport_mode = m;
        tracing::info!(
            "Command line argument overrides transport_mode: {}",
            config.server.transport_mode
        );
    }
    if let Some(eo) = enable_oauth {
        config.server.enable_oauth = eo;
        tracing::info!(
            "Command line argument overrides enable_oauth: {}",
            config.server.enable_oauth
        );
    }

    // Override command line OAuth parameters (if provided)
    if let Some(client_id) = oauth_client_id {
        config.oauth.client_id = Some(client_id);
        config.oauth.enabled = true;
    }
    if let Some(client_secret) = oauth_client_secret {
        config.oauth.client_secret = Some(client_secret);
    }
    if let Some(redirect_uri) = oauth_redirect_uri {
        config.oauth.redirect_uri = Some(redirect_uri);
    }

    // Validate configuration
    config
        .validate()
        .map_err(|e| format!("Configuration validation failed: {e}"))?;

    Ok(config)
}
