//! Library-level tests

use crates_docs::{NAME, VERSION};

// ============================================================================
// Constant tests
// ============================================================================

#[test]
fn test_version_constant() {
    // Verify version is not empty and format is correct
    assert!(!VERSION.is_empty());
    // Version should follow semver format
    let parts: Vec<&str> = VERSION.split('.').collect();
    assert!(parts.len() >= 2, "Version should have at least major.minor");
}

#[test]
fn test_name_constant() {
    assert_eq!(NAME, "crates-docs");
}

#[test]
fn test_user_agent_identifies_app_and_contact() {
    let ua = crates_docs::user_agent();
    assert!(ua.starts_with("CratesDocsMCP/"), "UA missing app id: {ua}");
    assert!(ua.contains(VERSION), "UA missing version: {ua}");
    // crates.io's data-access policy wants a contact handle; the repository URL
    // is embedded in parentheses when available.
    if !crates_docs::REPOSITORY.is_empty() {
        assert!(
            ua.contains(crates_docs::REPOSITORY),
            "UA missing contact/repository: {ua}"
        );
        assert!(ua.contains("https://"), "UA contact is not a URL: {ua}");
    }
}

// ============================================================================
// Re-exports tests
// ============================================================================

#[test]
fn test_error_reexport() {
    let err = crates_docs::Error::config("field", "test");
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
// init_logging_with_config tests
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
    // Logging initialization is global, multiple calls will fail, just verify no panic
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
    // Invalid level should default to info
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
    // Neither console nor file logging enabled
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
    // File logging only - use temp directory
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
    // Enable both console and file logging
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
    // File logging only but no path - use default path
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
