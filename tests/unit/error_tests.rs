//! 错误模块单元测试

use crates_docs::error::Error;
use std::io;

// ============================================================================
// Error 转换测试
// ============================================================================

#[test]
fn test_error_from_io_error() {
    let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
    let err: Error = io_err.into();
    assert!(matches!(err, Error::Io(_)));
}

#[test]
fn test_error_from_json_error() {
    let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
    let err: Error = json_err.into();
    assert!(matches!(err, Error::Json(_)));
}

#[test]
fn test_error_from_url_error() {
    let url_err = url::ParseError::EmptyHost;
    let err: Error = url_err.into();
    assert!(matches!(err, Error::Url(_)));
}

#[test]
fn test_error_from_boxed_error() {
    let boxed: Box<dyn std::error::Error + Send + Sync> = Box::new(io::Error::other("test error"));
    let err: Error = boxed.into();
    assert!(err.to_string().contains("test error"));
}

#[test]
fn test_error_from_anyhow_error() {
    let anyhow_err = anyhow::anyhow!("anyhow error");
    let err: Error = anyhow_err.into();
    assert!(err.to_string().contains("anyhow error"));
}

// ============================================================================
// Error Display 测试
// ============================================================================

#[test]
fn test_error_display() {
    let err = Error::Config("test config error".to_string());
    assert!(err.to_string().contains("test config error"));

    let err = Error::Cache("test cache error".to_string());
    assert!(err.to_string().contains("test cache error"));

    let err = Error::Mcp("test mcp error".to_string());
    assert!(err.to_string().contains("test mcp error"));
}

#[test]
fn test_error_variants_display() {
    // 测试各种错误类型的 Display
    let err = Error::Config("config error".to_string());
    assert!(!err.to_string().is_empty());

    let err = Error::Cache("cache error".to_string());
    assert!(!err.to_string().is_empty());

    let err = Error::HttpRequest("http error".to_string());
    assert!(!err.to_string().is_empty());

    let err = Error::Json(serde_json::from_str::<serde_json::Value>("bad").unwrap_err());
    assert!(!err.to_string().is_empty());

    let err = Error::Io(io::Error::other("io error"));
    assert!(!err.to_string().is_empty());

    let err = Error::Url(url::ParseError::EmptyHost);
    assert!(!err.to_string().is_empty());

    let err = Error::Mcp("mcp error".to_string());
    assert!(!err.to_string().is_empty());

    let err = Error::Initialization("init error".to_string());
    assert!(!err.to_string().is_empty());

    let err = Error::Auth("auth error".to_string());
    assert!(!err.to_string().is_empty());

    let err = Error::Parse("parse error".to_string());
    assert!(!err.to_string().is_empty());

    let err = Error::Other("other error".to_string());
    assert!(!err.to_string().is_empty());
}

// ============================================================================
// Result 类型测试
// ============================================================================

#[test]
fn test_result_type() {
    fn returns_result() -> crates_docs::Result<()> {
        Ok(())
    }

    let result = returns_result();
    assert!(result.is_ok());
}
