//! 库级别测试

use crates_docs::{NAME, VERSION};

// ============================================================================
// 常量测试
// ============================================================================

#[test]
fn test_version_constant() {
    // 验证版本号不为空且格式正确
    assert!(!VERSION.is_empty());
    // 版本号应该符合 semver 格式
    let parts: Vec<&str> = VERSION.split('.').collect();
    assert!(parts.len() >= 2, "Version should have at least major.minor");
}

#[test]
fn test_name_constant() {
    assert_eq!(NAME, "crates-docs");
}

// ============================================================================
// Re-exports 测试
// ============================================================================

#[test]
fn test_error_reexport() {
    let err = crates_docs::Error::Config("test".to_string());
    assert!(!err.to_string().is_empty());
}

#[test]
fn test_result_reexport() {
    fn returns_result() -> crates_docs::Result<()> {
        Ok(())
    }
    assert!(returns_result().is_ok());
}

#[test]
fn test_server_config_reexport() {
    let config = crates_docs::ServerConfig::default();
    assert_eq!(config.name, "crates-docs");
}

// ============================================================================
// init_logging_with_config 测试
// ============================================================================

#[test]
fn test_init_logging_with_console_only() {
    let config = crates_docs::config::LoggingConfig {
        level: "info".to_string(),
        file_path: None,
        enable_console: true,
        enable_file: false,
        max_file_size_mb: 100,
        max_files: 10,
    };
    // 日志初始化是全局的，多次调用会失败，这里只验证不 panic
    let _ = crates_docs::init_logging_with_config(&config);
}

#[test]
fn test_init_logging_with_debug_level() {
    let config = crates_docs::config::LoggingConfig {
        level: "debug".to_string(),
        file_path: None,
        enable_console: true,
        enable_file: false,
        max_file_size_mb: 100,
        max_files: 10,
    };
    let _ = crates_docs::init_logging_with_config(&config);
}

#[test]
fn test_init_logging_with_trace_level() {
    let config = crates_docs::config::LoggingConfig {
        level: "trace".to_string(),
        file_path: None,
        enable_console: true,
        enable_file: false,
        max_file_size_mb: 100,
        max_files: 10,
    };
    let _ = crates_docs::init_logging_with_config(&config);
}

#[test]
fn test_init_logging_with_warn_level() {
    let config = crates_docs::config::LoggingConfig {
        level: "warn".to_string(),
        file_path: None,
        enable_console: true,
        enable_file: false,
        max_file_size_mb: 100,
        max_files: 10,
    };
    let _ = crates_docs::init_logging_with_config(&config);
}

#[test]
fn test_init_logging_with_error_level() {
    let config = crates_docs::config::LoggingConfig {
        level: "error".to_string(),
        file_path: None,
        enable_console: true,
        enable_file: false,
        max_file_size_mb: 100,
        max_files: 10,
    };
    let _ = crates_docs::init_logging_with_config(&config);
}

#[test]
fn test_init_logging_with_invalid_level() {
    // 无效级别应该默认为 info
    let config = crates_docs::config::LoggingConfig {
        level: "invalid".to_string(),
        file_path: None,
        enable_console: true,
        enable_file: false,
        max_file_size_mb: 100,
        max_files: 10,
    };
    let _ = crates_docs::init_logging_with_config(&config);
}

#[test]
fn test_init_logging_no_console_no_file() {
    // 不启用控制台也不启用文件
    let config = crates_docs::config::LoggingConfig {
        level: "info".to_string(),
        file_path: None,
        enable_console: false,
        enable_file: false,
        max_file_size_mb: 100,
        max_files: 10,
    };
    let _ = crates_docs::init_logging_with_config(&config);
}

#[test]
fn test_init_logging_with_file_only() {
    // 仅文件日志 - 使用临时目录
    let temp_dir = tempfile::tempdir().unwrap();
    let log_path = temp_dir
        .path()
        .join("test.log")
        .to_string_lossy()
        .to_string();

    let config = crates_docs::config::LoggingConfig {
        level: "info".to_string(),
        file_path: Some(log_path),
        enable_console: false,
        enable_file: true,
        max_file_size_mb: 100,
        max_files: 10,
    };
    let _ = crates_docs::init_logging_with_config(&config);
}

#[test]
fn test_init_logging_with_console_and_file() {
    // 同时启用控制台和文件日志
    let temp_dir = tempfile::tempdir().unwrap();
    let log_path = temp_dir
        .path()
        .join("combined.log")
        .to_string_lossy()
        .to_string();

    let config = crates_docs::config::LoggingConfig {
        level: "info".to_string(),
        file_path: Some(log_path),
        enable_console: true,
        enable_file: true,
        max_file_size_mb: 100,
        max_files: 10,
    };
    let _ = crates_docs::init_logging_with_config(&config);
}

#[test]
fn test_init_logging_file_only_no_path() {
    // 仅文件日志但没有路径 - 使用默认路径
    let config = crates_docs::config::LoggingConfig {
        level: "info".to_string(),
        file_path: None,
        enable_console: false,
        enable_file: true,
        max_file_size_mb: 100,
        max_files: 10,
    };
    let _ = crates_docs::init_logging_with_config(&config);
}
