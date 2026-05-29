//! Configuration module unit tests

use crates_docs::config::AppConfig;
use crates_docs::config::{EnvLoggingConfig, ServerConfig};
use tempfile::tempdir;

// ============================================================================
// Configuration validation tests
// ============================================================================

#[test]
fn test_config_validation_empty_host() {
    let mut config = AppConfig::default();
    config.server.host = "".to_string();
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("host"));
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
// File load/save tests
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
// Environment variable loading tests
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
            let env_config = AppConfig::from_env().unwrap();
            // from_env returns EnvAppConfig - need to merge to get AppConfig
            let config = AppConfig::merge(None, Some(env_config));
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
// Configuration merge tests
// ============================================================================

#[test]
fn test_config_merge() {
    use crates_docs::config::{EnvAppConfig, EnvServerConfig};

    let mut file_config = AppConfig::default();
    file_config.server.name = "file-server".to_string();
    file_config.server.port = 7000;

    // Create EnvAppConfig with explicit overrides
    let env_config = EnvAppConfig {
        server: EnvServerConfig {
            name: Some("env-server".to_string()),
            host: None,
            port: Some(9000),
            transport_mode: None,
        },
        logging: Default::default(),
        #[cfg(feature = "api-key")]
        auth_api_key: Default::default(),
    };

    let merged = AppConfig::merge(Some(file_config), Some(env_config));
    // Environment variables take priority
    assert_eq!(merged.server.name, "env-server");
    assert_eq!(merged.server.port, 9000);
}

// ============================================================================
// Default value tests
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
    // By default enable_file is false (output to console only)
    assert!(!config.enable_file);
}

#[test]
fn test_performance_config_default() {
    let config = crates_docs::config::PerformanceConfig::default();
    assert!(config.http_client_pool_size > 0);
    assert!(config.cache_max_size > 0);
    // Test new HTTP client config fields
    assert!(config.http_client_pool_idle_timeout_secs > 0);
    assert!(config.http_client_connect_timeout_secs > 0);
    assert!(config.http_client_timeout_secs > 0);
    assert!(config.http_client_read_timeout_secs > 0);
    assert!(config.http_client_max_retries > 0);
    assert!(config.http_client_retry_initial_delay_ms > 0);
    assert!(config.http_client_retry_max_delay_ms > 0);
    assert!(config.enable_metrics);
}

#[test]
fn test_config_validation_zero_pool_idle_timeout() {
    let mut config = AppConfig::default();
    config.performance.http_client_pool_idle_timeout_secs = 0;
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_config_validation_zero_connect_timeout() {
    let mut config = AppConfig::default();
    config.performance.http_client_connect_timeout_secs = 0;
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_config_validation_zero_request_timeout() {
    let mut config = AppConfig::default();
    config.performance.http_client_timeout_secs = 0;
    let result = config.validate();
    assert!(result.is_err());
}

// ============================================================================
// Environment variable logging tests
// ============================================================================

#[test]
fn test_config_from_env_logging_vars() {
    temp_env::with_vars(
        [
            ("CRATES_DOCS_ENABLE_CONSOLE", Some("true")),
            ("CRATES_DOCS_ENABLE_FILE", Some("false")),
        ],
        || {
            let env_config = AppConfig::from_env().unwrap();
            assert_eq!(env_config.logging.enable_console, Some(true));
            assert_eq!(env_config.logging.enable_file, Some(false));
        },
    );
}

#[test]
fn test_config_from_env_invalid_console() {
    temp_env::with_vars([("CRATES_DOCS_ENABLE_CONSOLE", Some("notbool"))], || {
        let env_config = AppConfig::from_env().unwrap();
        // Invalid bool parse should result in None
        assert_eq!(env_config.logging.enable_console, None);
    });
}

#[test]
fn test_config_merge_logging_env_overrides() {
    use crates_docs::config::{EnvAppConfig, EnvLoggingConfig, EnvServerConfig};

    let env_config = EnvAppConfig {
        server: EnvServerConfig::default(),
        logging: EnvLoggingConfig {
            level: Some("debug".to_string()),
            enable_console: Some(false),
            enable_file: Some(true),
        },
        #[cfg(feature = "api-key")]
        auth_api_key: Default::default(),
    };

    let merged = AppConfig::merge(None, Some(env_config));
    assert_eq!(merged.logging.level, "debug");
    assert!(!merged.logging.enable_console);
    assert!(merged.logging.enable_file);
}

#[test]
fn test_config_merge_no_env_returns_default() {
    let merged = AppConfig::merge(None, None);
    assert_eq!(merged.server.name, "crates-docs");
}

// ============================================================================
// default_version function test
// ============================================================================

#[test]
fn test_default_version_matches_crate_version() {
    let config = ServerConfig::default();
    assert_eq!(config.version, crates_docs::VERSION);
}

// ============================================================================
// save_to_file error path tests
// ============================================================================

#[test]
fn test_config_save_to_file_serialization_error() {
    use crates_docs::config::AppConfig;
    use tempfile::tempdir;

    // Create a config that might cause issues with serialization
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");

    // Normal config should serialize fine
    let config = AppConfig::default();
    let result = config.save_to_file(&path);
    assert!(result.is_ok());
}

// ============================================================================
// API key environment variable tests (feature-gated)
// ============================================================================

#[cfg(feature = "api-key")]
#[test]
fn test_config_from_env_api_key_vars() {
    temp_env::with_vars(
        [
            ("CRATES_DOCS_API_KEY_ENABLED", Some("true")),
            ("CRATES_DOCS_API_KEYS", Some("key1,key2,key3")),
            ("CRATES_DOCS_API_KEY_HEADER", Some("X-Custom-Key")),
            ("CRATES_DOCS_API_KEY_QUERY_PARAM_NAME", Some("token")),
            ("CRATES_DOCS_API_KEY_ALLOW_QUERY", Some("true")),
            ("CRATES_DOCS_API_KEY_PREFIX", Some("pk")),
        ],
        || {
            let env_config = AppConfig::from_env().unwrap();
            assert_eq!(env_config.auth_api_key.enabled, Some(true));
            assert_eq!(
                env_config.auth_api_key.keys,
                Some(vec![
                    "key1".to_string(),
                    "key2".to_string(),
                    "key3".to_string()
                ])
            );
            assert_eq!(
                env_config.auth_api_key.header_name,
                Some("X-Custom-Key".to_string())
            );
            assert_eq!(
                env_config.auth_api_key.query_param_name,
                Some("token".to_string())
            );
            assert_eq!(env_config.auth_api_key.allow_query_param, Some(true));
            assert_eq!(env_config.auth_api_key.key_prefix, Some("pk".to_string()));
        },
    );
}

#[cfg(feature = "api-key")]
#[test]
fn test_config_from_env_api_key_invalid_bool() {
    temp_env::with_vars(
        [
            ("CRATES_DOCS_API_KEY_ENABLED", Some("not-a-bool")),
            ("CRATES_DOCS_API_KEY_ALLOW_QUERY", Some("invalid")),
        ],
        || {
            let env_config = AppConfig::from_env().unwrap();
            // Invalid bool should result in None
            assert_eq!(env_config.auth_api_key.enabled, None);
            assert_eq!(env_config.auth_api_key.allow_query_param, None);
        },
    );
}

#[cfg(feature = "api-key")]
#[test]
fn test_config_merge_api_key_env_overrides() {
    use crates_docs::config::{EnvApiKeyConfig, EnvAppConfig, EnvServerConfig};

    let env_config = EnvAppConfig {
        server: EnvServerConfig::default(),
        logging: EnvLoggingConfig::default(),
        auth_api_key: EnvApiKeyConfig {
            enabled: Some(true),
            keys: Some(vec!["env-key".to_string()]),
            header_name: Some("X-Env-Key".to_string()),
            query_param_name: Some("api_token".to_string()),
            allow_query_param: Some(true),
            key_prefix: Some("env".to_string()),
        },
    };

    let merged = AppConfig::merge(None, Some(env_config));
    assert!(merged.auth.api_key.enabled);
    assert_eq!(merged.auth.api_key.keys, vec!["env-key"]);
    assert_eq!(merged.auth.api_key.header_name, "X-Env-Key");
    assert_eq!(merged.auth.api_key.query_param_name, "api_token");
    assert!(merged.auth.api_key.allow_query_param);
    assert_eq!(merged.auth.api_key.key_prefix, "env");
}

#[cfg(feature = "api-key")]
#[test]
fn test_config_merge_api_key_partial_override() {
    use crates_docs::config::{EnvApiKeyConfig, EnvAppConfig, EnvServerConfig};

    let mut file_config = AppConfig::default();
    file_config.auth.api_key.enabled = true;
    file_config.auth.api_key.keys = vec!["file-key".to_string()];
    file_config.auth.api_key.header_name = "X-File-Key".to_string();

    // Only override enabled, leave other fields as file values
    let env_config = EnvAppConfig {
        server: EnvServerConfig::default(),
        logging: EnvLoggingConfig::default(),
        auth_api_key: EnvApiKeyConfig {
            enabled: Some(false),
            keys: None,
            header_name: None,
            query_param_name: None,
            allow_query_param: None,
            key_prefix: None,
        },
    };

    let merged = AppConfig::merge(Some(file_config), Some(env_config));
    assert!(!merged.auth.api_key.enabled);
    // Keys should remain from file config since env was None
    assert_eq!(merged.auth.api_key.keys, vec!["file-key"]);
    assert_eq!(merged.auth.api_key.header_name, "X-File-Key");
}

// ============================================================================
// Partial configuration parsing (regression: BUG7 - missing field errors)
// ============================================================================

#[test]
fn test_parse_only_auth_api_key_section() {
    // Providing only [auth.api_key] must not fail with "missing field".
    let toml_str = r#"
[auth.api_key]
enabled = true
keys = ["$argon2id$v=19$m=47104,t=1,p=1$c2FsdA$hash1"]
"#;
    let config: AppConfig = toml::from_str(toml_str).expect("partial config should parse");
    assert!(config.auth.api_key.enabled);
    assert_eq!(config.auth.api_key.keys.len(), 1);
    // Untouched sections fall back to defaults.
    assert_eq!(config.server.name, ServerConfig::default().name);
    assert_eq!(config.server.port, ServerConfig::default().port);
}

#[test]
fn test_parse_only_server_port() {
    // A user changing just the port should not need to restate every field.
    let toml_str = r#"
[server]
port = 9999
"#;
    let config: AppConfig = toml::from_str(toml_str).expect("partial server should parse");
    assert_eq!(config.server.port, 9999);
    assert_eq!(config.server.host, ServerConfig::default().host);
    assert_eq!(config.server.name, ServerConfig::default().name);
}

#[test]
fn test_parse_empty_config_uses_defaults() {
    let config: AppConfig = toml::from_str("").expect("empty config should parse");
    assert_eq!(config.server.port, ServerConfig::default().port);
    assert!(!config.auth.api_key.enabled);
}

#[test]
fn test_parse_only_logging_level() {
    let toml_str = r#"
[logging]
level = "debug"
"#;
    let config: AppConfig = toml::from_str(toml_str).expect("partial logging should parse");
    assert_eq!(config.logging.level, "debug");
    assert!(config.logging.enable_console);
}
