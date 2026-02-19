//! Crates Docs MCP Server main program

use clap::{Parser, Subcommand};
use crates_docs::server::transport;
use crates_docs::CratesDocsServer;
use rust_mcp_sdk::schema::{Icon, IconTheme};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "crates-docs")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "High-performance Rust crate documentation query MCP server", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Configuration file path
    #[arg(short, long, global = true, default_value = "config.toml")]
    config: PathBuf,

    /// Enable debug logging
    #[arg(short, long, global = true)]
    debug: bool,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the server
    Serve {
        /// Transport mode [stdio, http, sse, hybrid]
        #[arg(short, long)]
        mode: Option<String>,

        /// Listen host
        #[arg(long)]
        host: Option<String>,

        /// Listen port
        #[arg(short, long)]
        port: Option<u16>,

        /// Enable OAuth authentication
        #[arg(long)]
        enable_oauth: Option<bool>,

        /// OAuth client ID
        #[arg(long)]
        oauth_client_id: Option<String>,

        /// OAuth client secret
        #[arg(long)]
        oauth_client_secret: Option<String>,

        /// OAuth redirect URI
        #[arg(long)]
        oauth_redirect_uri: Option<String>,
    },

    /// Generate configuration file
    Config {
        /// Output file path
        #[arg(short, long, default_value = "config.toml")]
        output: PathBuf,

        /// Overwrite existing file
        #[arg(short, long)]
        force: bool,
    },

    /// Test tool
    Test {
        /// Tool to test [lookup_crate, search_crates, lookup_item, health_check]
        #[arg(short, long, default_value = "lookup_crate")]
        tool: String,

        /// Crate name (for lookup_crate and lookup_item)
        #[arg(long)]
        crate_name: Option<String>,

        /// Item path (for lookup_item)
        #[arg(long)]
        item_path: Option<String>,

        /// Search query (for search_crates)
        #[arg(long)]
        query: Option<String>,

        /// Version number (optional)
        #[arg(long)]
        version: Option<String>,

        /// Result limit (for search_crates)
        #[arg(long, default_value = "10")]
        limit: u32,

        /// Output format [json, markdown, text]
        #[arg(long, default_value = "markdown")]
        format: String,
    },

    /// Check server health status
    Health {
        /// Check type [all, external, internal, docs_rs, crates_io]
        #[arg(short = 't', long, default_value = "all")]
        check_type: String,

        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Display version information
    Version,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Note: The logging system will be initialized in serve_command (using config file)
    // We don't initialize it here to allow using log settings from the config file

    match cli.command {
        Commands::Serve {
            mode,
            host,
            port,
            enable_oauth,
            oauth_client_id,
            oauth_client_secret,
            oauth_redirect_uri,
        } => {
            serve_command(
                &cli.config,
                cli.debug,
                mode,
                host,
                port,
                enable_oauth,
                oauth_client_id,
                oauth_client_secret,
                oauth_redirect_uri,
            )
            .await?;
        }
        Commands::Config { output, force } => {
            config_command(&output, force)?;
        }
        Commands::Test {
            tool,
            crate_name,
            item_path,
            query,
            version,
            limit,
            format,
        } => {
            test_command(
                &tool,
                crate_name.as_deref(),
                item_path.as_deref(),
                query.as_deref(),
                version.as_deref(),
                limit,
                &format,
            )
            .await?;
        }
        Commands::Health {
            check_type,
            verbose,
        } => {
            health_command(&check_type, verbose).await?;
        }
        Commands::Version => {
            version_command();
        }
    }

    Ok(())
}

/// Start server command
#[allow(clippy::too_many_arguments)]
async fn serve_command(
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
    )
    .await?;

    // Get the actual transport mode (for logging and startup)
    let transport_mode = config.transport_mode.clone();

    // Initialize logging system (prefer config file, debug mode uses debug level)
    if debug {
        // In debug mode, override log level from config file
        let mut debug_config = config.logging.clone();
        debug_config.level = "debug".to_string();
        crates_docs::init_logging_with_config(&debug_config)
            .map_err(|e| format!("Failed to initialize logging system: {e}"))?;
    } else {
        crates_docs::init_logging_with_config(&config.logging)
            .map_err(|e| format!("Failed to initialize logging system: {e}"))?;
    }

    tracing::info!("Starting Crates Docs MCP Server v{}", env!("CARGO_PKG_VERSION"));

    // Create server (async to support Redis)
    let server: CratesDocsServer = CratesDocsServer::new_async(config)
        .await
        .map_err(|e| format!("Failed to create server: {}", e))?;

    // Start server based on mode
    match transport_mode.to_lowercase().as_str() {
        "stdio" => {
            tracing::info!("Using Stdio transport mode");
            transport::run_stdio_server(&server)
                .await
                .map_err(|e| format!("Failed to start Stdio server: {}", e))?;
        }
        "http" => {
            tracing::info!(
                "Using HTTP transport mode, listening on {}:{}",
                server.config().host,
                server.config().port
            );
            transport::run_http_server(&server)
                .await
                .map_err(|e| format!("Failed to start HTTP server: {}", e))?;
        }
        "sse" => {
            tracing::info!(
                "Using SSE transport mode, listening on {}:{}",
                server.config().host,
                server.config().port
            );
            transport::run_sse_server(&server)
                .await
                .map_err(|e| format!("Failed to start SSE server: {}", e))?;
        }
        "hybrid" => {
            tracing::info!(
                "Using hybrid transport mode (HTTP + SSE), listening on {}:{}",
                server.config().host,
                server.config().port
            );
            transport::run_hybrid_server(&server)
                .await
                .map_err(|e| format!("Failed to start hybrid server: {}", e))?;
        }
        _ => {
            return Err(format!("Unknown transport mode: {}", transport_mode).into());
        }
    }

    Ok(())
}

/// Load configuration
#[allow(clippy::too_many_arguments)]
async fn load_config(
    config_path: &PathBuf,
    host: Option<String>,
    port: Option<u16>,
    mode: Option<String>,
    enable_oauth: Option<bool>,
    oauth_client_id: Option<String>,
    oauth_client_secret: Option<String>,
    oauth_redirect_uri: Option<String>,
) -> Result<crates_docs::ServerConfig, Box<dyn std::error::Error>> {
    let mut config = if config_path.exists() {
        tracing::info!("Loading configuration from file: {}", config_path.display());
        crates_docs::config::AppConfig::from_file(config_path)
            .map_err(|e| format!("Failed to load config file: {}", e))?
    } else {
        tracing::warn!("Config file does not exist, using default config: {}", config_path.display());
        crates_docs::config::AppConfig::default()
    };

    // Only override config file when command line arguments are explicitly provided
    if let Some(h) = host {
        config.server.host = h;
        tracing::info!("Command line argument overrides host: {}", config.server.host);
    }
    if let Some(p) = port {
        config.server.port = p;
        tracing::info!("Command line argument overrides port: {}", config.server.port);
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
        .map_err(|e| format!("Configuration validation failed: {}", e))?;

    // Convert config::AppConfig to server::ServerConfig (pass all configuration)
    let server_config = crates_docs::ServerConfig {
        name: config.server.name,
        version: config.server.version,
        description: config.server.description,
        icons: vec![
            Icon {
                src: "https://docs.rs/static/favicon-32x32.png".to_string(),
                mime_type: Some("image/png".to_string()),
                sizes: vec!["32x32".to_string()],
                theme: Some(IconTheme::Light),
            },
            Icon {
                src: "https://docs.rs/static/favicon-32x32.png".to_string(),
                mime_type: Some("image/png".to_string()),
                sizes: vec!["32x32".to_string()],
                theme: Some(IconTheme::Dark),
            },
        ],
        website_url: Some("https://github.com/KingingWang/crates-docs".to_string()),
        host: config.server.host,
        port: config.server.port,
        transport_mode: config.server.transport_mode,
        enable_sse: config.server.enable_sse,
        enable_oauth: config.server.enable_oauth,
        max_connections: config.server.max_connections,
        request_timeout_secs: config.server.request_timeout_secs,
        response_timeout_secs: config.server.response_timeout_secs,
        cache: config.cache,
        oauth: config.oauth,
        logging: config.logging,
        performance: config.performance,
    };

    Ok(server_config)
}

/// Generate configuration file command
fn config_command(output: &PathBuf, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    if output.exists() && !force {
        return Err(format!("Config file already exists: {}, use --force to overwrite", output.display()).into());
    }

    let config = crates_docs::config::AppConfig::default();
    config
        .save_to_file(output)
        .map_err(|e| format!("Failed to save config file: {}", e))?;

    println!("Config file generated: {}", output.display());
    println!("Please edit the config file as needed.");

    Ok(())
}

/// Test tool command
async fn test_command(
    tool: &str,
    crate_name: Option<&str>,
    item_path: Option<&str>,
    query: Option<&str>,
    version: Option<&str>,
    limit: u32,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Testing tool: {}", tool);

    // Create cache
    let cache_config = crates_docs::cache::CacheConfig {
        cache_type: "memory".to_string(),
        memory_size: Some(1000),
        default_ttl: Some(3600),
        redis_url: None,
    };

    let cache = crates_docs::cache::create_cache(&cache_config)?;
    let cache_arc: std::sync::Arc<dyn crates_docs::cache::Cache> = std::sync::Arc::from(cache);

    // Create document service
    let doc_service = std::sync::Arc::new(crates_docs::tools::docs::DocService::new(cache_arc));

    // Create tool registry
    let registry = crates_docs::tools::create_default_registry(&doc_service);

    match tool {
        "lookup_crate" => {
            if let Some(name) = crate_name {
                println!("Testing crate lookup: {} (version: {:?})", name, version);
                println!("Output format: {}", format);

                // Prepare arguments
                let mut arguments = serde_json::json!({
                    "crate_name": name,
                    "format": format
                });

                if let Some(v) = version {
                    arguments["version"] = serde_json::Value::String(v.to_string());
                }

                // Execute tool
                match registry.execute_tool("lookup_crate", arguments).await {
                    Ok(result) => {
                        println!("Tool executed successfully:");
                        if let Some(content) = result.content.first() {
                            match content {
                                rust_mcp_sdk::schema::ContentBlock::TextContent(text_content) => {
                                    println!("{}", text_content.text);
                                }
                                other => {
                                    println!("Non-text content: {:?}", other);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Tool execution failed: {}", e);
                    }
                }
            } else {
                return Err("lookup_crate requires --crate-name parameter".into());
            }
        }
        "search_crates" => {
            if let Some(q) = query {
                println!("Testing crate search: {} (limit: {})", q, limit);
                println!("Output format: {}", format);

                // Prepare arguments - search_crates may also need camelCase
                let arguments = serde_json::json!({
                    "query": q,
                    "limit": limit,
                    "format": format
                });

                // Execute tool
                match registry.execute_tool("search_crates", arguments).await {
                    Ok(result) => {
                        println!("Tool executed successfully:");
                        if let Some(content) = result.content.first() {
                            match content {
                                rust_mcp_sdk::schema::ContentBlock::TextContent(text_content) => {
                                    println!("{}", text_content.text);
                                }
                                other => {
                                    println!("Non-text content: {:?}", other);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Tool execution failed: {}", e);
                    }
                }
            } else {
                return Err("search_crates requires --query parameter".into());
            }
        }
        "lookup_item" => {
            if let (Some(name), Some(path)) = (crate_name, item_path) {
                println!("Testing item lookup: {}::{} (version: {:?})", name, path, version);
                println!("Output format: {}", format);

                // Prepare arguments
                let mut arguments = serde_json::json!({
                    "crate_name": name,
                    "itemPath": path,
                    "format": format
                });

                if let Some(v) = version {
                    arguments["version"] = serde_json::Value::String(v.to_string());
                }

                // Execute tool
                match registry.execute_tool("lookup_item", arguments).await {
                    Ok(result) => {
                        println!("Tool executed successfully:");
                        if let Some(content) = result.content.first() {
                            match content {
                                rust_mcp_sdk::schema::ContentBlock::TextContent(text_content) => {
                                    println!("{}", text_content.text);
                                }
                                other => {
                                    println!("Non-text content: {:?}", other);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Tool execution failed: {}", e);
                    }
                }
            } else {
                return Err("lookup_item requires --crate-name and --item-path parameters".into());
            }
        }
        "health_check" => {
            println!("Testing health check");

            // Prepare arguments - health_check may also need camelCase
            let arguments = serde_json::json!({
                "checkType": "all",
                "verbose": true
            });

            // Execute tool
            match registry.execute_tool("health_check", arguments).await {
                Ok(result) => {
                    println!("Tool executed successfully:");
                    if let Some(content) = result.content.first() {
                        match content {
                            rust_mcp_sdk::schema::ContentBlock::TextContent(text_content) => {
                                println!("{}", text_content.text);
                            }
                            other => {
                                println!("Non-text content: {:?}", other);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Tool execution failed: {}", e);
                }
            }
        }
        _ => {
            return Err(format!("Unknown tool: {}", tool).into());
        }
    }

    println!("Tool test completed");
    Ok(())
}

/// Health check command
async fn health_command(check_type: &str, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Performing health check: {}", check_type);
    println!("Verbose mode: {}", verbose);

    // Actual health check logic can be added here
    println!("Health check completed (simulated)");
    Ok(())
}

/// Version command
fn version_command() {
    println!("Crates Docs MCP Server v{}", env!("CARGO_PKG_VERSION"));
    println!("Build time: {}", env!("BUILD_TIMESTAMP"));
    println!("Git commit: {}", env!("GIT_COMMIT"));
    println!("Rust version: {}", env!("RUST_VERSION"));
}
