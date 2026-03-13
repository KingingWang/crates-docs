//! 服务器模块单元测试

use crates_docs::{AppConfig, CratesDocsServer};

// ============================================================================
// ServerConfig 测试
// ============================================================================

#[test]
fn test_server_config_default() {
    let config = AppConfig::default();
    assert_eq!(config.server.name, "crates-docs");
    assert!(!config.server.version.is_empty());
    assert_eq!(config.server.host, "127.0.0.1");
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.server.transport_mode, "hybrid");
}

// ============================================================================
// CratesDocsServer 测试
// ============================================================================

#[test]
fn test_server_new() {
    let config = AppConfig::default();
    let server = CratesDocsServer::new(config.clone()).unwrap();
    assert_eq!(server.config().server.name, config.server.name);
    assert!(server.tool_registry().get_tools().len() >= 4);
}

#[tokio::test]
async fn test_server_new_async() {
    let config = AppConfig::default();
    let server = CratesDocsServer::new_async(config.clone()).await.unwrap();
    assert_eq!(server.config().server.name, config.server.name);
    assert!(server.tool_registry().get_tools().len() >= 4);
}

#[test]
fn test_server_new_async_and_accessors() {
    let config = AppConfig::default();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt
        .block_on(async { CratesDocsServer::new_async(config.clone()).await })
        .unwrap();

    assert_eq!(server.config().server.name, config.server.name);
    assert!(server.tool_registry().get_tools().len() >= 4);
    assert!(!server.server_info().server_info.name.is_empty());

    let cache = server.cache();
    rt.block_on(async {
        cache
            .set("server-cache-key".to_string(), "value".to_string(), None)
            .await
            .expect("cache set should succeed");
        assert_eq!(
            cache.get("server-cache-key").await,
            Some("value".to_string())
        );
    });
}

#[test]
fn test_server_info_content() {
    let server = CratesDocsServer::new(AppConfig::default()).unwrap();
    let info = server.server_info();

    assert_eq!(info.server_info.name, "crates-docs");
    assert_eq!(
        info.server_info.title.as_deref(),
        Some("Crates Docs MCP Server")
    );
    assert!(info.server_info.description.is_some());
    assert_eq!(info.server_info.icons.len(), 2);
    assert!(info.capabilities.tools.is_some());
    assert!(info
        .instructions
        .unwrap()
        .contains("Rust crate documentation"));
}

// ============================================================================
// TransportMode 测试
// ============================================================================

#[test]
fn test_transport_mode_from_str() {
    use std::str::FromStr;

    let mode = crates_docs::server::transport::TransportMode::from_str("stdio").unwrap();
    assert_eq!(mode, crates_docs::server::transport::TransportMode::Stdio);

    let mode = crates_docs::server::transport::TransportMode::from_str("HTTP").unwrap();
    assert_eq!(mode, crates_docs::server::transport::TransportMode::Http);

    let mode = crates_docs::server::transport::TransportMode::from_str("Sse").unwrap();
    assert_eq!(mode, crates_docs::server::transport::TransportMode::Sse);

    let mode = crates_docs::server::transport::TransportMode::from_str("hybrid").unwrap();
    assert_eq!(mode, crates_docs::server::transport::TransportMode::Hybrid);

    let result = crates_docs::server::transport::TransportMode::from_str("invalid");
    assert!(result.is_err());
}

#[test]
fn test_transport_mode_display() {
    use crates_docs::server::transport::TransportMode;

    assert_eq!(format!("{}", TransportMode::Stdio), "stdio");
    assert_eq!(format!("{}", TransportMode::Http), "http");
    assert_eq!(format!("{}", TransportMode::Sse), "sse");
    assert_eq!(format!("{}", TransportMode::Hybrid), "hybrid");
}
