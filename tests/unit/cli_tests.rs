//! CLI module unit tests

use clap::Parser;
use std::path::PathBuf;

// ============================================================================
// Cli struct tests
// ============================================================================

/// Test Cli struct parsing - Serve command
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
            enable_api_key,
            api_keys,
            api_key_header,
            api_key_query_param,
        } => {
            assert_eq!(mode, Some("http".to_string()));
            assert_eq!(host, Some("0.0.0.0".to_string()));
            assert_eq!(port, Some(9090));
            assert!(enable_oauth.is_none());
            assert!(oauth_client_id.is_none());
            assert!(oauth_client_secret.is_none());
            assert!(oauth_redirect_uri.is_none());
            assert!(enable_api_key.is_none());
            assert!(api_keys.is_none());
            assert!(api_key_header.is_none());
            assert!(api_key_query_param.is_none());
        }
        _ => panic!("Expected Serve command"),
    }
}

/// Test Cli struct parsing - Serve command with OAuth parameters
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

/// Test Cli struct parsing - Config command
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

/// Test Cli struct parsing - Config command defaults
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

/// Test Cli struct parsing - Test command
#[test]
fn test_cli_parse_test_command() {
    let cli = crates_docs::cli::Cli::try_parse_from([
        "crates-docs",
        "test",
        "--tool",
        "search_crates",
        "--query",
        "serde",
        "--sort",
        "downloads",
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
            sort,
            version,
            limit,
            format,
        } => {
            assert_eq!(tool, "search_crates");
            assert!(crate_name.is_none());
            assert!(item_path.is_none());
            assert_eq!(query, Some("serde".to_string()));
            assert_eq!(sort, Some("downloads".to_string()));
            assert!(version.is_none());
            assert_eq!(limit, 20);
            assert_eq!(format, "json");
        }
        _ => panic!("Expected Test command"),
    }
}

/// Test Cli struct parsing - Test command defaults
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

/// Test Cli struct parsing - Test command with all arguments
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
            sort,
            version,
            limit,
            format,
        } => {
            assert_eq!(tool, "lookup_item");
            assert_eq!(crate_name, Some("serde".to_string()));
            assert_eq!(item_path, Some("Deserialize".to_string()));
            assert!(query.is_none());
            assert!(sort.is_none());
            assert_eq!(version, Some("1.0.0".to_string()));
            assert_eq!(limit, 5);
            assert_eq!(format, "text");
        }
        _ => panic!("Expected Test command"),
    }
}

/// Test Cli struct parsing - Health command
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

/// Test Cli struct parsing - Health command defaults
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

/// Test Cli struct parsing - GenerateApiKey command
#[test]
fn test_cli_parse_generate_api_key_command() {
    let cli = crates_docs::cli::Cli::try_parse_from([
        "crates-docs",
        "generate-api-key",
        "--prefix",
        "ck",
    ]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        crates_docs::cli::Commands::GenerateApiKey { prefix } => {
            assert_eq!(prefix, "ck");
        }
        _ => panic!("Expected GenerateApiKey command"),
    }
}

/// Test Cli struct parsing - GenerateApiKey command defaults
#[test]
fn test_cli_parse_generate_api_key_command_defaults() {
    let cli = crates_docs::cli::Cli::try_parse_from(["crates-docs", "generate-api-key"]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        crates_docs::cli::Commands::GenerateApiKey { prefix } => {
            assert_eq!(prefix, "sk");
        }
        _ => panic!("Expected GenerateApiKey command"),
    }
}

/// Test Cli struct parsing - Version command
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

/// Test Cli global option - config
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

/// Test Cli global option - debug
#[test]
fn test_cli_global_debug_option() {
    let cli = crates_docs::cli::Cli::try_parse_from(["crates-docs", "--debug", "version"]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    assert!(cli.debug);
}

/// Test Cli global option - verbose
#[test]
fn test_cli_global_verbose_option() {
    let cli = crates_docs::cli::Cli::try_parse_from(["crates-docs", "--verbose", "version"]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    assert!(cli.verbose);
}

/// Test Cli global options combined
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

/// Test Cli default config path
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
// config_cmd tests
// ============================================================================

/// Test config command - successfully generate config file
#[test]
fn test_run_config_command_success() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("test-config.toml");

    let result = crates_docs::cli::run_config_command(&output_path, false);

    assert!(result.is_ok());
    assert!(output_path.exists());
}

/// Test config command - file exists but no overwrite
#[test]
fn test_run_config_command_file_exists_no_force() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("existing-config.toml");

    // Create the file first
    std::fs::write(&output_path, "existing content").unwrap();
    assert!(output_path.exists());

    let result = crates_docs::cli::run_config_command(&output_path, false);

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Config file already exists"));
    assert!(err.contains("--force"));
}

/// Test config command - file exists with force overwrite
#[test]
fn test_run_config_command_file_exists_with_force() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("existing-config.toml");

    // Create the file first
    std::fs::write(&output_path, "existing content").unwrap();
    assert!(output_path.exists());

    let result = crates_docs::cli::run_config_command(&output_path, true);

    assert!(result.is_ok());
    // Verify file was overwritten
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("name = \"crates-docs\""));
}

/// Test config command - create nested directory
#[test]
fn test_run_config_command_nested_directory() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("nested/deep/config.toml");

    let result = crates_docs::cli::run_config_command(&output_path, false);

    assert!(result.is_ok());
    assert!(output_path.exists());
}

// ============================================================================
// health_cmd tests
// ============================================================================

/// Test health command - default check type
#[tokio::test]
async fn test_run_health_command_default() {
    let result = crates_docs::cli::run_health_command("all", false).await;
    assert!(result.is_ok());
}

/// Test health command - verbose mode
#[tokio::test]
async fn test_run_health_command_verbose() {
    let result = crates_docs::cli::run_health_command("external", true).await;
    assert!(result.is_ok());
}

/// Test health command - various check types
#[tokio::test]
async fn test_run_health_command_various_types() {
    let check_types = ["all", "external", "internal", "docs_rs", "crates_io"];

    for check_type in check_types {
        let result = crates_docs::cli::run_health_command(check_type, false).await;
        assert!(result.is_ok(), "Failed for check_type: {}", check_type);
    }
}

// ============================================================================
// version_cmd tests
// ============================================================================

/// Test version command - verify output contains version info
#[test]
fn test_run_version_command() {
    // version command just prints info, we verify it doesn't panic
    crates_docs::cli::run_version_command();
}

// ============================================================================
// test_cmd tests
// ============================================================================

/// Test test command - unknown tool
#[tokio::test]
async fn test_run_test_command_unknown_tool() {
    let result = crates_docs::cli::run_test_command(
        "unknown_tool",
        None,
        None,
        None,
        None,
        None,
        10,
        "markdown",
    )
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Unknown tool"));
}

/// Test test command - lookup_crate missing crate_name
#[tokio::test]
async fn test_run_test_command_lookup_crate_missing_name() {
    let result = crates_docs::cli::run_test_command(
        "lookup_crate",
        None,
        None,
        None,
        None,
        None,
        10,
        "markdown",
    )
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("--crate-name"));
}

/// Test test command - search_crates missing query
#[tokio::test]
async fn test_run_test_command_search_crates_missing_query() {
    let result = crates_docs::cli::run_test_command(
        "search_crates",
        None,
        None,
        None,
        None,
        None,
        10,
        "markdown",
    )
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("--query"));
}

/// Test test command - lookup_item missing arguments
#[tokio::test]
async fn test_run_test_command_lookup_item_missing_args() {
    // Missing item_path
    let result = crates_docs::cli::run_test_command(
        "lookup_item",
        Some("serde"),
        None,
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

    // Missing crate_name
    let result = crates_docs::cli::run_test_command(
        "lookup_item",
        None,
        Some("Deserialize"),
        None,
        None,
        None,
        10,
        "markdown",
    )
    .await;

    assert!(result.is_err());
}

/// Test test command - health_check tool
#[tokio::test]
async fn test_run_test_command_health_check() {
    let result = crates_docs::cli::run_test_command(
        "health_check",
        None,
        None,
        None,
        None,
        None,
        10,
        "markdown",
    )
    .await;

    // health_check should execute successfully
    assert!(result.is_ok());
}

/// Test test command - search_crates accepts sort parameter
#[tokio::test]
async fn test_run_test_command_search_crates_with_sort() {
    let result = crates_docs::cli::run_test_command(
        "search_crates",
        None,
        None,
        Some("serde"),
        Some("downloads"),
        None,
        1,
        "json",
    )
    .await;

    assert!(result.is_ok());
}

// ============================================================================
// Commands enum tests
// ============================================================================

/// Test Commands enum variant matching
#[test]
fn test_commands_enum_variants() {
    // Verify all command variants can be created correctly
    let commands: Vec<crates_docs::cli::Commands> = vec![
        crates_docs::cli::Commands::Serve {
            mode: None,
            host: None,
            port: None,
            enable_oauth: None,
            oauth_client_id: None,
            oauth_client_secret: None,
            oauth_redirect_uri: None,
            enable_api_key: None,
            api_keys: None,
            api_key_header: None,
            api_key_query_param: None,
        },
        crates_docs::cli::Commands::GenerateApiKey {
            prefix: "sk".to_string(),
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
            sort: None,
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

    // Verify each command can be matched correctly
    for cmd in commands {
        match cmd {
            crates_docs::cli::Commands::Serve { .. } => {}
            crates_docs::cli::Commands::GenerateApiKey { .. } => {}
            crates_docs::cli::Commands::ListApiKeys { .. } => {}
            crates_docs::cli::Commands::RevokeApiKey { .. } => {}
            crates_docs::cli::Commands::Config { .. } => {}
            crates_docs::cli::Commands::Test { .. } => {}
            crates_docs::cli::Commands::Health { .. } => {}
            crates_docs::cli::Commands::Version => {}
        }
    }
}

// ============================================================================
// Cli parsing error tests
// ============================================================================

/// Test Cli parsing - missing subcommand
#[test]
fn test_cli_parse_missing_subcommand() {
    let result = crates_docs::cli::Cli::try_parse_from(["crates-docs"]);
    assert!(result.is_err());
}

/// Test Cli parsing - invalid subcommand
#[test]
fn test_cli_parse_invalid_subcommand() {
    let result = crates_docs::cli::Cli::try_parse_from(["crates-docs", "invalid_command"]);
    assert!(result.is_err());
}

/// Test Cli parsing - invalid port number
#[test]
fn test_cli_parse_invalid_port() {
    let result =
        crates_docs::cli::Cli::try_parse_from(["crates-docs", "serve", "--port", "not_a_number"]);
    assert!(result.is_err());
}

/// Test Cli parsing - invalid limit
#[test]
fn test_cli_parse_invalid_limit() {
    let result =
        crates_docs::cli::Cli::try_parse_from(["crates-docs", "test", "--limit", "not_a_number"]);
    assert!(result.is_err());
}

/// Test Cli parsing - search_crates sort parameter
#[test]
fn test_cli_parse_test_command_with_sort() {
    let cli = crates_docs::cli::Cli::try_parse_from([
        "crates-docs",
        "test",
        "--tool",
        "search_crates",
        "--query",
        "mcp",
        "--sort",
        "recent-downloads",
    ]);

    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        crates_docs::cli::Commands::Test { query, sort, .. } => {
            assert_eq!(query, Some("mcp".to_string()));
            assert_eq!(sort, Some("recent-downloads".to_string()));
        }
        _ => panic!("Expected Test command"),
    }
}
