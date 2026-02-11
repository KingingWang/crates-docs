//! Crates Docs MCP 服务器主程序

use clap::{Parser, Subcommand};
use crates_docs::server::transport;
use crates_docs::CratesDocsServer;
use rust_mcp_sdk::schema::{Icon, IconTheme};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "crates-docs")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "高性能 Rust crate 文档查询 MCP 服务器", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// 配置文件路径
    #[arg(short, long, global = true, default_value = "config.toml")]
    config: PathBuf,

    /// 启用调试日志
    #[arg(short, long, global = true)]
    debug: bool,

    /// 启用详细输出
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// 启动服务器
    Serve {
        /// 传输模式 [stdio, http, sse, hybrid]
        #[arg(short, long)]
        mode: Option<String>,

        /// 监听主机
        #[arg(long)]
        host: Option<String>,

        /// 监听端口
        #[arg(short, long)]
        port: Option<u16>,

        /// 启用 OAuth 认证
        #[arg(long)]
        enable_oauth: Option<bool>,

        /// OAuth 客户端 ID
        #[arg(long)]
        oauth_client_id: Option<String>,

        /// OAuth 客户端密钥
        #[arg(long)]
        oauth_client_secret: Option<String>,

        /// OAuth 重定向 URI
        #[arg(long)]
        oauth_redirect_uri: Option<String>,
    },

    /// 生成配置文件
    Config {
        /// 输出文件路径
        #[arg(short, long, default_value = "config.toml")]
        output: PathBuf,

        /// 覆盖已存在的文件
        #[arg(short, long)]
        force: bool,
    },

    /// 测试工具
    Test {
        /// 要测试的工具 [lookup_crate, search_crates, lookup_item, health_check]
        #[arg(short, long, default_value = "lookup_crate")]
        tool: String,

        /// Crate 名称（用于 lookup_crate 和 lookup_item）
        #[arg(long)]
        crate_name: Option<String>,

        /// 项目路径（用于 lookup_item）
        #[arg(long)]
        item_path: Option<String>,

        /// 搜索查询（用于 search_crates）
        #[arg(long)]
        query: Option<String>,

        /// 版本号（可选）
        #[arg(long)]
        version: Option<String>,

        /// 结果限制（用于 search_crates）
        #[arg(long, default_value = "10")]
        limit: u32,

        /// 输出格式 [json, markdown, text]
        #[arg(long, default_value = "markdown")]
        format: String,
    },

    /// 检查服务器健康状态
    Health {
        /// 检查类型 [all, external, internal, docs_rs, crates_io]
        #[arg(short = 't', long, default_value = "all")]
        check_type: String,

        /// 详细输出
        #[arg(short, long)]
        verbose: bool,
    },

    /// 显示版本信息
    Version,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // 注意：日志系统将在 serve_command 中初始化（使用配置文件）
    // 这里不提前初始化，以便使用配置文件中的日志设置

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

/// 启动服务器命令
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
    // 加载配置
    let config = load_config(config_path, host, port, mode, enable_oauth, oauth_client_id, oauth_client_secret, oauth_redirect_uri).await?;

    // 获取实际使用的传输模式（用于日志和启动）
    let transport_mode = config.transport_mode.clone();

    // 初始化日志系统（优先使用配置文件，debug 模式使用 debug 级别）
    if debug {
        // 在 debug 模式下，覆盖配置文件中的日志级别
        let mut debug_config = config.logging.clone();
        debug_config.level = "debug".to_string();
        crates_docs::init_logging_with_config(&debug_config)
            .map_err(|e| format!("初始化日志系统失败: {e}"))?;
    } else {
        crates_docs::init_logging_with_config(&config.logging)
            .map_err(|e| format!("初始化日志系统失败: {e}"))?;
    }

    tracing::info!("启动 Crates Docs MCP 服务器 v{}", env!("CARGO_PKG_VERSION"));

    // 创建服务器（异步方式支持 Redis）
    let server: CratesDocsServer =
        CratesDocsServer::new_async(config).await.map_err(|e| format!("创建服务器失败: {}", e))?;

    // 根据模式启动服务器
    match transport_mode.to_lowercase().as_str() {
        "stdio" => {
            tracing::info!("使用 Stdio 传输模式");
            transport::run_stdio_server(&server)
                .await
                .map_err(|e| format!("Stdio 服务器启动失败: {}", e))?;
        }
        "http" => {
            tracing::info!("使用 HTTP 传输模式，监听 {}:{}", server.config().host, server.config().port);
            transport::run_http_server(&server)
                .await
                .map_err(|e| format!("HTTP 服务器启动失败: {}", e))?;
        }
        "sse" => {
            tracing::info!("使用 SSE 传输模式，监听 {}:{}", server.config().host, server.config().port);
            transport::run_sse_server(&server)
                .await
                .map_err(|e| format!("SSE 服务器启动失败: {}", e))?;
        }
        "hybrid" => {
            tracing::info!("使用混合传输模式（HTTP + SSE），监听 {}:{}", server.config().host, server.config().port);
            transport::run_hybrid_server(&server)
                .await
                .map_err(|e| format!("混合服务器启动失败: {}", e))?;
        }
        _ => {
            return Err(format!("未知的传输模式: {}", transport_mode).into());
        }
    }

    Ok(())
}

/// 加载配置
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
        tracing::info!("从文件加载配置: {}", config_path.display());
        crates_docs::config::AppConfig::from_file(config_path)
            .map_err(|e| format!("加载配置文件失败: {}", e))?
    } else {
        tracing::warn!("配置文件不存在，使用默认配置: {}", config_path.display());
        crates_docs::config::AppConfig::default()
    };

    // 仅当命令行参数显式提供时，才覆盖配置文件
    if let Some(h) = host {
        config.server.host = h;
        tracing::info!("命令行参数覆盖 host: {}", config.server.host);
    }
    if let Some(p) = port {
        config.server.port = p;
        tracing::info!("命令行参数覆盖 port: {}", config.server.port);
    }
    if let Some(m) = mode {
        config.server.transport_mode = m;
        tracing::info!("命令行参数覆盖 transport_mode: {}", config.server.transport_mode);
    }
    if let Some(eo) = enable_oauth {
        config.server.enable_oauth = eo;
        tracing::info!("命令行参数覆盖 enable_oauth: {}", config.server.enable_oauth);
    }

    // 覆盖命令行 OAuth 参数（如果提供）
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

    // 验证配置
    config
        .validate()
        .map_err(|e| format!("配置验证失败: {}", e))?;

    // 将 config::AppConfig 转换为 server::ServerConfig（传递所有配置）
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

/// 生成配置文件命令
fn config_command(output: &PathBuf, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    if output.exists() && !force {
        return Err(format!("配置文件已存在: {}，使用 --force 覆盖", output.display()).into());
    }

    let config = crates_docs::config::AppConfig::default();
    config
        .save_to_file(output)
        .map_err(|e| format!("保存配置文件失败: {}", e))?;

    println!("配置文件已生成: {}", output.display());
    println!("请根据需要编辑配置文件。");

    Ok(())
}

/// 测试工具命令
async fn test_command(
    tool: &str,
    crate_name: Option<&str>,
    item_path: Option<&str>,
    query: Option<&str>,
    version: Option<&str>,
    limit: u32,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("测试工具: {}", tool);

    // 创建缓存
    let cache_config = crates_docs::cache::CacheConfig {
        cache_type: "memory".to_string(),
        memory_size: Some(1000),
        default_ttl: Some(3600),
        redis_url: None,
    };

    let cache = crates_docs::cache::create_cache(&cache_config)?;
    let cache_arc: std::sync::Arc<dyn crates_docs::cache::Cache> = std::sync::Arc::from(cache);

    // 创建文档服务
    let doc_service = std::sync::Arc::new(crates_docs::tools::docs::DocService::new(cache_arc));

    // 创建工具注册表
    let registry = crates_docs::tools::create_default_registry(&doc_service);

    match tool {
        "lookup_crate" => {
            if let Some(name) = crate_name {
                println!("测试查找 crate: {} (版本: {:?})", name, version);
                println!("输出格式: {}", format);

                // 准备参数
                let mut arguments = serde_json::json!({
                    "crate_name": name,
                    "format": format
                });

                if let Some(v) = version {
                    arguments["version"] = serde_json::Value::String(v.to_string());
                }

                // 执行工具
                match registry.execute_tool("lookup_crate", arguments).await {
                    Ok(result) => {
                        println!("工具执行成功:");
                        if let Some(content) = result.content.first() {
                            match content {
                                rust_mcp_sdk::schema::ContentBlock::TextContent(text_content) => {
                                    println!("{}", text_content.text);
                                }
                                other => {
                                    println!("非文本内容: {:?}", other);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("工具执行失败: {}", e);
                    }
                }
            } else {
                return Err("lookup_crate 需要 --crate-name 参数".into());
            }
        }
        "search_crates" => {
            if let Some(q) = query {
                println!("测试搜索 crate: {} (限制: {})", q, limit);
                println!("输出格式: {}", format);

                // 准备参数 - search_crates 可能也需要 camelCase
                let arguments = serde_json::json!({
                    "query": q,
                    "limit": limit,
                    "format": format
                });

                // 执行工具
                match registry.execute_tool("search_crates", arguments).await {
                    Ok(result) => {
                        println!("工具执行成功:");
                        if let Some(content) = result.content.first() {
                            match content {
                                rust_mcp_sdk::schema::ContentBlock::TextContent(text_content) => {
                                    println!("{}", text_content.text);
                                }
                                other => {
                                    println!("非文本内容: {:?}", other);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("工具执行失败: {}", e);
                    }
                }
            } else {
                return Err("search_crates 需要 --query 参数".into());
            }
        }
        "lookup_item" => {
            if let (Some(name), Some(path)) = (crate_name, item_path) {
                println!("测试查找项目: {}::{} (版本: {:?})", name, path, version);
                println!("输出格式: {}", format);

                // 准备参数
                let mut arguments = serde_json::json!({
                    "crate_name": name,
                    "itemPath": path,
                    "format": format
                });

                if let Some(v) = version {
                    arguments["version"] = serde_json::Value::String(v.to_string());
                }

                // 执行工具
                match registry.execute_tool("lookup_item", arguments).await {
                    Ok(result) => {
                        println!("工具执行成功:");
                        if let Some(content) = result.content.first() {
                            match content {
                                rust_mcp_sdk::schema::ContentBlock::TextContent(text_content) => {
                                    println!("{}", text_content.text);
                                }
                                other => {
                                    println!("非文本内容: {:?}", other);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("工具执行失败: {}", e);
                    }
                }
            } else {
                return Err("lookup_item 需要 --crate-name 和 --item-path 参数".into());
            }
        }
        "health_check" => {
            println!("测试健康检查");

            // 准备参数 - health_check 可能也需要 camelCase
            let arguments = serde_json::json!({
                "checkType": "all",
                "verbose": true
            });

            // 执行工具
            match registry.execute_tool("health_check", arguments).await {
                Ok(result) => {
                    println!("工具执行成功:");
                    if let Some(content) = result.content.first() {
                        match content {
                            rust_mcp_sdk::schema::ContentBlock::TextContent(text_content) => {
                                println!("{}", text_content.text);
                            }
                            other => {
                                println!("非文本内容: {:?}", other);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("工具执行失败: {}", e);
                }
            }
        }
        _ => {
            return Err(format!("未知的工具: {}", tool).into());
        }
    }

    println!("工具测试完成");
    Ok(())
}

/// 健康检查命令
async fn health_command(check_type: &str, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("执行健康检查: {}", check_type);
    println!("详细模式: {}", verbose);

    // 这里可以添加实际的健康检查逻辑
    println!("健康检查完成（模拟）");
    Ok(())
}

/// 版本命令
fn version_command() {
    println!("Crates Docs MCP 服务器 v{}", env!("CARGO_PKG_VERSION"));
    println!("构建时间: {}", env!("BUILD_TIMESTAMP"));
    println!("Git 提交: {}", env!("GIT_COMMIT"));
    println!("Rust 版本: {}", env!("RUST_VERSION"));
}
