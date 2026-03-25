//! Serve command implementation

use crate::config_reload::ConfigReloader;
use crate::server::transport;
use crate::CratesDocsServer;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::{interval, Duration};

#[cfg(feature = "api-key")]
fn normalize_api_keys(
    api_key_config: &crate::server::auth::ApiKeyConfig,
    keys: Vec<String>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    keys.into_iter()
        .map(|key| {
            api_key_config
                .normalize_key_material(&key)
                .map_err(|e| format!("Failed to normalize API key material: {e}").into())
        })
        .collect()
}

fn load_from_env(config: &mut crate::config::AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    let env_config = match crate::config::AppConfig::from_env() {
        Ok(config) => Some(config),
        Err(e) if e.to_string().contains("Invalid port") => return Err(e.to_string().into()),
        Err(_) => None,
    };

    *config = crate::config::AppConfig::merge(Some(config.clone()), env_config);

    #[cfg(feature = "api-key")]
    if !config.auth.api_key.keys.is_empty() {
        config.auth.api_key.keys =
            normalize_api_keys(&config.auth.api_key, config.auth.api_key.keys.clone())?;
    }

    Ok(())
}

fn init_logging(
    config: &crate::config::AppConfig,
    debug: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if debug {
        let mut debug_config = config.logging.clone();
        debug_config.level = "debug".to_string();
        crate::init_logging_with_config(&debug_config)
            .map_err(|e| format!("Failed to initialize logging system: {e}"))?;
    } else {
        crate::init_logging_with_config(&config.logging)
            .map_err(|e| format!("Failed to initialize logging system: {e}"))?;
    }
    Ok(())
}

fn start_config_reloader(config_path: &std::path::Path, server: &CratesDocsServer) {
    let config_path_arc = Arc::from(config_path.to_path_buf().into_boxed_path());
    let current_config = server.config().clone();

    match ConfigReloader::new(config_path_arc, current_config) {
        Ok(mut reloader) => {
            tracing::info!(
                "Configuration hot-reload enabled for {}",
                config_path.display()
            );

            tokio::spawn(async move {
                let mut check_interval = interval(Duration::from_secs(1));

                loop {
                    check_interval.tick().await;

                    if let Some(change) = reloader.check_for_changes() {
                        if let Some(changes) = change.changes() {
                            tracing::info!("Configuration file changed:");
                            for change_desc in changes {
                                tracing::info!("  - {}", change_desc);
                            }
                            tracing::warn!("Configuration has been reloaded. Some changes may require server restart.");
                            tracing::warn!("API key changes: New keys are now active. Removed keys are revoked immediately.");
                        }
                    }
                }
            });
        }
        Err(e) => {
            tracing::warn!("Failed to enable configuration hot-reload: {}", e);
        }
    }
}

async fn run_server_by_mode(
    server: &CratesDocsServer,
    transport_mode: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mode_str = transport_mode.to_lowercase();
    match mode_str.as_str() {
        "stdio" => {
            tracing::info!("Using Stdio transport mode");
            transport::run_stdio_server(server)
                .await
                .map_err(|e| format!("Failed to start Stdio server: {e}"))?;
        }
        "http" => {
            tracing::info!(
                "Using HTTP transport mode, listening on {}:{}",
                server.config().server.host,
                server.config().server.port
            );
            transport::run_http_server(server)
                .await
                .map_err(|e| format!("Failed to start HTTP server: {e}"))?;
        }
        "sse" => {
            tracing::info!(
                "Using SSE transport mode, listening on {}:{}",
                server.config().server.host,
                server.config().server.port
            );
            transport::run_sse_server(server)
                .await
                .map_err(|e| format!("Failed to start SSE server: {e}"))?;
        }
        "hybrid" => {
            tracing::info!(
                "Using hybrid transport mode (HTTP + SSE), listening on {}:{}",
                server.config().server.host,
                server.config().server.port
            );
            transport::run_hybrid_server(server)
                .await
                .map_err(|e| format!("Failed to start hybrid server: {e}"))?;
        }
        _ => {
            return Err(format!("Unknown transport mode: {transport_mode}").into());
        }
    }
    Ok(())
}

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
    enable_api_key: Option<bool>,
    api_keys: Option<String>,
    api_key_header: Option<String>,
    api_key_query_param: Option<bool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config(
        config_path,
        host,
        port,
        mode,
        enable_oauth,
        oauth_client_id,
        oauth_client_secret,
        oauth_redirect_uri,
        enable_api_key,
        api_keys,
        api_key_header,
        api_key_query_param,
    )?;

    let transport_mode = config.server.transport_mode.clone();

    init_logging(&config, debug)?;

    tracing::info!(
        "Starting Crates Docs MCP Server v{}",
        env!("CARGO_PKG_VERSION")
    );

    let server: CratesDocsServer = CratesDocsServer::new_async(config.clone())
        .await
        .map_err(|e| format!("Failed to create server: {e}"))?;

    let mode_str = transport_mode.to_lowercase();
    let should_enable_reload = matches!(mode_str.as_str(), "http" | "sse" | "hybrid");

    if should_enable_reload && config_path.exists() {
        start_config_reloader(config_path, &server);
    }

    run_server_by_mode(&server, &transport_mode).await
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
    enable_api_key: Option<bool>,
    api_keys: Option<String>,
    api_key_header: Option<String>,
    api_key_query_param: Option<bool>,
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

    load_from_env(&mut config)?;

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

    // Override command line API Key parameters (if provided)
    if let Some(eak) = enable_api_key {
        config.auth.api_key.enabled = eak;
        tracing::info!(
            "Command line argument overrides enable_api_key: {}",
            config.auth.api_key.enabled
        );
    }
    if let Some(keys) = api_keys {
        let parsed_keys: Vec<String> = keys
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        if !parsed_keys.is_empty() {
            #[cfg(feature = "api-key")]
            {
                config.auth.api_key.keys = normalize_api_keys(&config.auth.api_key, parsed_keys)?;
            }
            #[cfg(not(feature = "api-key"))]
            {
                config.auth.api_key.keys = parsed_keys;
            }
            config.auth.api_key.enabled = true;
            tracing::info!("Command line argument provided API key material");
        }
    }
    if let Some(header) = api_key_header {
        config.auth.api_key.header_name = header;
        tracing::info!(
            "Command line argument overrides api_key_header: {}",
            config.auth.api_key.header_name
        );
    }
    if let Some(allow_query) = api_key_query_param {
        config.auth.api_key.allow_query_param = allow_query;
        tracing::info!(
            "Command line argument overrides api_key_query_param: {}",
            config.auth.api_key.allow_query_param
        );
    }

    // Validate configuration
    config
        .validate()
        .map_err(|e| format!("Configuration validation failed: {e}"))?;

    Ok(config)
}
