//! CLI 模块单元测试

use clap::Parser;
use std::path::PathBuf;

// ============================================================================
// Cli 结构体测试
// ============================================================================

/// 测试 Cli 结构体解析 - Serve 命令
#[test]
fn test_cli_parse_serve_command() {
    let cli = crates_docs::cli::Cli::try_parse_from([
        "crates-docs",
        "serve",
        "--mode",
        "http",
        "--host",
        "0.0.0.0",
        "--port",
        "9090",
    ]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        crates_docs::cli::Commands::Serve {
            mode,
            host,
            port,
            enable_oauth,
            oauth_client_id,
            oauth_client_secret,
            oauth_redirect_uri,
        } => {
            assert_eq!(mode, Some("http".to_string()));
            assert_eq!(host, Some("0.0.0.0".to_string()));
            assert_eq!(port, Some(9090));
            assert!(enable_oauth.is_none());
            assert!(oauth_client_id.is_none());
            assert!(oauth_client_secret.is_none());
            assert!(oauth_redirect_uri.is_none());
        }
        _ => panic!("Expected Serve command"),
    }
}

/// 测试 Cli 结构体解析 - Serve 命令带 OAuth 参数
#[test]
fn test_cli_parse_serve_command_with_oauth() {
    let cli = crates_docs::cli::Cli::try_parse_from([
        "crates-docs",
        "serve",
        "--enable-oauth",
        "true",
        "--oauth-client-id",
        "test-client-id",
        "--oauth-client-secret",
        "test-secret",
        "--oauth-redirect-uri",
        "http://localhost:8080/callback",
    ]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        crates_docs::cli::Commands::Serve {
            enable_oauth,
            oauth_client_id,
            oauth_client_secret,
            oauth_redirect_uri,
            ..
        } => {
            assert_eq!(enable_oauth, Some(true));
            assert_eq!(oauth_client_id, Some("test-client-id".to_string()));
            assert_eq!(oauth_client_secret, Some("test-secret".to_string()));
            assert_eq!(
                oauth_redirect_uri,
                Some("http://localhost:8080/callback".to_string())
            );
        }
        _ => panic!("Expected Serve command"),
    }
}

/// 测试 Cli 结构体解析 - Config 命令
#[test]
fn test_cli_parse_config_command() {
    let cli = crates_docs::cli::Cli::try_parse_from([
        "crates-docs",
        "config",
        "--output",
        "/tmp/test-config.toml",
        "--force",
    ]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        crates_docs::cli::Commands::Config { output, force } => {
            assert_eq!(output, PathBuf::from("/tmp/test-config.toml"));
            assert!(force);
        }
        _ => panic!("Expected Config command"),
    }
}

/// 测试 Cli 结构体解析 - Config 命令默认值
#[test]
fn test_cli_parse_config_command_defaults() {
    let cli = crates_docs::cli::Cli::try_parse_from(["crates-docs", "config"]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        crates_docs::cli::Commands::Config { output, force } => {
            assert_eq!(output, PathBuf::from("config.toml"));
            assert!(!force);
        }
        _ => panic!("Expected Config command"),
    }
}

/// 测试 Cli 结构体解析 - Test 命令
#[test]
fn test_cli_parse_test_command() {
    let cli = crates_docs::cli::Cli::try_parse_from([
        "crates-docs",
        "test",
        "--tool",
        "search_crates",
        "--query",
        "serde",
        "--limit",
        "20",
        "--format",
        "json",
    ]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        crates_docs::cli::Commands::Test {
            tool,
            crate_name,
            item_path,
            query,
            version,
            limit,
            format,
        } => {
            assert_eq!(tool, "search_crates");
            assert!(crate_name.is_none());
            assert!(item_path.is_none());
            assert_eq!(query, Some("serde".to_string()));
            assert!(version.is_none());
            assert_eq!(limit, 20);
            assert_eq!(format, "json");
        }
        _ => panic!("Expected Test command"),
    }
}

/// 测试 Cli 结构体解析 - Test 命令默认值
#[test]
fn test_cli_parse_test_command_defaults() {
    let cli = crates_docs::cli::Cli::try_parse_from(["crates-docs", "test"]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        crates_docs::cli::Commands::Test {
            tool,
            limit,
            format,
            ..
        } => {
            assert_eq!(tool, "lookup_crate");
            assert_eq!(limit, 10);
            assert_eq!(format, "markdown");
        }
        _ => panic!("Expected Test command"),
    }
}

/// 测试 Cli 结构体解析 - Test 命令带所有参数
#[test]
fn test_cli_parse_test_command_all_args() {
    let cli = crates_docs::cli::Cli::try_parse_from([
        "crates-docs",
        "test",
        "--tool",
        "lookup_item",
        "--crate-name",
        "serde",
        "--item-path",
        "Deserialize",
        "--version",
        "1.0.0",
        "--limit",
        "5",
        "--format",
        "text",
    ]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        crates_docs::cli::Commands::Test {
            tool,
            crate_name,
            item_path,
            query,
            version,
            limit,
            format,
        } => {
            assert_eq!(tool, "lookup_item");
            assert_eq!(crate_name, Some("serde".to_string()));
            assert_eq!(item_path, Some("Deserialize".to_string()));
            assert!(query.is_none());
            assert_eq!(version, Some("1.0.0".to_string()));
            assert_eq!(limit, 5);
            assert_eq!(format, "text");
        }
        _ => panic!("Expected Test command"),
    }
}

/// 测试 Cli 结构体解析 - Health 命令
#[test]
fn test_cli_parse_health_command() {
    let cli = crates_docs::cli::Cli::try_parse_from([
        "crates-docs",
        "health",
        "--check-type",
        "external",
        "--verbose",
    ]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        crates_docs::cli::Commands::Health {
            check_type,
            verbose,
        } => {
            assert_eq!(check_type, "external");
            assert!(verbose);
        }
        _ => panic!("Expected Health command"),
    }
}

/// 测试 Cli 结构体解析 - Health 命令默认值
#[test]
fn test_cli_parse_health_command_defaults() {
    let cli = crates_docs::cli::Cli::try_parse_from(["crates-docs", "health"]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        crates_docs::cli::Commands::Health {
            check_type,
            verbose,
        } => {
            assert_eq!(check_type, "all");
            assert!(!verbose);
        }
        _ => panic!("Expected Health command"),
    }
}

/// 测试 Cli 结构体解析 - Version 命令
#[test]
fn test_cli_parse_version_command() {
    let cli = crates_docs::cli::Cli::try_parse_from(["crates-docs", "version"]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        crates_docs::cli::Commands::Version => {}
        _ => panic!("Expected Version command"),
    }
}

/// 测试 Cli 全局参数 - config
#[test]
fn test_cli_global_config_option() {
    let cli = crates_docs::cli::Cli::try_parse_from([
        "crates-docs",
        "--config",
        "/custom/config.toml",
        "version",
    ]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    assert_eq!(cli.config, PathBuf::from("/custom/config.toml"));
}

/// 测试 Cli 全局参数 - debug
#[test]
fn test_cli_global_debug_option() {
    let cli = crates_docs::cli::Cli::try_parse_from(["crates-docs", "--debug", "version"]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    assert!(cli.debug);
}

/// 测试 Cli 全局参数 - verbose
#[test]
fn test_cli_global_verbose_option() {
    let cli = crates_docs::cli::Cli::try_parse_from(["crates-docs", "--verbose", "version"]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    assert!(cli.verbose);
}

/// 测试 Cli 全局参数组合
#[test]
fn test_cli_global_options_combined() {
    let cli = crates_docs::cli::Cli::try_parse_from([
        "crates-docs",
        "--config",
        "test.toml",
        "--debug",
        "--verbose",
        "version",
    ]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    assert_eq!(cli.config, PathBuf::from("test.toml"));
    assert!(cli.debug);
    assert!(cli.verbose);
}

/// 测试 Cli 默认配置路径
#[test]
fn test_cli_default_config_path() {
    let cli = crates_docs::cli::Cli::try_parse_from(["crates-docs", "version"]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    assert_eq!(cli.config, PathBuf::from("config.toml"));
    assert!(!cli.debug);
    assert!(!cli.verbose);
}

// ============================================================================
// config_cmd 测试
// ============================================================================

/// 测试 config 命令 - 成功生成配置文件
#[test]
fn test_run_config_command_success() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("test-config.toml");

    let result = crates_docs::cli::run_config_command(&output_path, false);

    assert!(result.is_ok());
    assert!(output_path.exists());
}

/// 测试 config 命令 - 文件已存在但不覆盖
#[test]
fn test_run_config_command_file_exists_no_force() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("existing-config.toml");

    // 先创建文件
    std::fs::write(&output_path, "existing content").unwrap();
    assert!(output_path.exists());

    let result = crates_docs::cli::run_config_command(&output_path, false);

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Config file already exists"));
    assert!(err.contains("--force"));
}

/// 测试 config 命令 - 文件已存在且强制覆盖
#[test]
fn test_run_config_command_file_exists_with_force() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("existing-config.toml");

    // 先创建文件
    std::fs::write(&output_path, "existing content").unwrap();
    assert!(output_path.exists());

    let result = crates_docs::cli::run_config_command(&output_path, true);

    assert!(result.is_ok());
    // 验证文件被覆盖
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("name = \"crates-docs\""));
}

/// 测试 config 命令 - 创建嵌套目录
#[test]
fn test_run_config_command_nested_directory() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("nested/deep/config.toml");

    let result = crates_docs::cli::run_config_command(&output_path, false);

    assert!(result.is_ok());
    assert!(output_path.exists());
}

// ============================================================================
// health_cmd 测试
// ============================================================================

/// 测试 health 命令 - 默认检查类型
#[tokio::test]
async fn test_run_health_command_default() {
    let result = crates_docs::cli::run_health_command("all", false).await;
    assert!(result.is_ok());
}

/// 测试 health 命令 - 详细模式
#[tokio::test]
async fn test_run_health_command_verbose() {
    let result = crates_docs::cli::run_health_command("external", true).await;
    assert!(result.is_ok());
}

/// 测试 health 命令 - 各种检查类型
#[tokio::test]
async fn test_run_health_command_various_types() {
    let check_types = ["all", "external", "internal", "docs_rs", "crates_io"];

    for check_type in check_types {
        let result = crates_docs::cli::run_health_command(check_type, false).await;
        assert!(result.is_ok(), "Failed for check_type: {}", check_type);
    }
}

// ============================================================================
// version_cmd 测试
// ============================================================================

/// 测试 version 命令 - 验证输出包含版本信息
#[test]
fn test_run_version_command() {
    // version 命令只是打印信息，我们验证它不会 panic
    crates_docs::cli::run_version_command();
}

// ============================================================================
// test_cmd 测试
// ============================================================================

/// 测试 test 命令 - 未知工具
#[tokio::test]
async fn test_run_test_command_unknown_tool() {
    let result =
        crates_docs::cli::run_test_command("unknown_tool", None, None, None, None, 10, "markdown")
            .await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Unknown tool"));
}

/// 测试 test 命令 - lookup_crate 缺少 crate_name
#[tokio::test]
async fn test_run_test_command_lookup_crate_missing_name() {
    let result =
        crates_docs::cli::run_test_command("lookup_crate", None, None, None, None, 10, "markdown")
            .await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("--crate-name"));
}

/// 测试 test 命令 - search_crates 缺少 query
#[tokio::test]
async fn test_run_test_command_search_crates_missing_query() {
    let result =
        crates_docs::cli::run_test_command("search_crates", None, None, None, None, 10, "markdown")
            .await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("--query"));
}

/// 测试 test 命令 - lookup_item 缺少参数
#[tokio::test]
async fn test_run_test_command_lookup_item_missing_args() {
    // 缺少 item_path
    let result = crates_docs::cli::run_test_command(
        "lookup_item",
        Some("serde"),
        None,
        None,
        None,
        10,
        "markdown",
    )
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("--crate-name") || err.contains("--item-path"));

    // 缺少 crate_name
    let result = crates_docs::cli::run_test_command(
        "lookup_item",
        None,
        Some("Deserialize"),
        None,
        None,
        10,
        "markdown",
    )
    .await;

    assert!(result.is_err());
}

/// 测试 test 命令 - health_check 工具
#[tokio::test]
async fn test_run_test_command_health_check() {
    let result =
        crates_docs::cli::run_test_command("health_check", None, None, None, None, 10, "markdown")
            .await;

    // health_check 应该成功执行
    assert!(result.is_ok());
}

// ============================================================================
// Commands 枚举测试
// ============================================================================

/// 测试 Commands 枚举变体匹配
#[test]
fn test_commands_enum_variants() {
    // 验证所有命令变体都可以正确创建
    let commands: Vec<crates_docs::cli::Commands> = vec![
        crates_docs::cli::Commands::Serve {
            mode: None,
            host: None,
            port: None,
            enable_oauth: None,
            oauth_client_id: None,
            oauth_client_secret: None,
            oauth_redirect_uri: None,
        },
        crates_docs::cli::Commands::Config {
            output: PathBuf::from("config.toml"),
            force: false,
        },
        crates_docs::cli::Commands::Test {
            tool: "lookup_crate".to_string(),
            crate_name: None,
            item_path: None,
            query: None,
            version: None,
            limit: 10,
            format: "markdown".to_string(),
        },
        crates_docs::cli::Commands::Health {
            check_type: "all".to_string(),
            verbose: false,
        },
        crates_docs::cli::Commands::Version,
    ];

    // 验证每个命令都可以被正确匹配
    for cmd in commands {
        match cmd {
            crates_docs::cli::Commands::Serve { .. } => {}
            crates_docs::cli::Commands::Config { .. } => {}
            crates_docs::cli::Commands::Test { .. } => {}
            crates_docs::cli::Commands::Health { .. } => {}
            crates_docs::cli::Commands::Version => {}
        }
    }
}

// ============================================================================
// Cli 解析错误测试
// ============================================================================

/// 测试 Cli 解析 - 缺少子命令
#[test]
fn test_cli_parse_missing_subcommand() {
    let result = crates_docs::cli::Cli::try_parse_from(["crates-docs"]);
    assert!(result.is_err());
}

/// 测试 Cli 解析 - 无效的子命令
#[test]
fn test_cli_parse_invalid_subcommand() {
    let result = crates_docs::cli::Cli::try_parse_from(["crates-docs", "invalid_command"]);
    assert!(result.is_err());
}

/// 测试 Cli 解析 - 无效的端口号
#[test]
fn test_cli_parse_invalid_port() {
    let result =
        crates_docs::cli::Cli::try_parse_from(["crates-docs", "serve", "--port", "not_a_number"]);
    assert!(result.is_err());
}

/// 测试 Cli 解析 - 无效的 limit
#[test]
fn test_cli_parse_invalid_limit() {
    let result =
        crates_docs::cli::Cli::try_parse_from(["crates-docs", "test", "--limit", "not_a_number"]);
    assert!(result.is_err());
}
