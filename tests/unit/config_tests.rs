//! 配置模块单元测试

use crates_docs::config::AppConfig;
use tempfile::tempdir;

// ============================================================================
// 配置验证测试
// ============================================================================

#[test]
fn test_config_validation_empty_host() {
    let mut config = AppConfig::default();
    config.server.host = "".to_string();
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Server host"));
}

#[test]
fn test_config_validation_zero_port() {
    let mut config = AppConfig::default();
    config.server.port = 0;
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_config_validation_invalid_transport_mode() {
    let mut config = AppConfig::default();
    config.server.transport_mode = "invalid".to_string();
    let result = config.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid transport mode"));
}

#[test]
fn test_config_validation_invalid_log_level() {
    let mut config = AppConfig::default();
    config.logging.level = "invalid".to_string();
    let result = config.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid log level"));
}

#[test]
fn test_config_validation_zero_max_connections() {
    let mut config = AppConfig::default();
    config.server.max_connections = 0;
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_config_validation_zero_pool_size() {
    let mut config = AppConfig::default();
    config.performance.http_client_pool_size = 0;
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_config_validation_zero_cache_size() {
    let mut config = AppConfig::default();
    config.performance.cache_max_size = 0;
    let result = config.validate();
    assert!(result.is_err());
}

// ============================================================================
// 文件加载/保存测试
// ============================================================================

#[test]
fn test_config_save_and_load() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");

    let config = AppConfig::default();
    config.save_to_file(&path).unwrap();
    assert!(path.exists());

    let loaded = AppConfig::from_file(&path).unwrap();
    assert_eq!(loaded.server.name, config.server.name);
    assert_eq!(loaded.server.host, config.server.host);
    assert_eq!(loaded.server.port, config.server.port);
}

#[test]
fn test_config_from_file_invalid_toml() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bad.toml");
    std::fs::write(&path, "this = [invalid toml").unwrap();

    let result = AppConfig::from_file(&path);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Failed to parse config file"));
}

#[test]
fn test_config_from_file_missing_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("missing.toml");
    let result = AppConfig::from_file(&path);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Failed to read config file"));
}

#[test]
fn test_config_save_to_file_nested_directory() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nested/config/app.toml");
    let config = AppConfig::default();

    config.save_to_file(&path).unwrap();
    assert!(path.exists());

    let loaded = AppConfig::from_file(&path).unwrap();
    assert_eq!(loaded.server.host, config.server.host);
    assert_eq!(loaded.server.port, config.server.port);
}

// ============================================================================
// 环境变量加载测试
// ============================================================================

#[test]
fn test_config_from_env() {
    temp_env::with_vars(
        [
            ("CRATES_DOCS_NAME", Some("custom-server")),
            ("CRATES_DOCS_HOST", Some("0.0.0.0")),
            ("CRATES_DOCS_PORT", Some("9000")),
            ("CRATES_DOCS_TRANSPORT_MODE", Some("http")),
            ("CRATES_DOCS_LOG_LEVEL", Some("debug")),
        ],
        || {
            let config = AppConfig::from_env().unwrap();
            assert_eq!(config.server.name, "custom-server");
            assert_eq!(config.server.host, "0.0.0.0");
            assert_eq!(config.server.port, 9000);
            assert_eq!(config.server.transport_mode, "http");
            assert_eq!(config.logging.level, "debug");
        },
    );
}

#[test]
fn test_config_from_env_invalid_port() {
    temp_env::with_vars([("CRATES_DOCS_PORT", Some("not-a-number"))], || {
        let result = AppConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid port"));
    });
}

// ============================================================================
// 配置合并测试
// ============================================================================

#[test]
fn test_config_merge() {
    let mut file_config = AppConfig::default();
    file_config.server.name = "file-server".to_string();
    file_config.server.port = 7000;

    let mut env_config = AppConfig::default();
    env_config.server.name = "env-server".to_string();
    env_config.server.port = 9000;

    let merged = AppConfig::merge(Some(file_config), Some(env_config));
    // 环境变量优先
    assert_eq!(merged.server.name, "env-server");
    assert_eq!(merged.server.port, 9000);
}

// ============================================================================
// 默认值测试
// ============================================================================

#[test]
fn test_app_config_default() {
    let config = AppConfig::default();
    assert_eq!(config.server.name, "crates-docs");
    assert_eq!(config.server.host, "127.0.0.1");
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.server.transport_mode, "hybrid");
    assert_eq!(config.logging.level, "info");
}

#[test]
fn test_server_config_default() {
    let config = crates_docs::config::ServerConfig::default();
    assert_eq!(config.name, "crates-docs");
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 8080);
}

#[test]
fn test_logging_config_default() {
    let config = crates_docs::config::LoggingConfig::default();
    assert_eq!(config.level, "info");
    assert!(config.enable_console);
    // 默认 enable_file 为 false（仅输出到控制台）
    assert!(!config.enable_file);
}

#[test]
fn test_performance_config_default() {
    let config = crates_docs::config::PerformanceConfig::default();
    assert!(config.http_client_pool_size > 0);
    assert!(config.cache_max_size > 0);
}
