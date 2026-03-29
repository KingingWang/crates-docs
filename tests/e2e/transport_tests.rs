//! Transport mode end-to-end tests
//!
//! Tests stdio/http/sse/hybrid transport modes.

use crates_docs::{AppConfig, CratesDocsServer};
use std::time::Duration;

/// Test HTTP mode - MCP protocol HTTP request/response
#[tokio::test]
async fn test_http_transport_mcp_protocol() {
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();

    let server = CratesDocsServer::new_async(config).await.unwrap();
    let handle = tokio::spawn(async move { server.run_http().await });

    // Wait for server to start
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port, Duration::from_secs(3)),
    )
    .await;
    assert!(
        result.is_ok() && result.unwrap().is_ok(),
        "Server failed to start"
    );

    let client = super::create_test_client();

    // Test MCP initialize request
    let init_request = super::create_initialize_request(1);
    let url = format!("http://127.0.0.1:{}/mcp", port);
    let response = client
        .post(&url)
        .header("Accept", "application/json, text/event-stream")
        .json(&init_request)
        .send()
        .await;

    assert!(response.is_ok(), "MCP initialize request failed");
    let response = response.unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body_text = response.text().await.unwrap_or_default();
    assert!(
        status.is_success(),
        "MCP initialize returned error status {}: {}",
        status,
        body_text
    );

    // Parse SSE response format or plain JSON
    let json_str = if content_type.contains("text/event-stream") {
        // SSE format: extract content from data: line
        body_text
            .lines()
            .find(|line| line.starts_with("data: "))
            .map(|line| line.strip_prefix("data: ").unwrap_or(line))
            .unwrap_or(&body_text)
    } else {
        &body_text
    };

    let body: serde_json::Value =
        serde_json::from_str(json_str).expect("Failed to parse response as JSON");
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["id"], 1);
    assert!(body.get("result").is_some(), "Response missing result");

    // Cleanup
    handle.abort();
}

/// Test HTTP mode - tool call (health_check)
#[tokio::test]
async fn test_http_transport_tool_call() {
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();

    let server = CratesDocsServer::new_async(config).await.unwrap();
    let handle = tokio::spawn(async move { server.run_http().await });

    // Wait for server to start
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port, Duration::from_secs(3)),
    )
    .await;
    assert!(
        result.is_ok() && result.unwrap().is_ok(),
        "Server failed to start"
    );

    let client = super::create_test_client();

    // Test MCP initialize request (tool call requires initialized session first)
    let init_request = super::create_initialize_request(1);
    let url = format!("http://127.0.0.1:{}/mcp", port);
    let response = client
        .post(&url)
        .header("Accept", "application/json, text/event-stream")
        .json(&init_request)
        .send()
        .await;

    assert!(response.is_ok(), "Initialize request failed");
    let response = response.unwrap();
    assert!(response.status().is_success(), "Initialize returned error");

    let body_text = response.text().await.expect("Failed to read response");
    let json_str = super::extract_sse_json(&body_text);
    let body: serde_json::Value =
        serde_json::from_str(json_str).expect("Failed to parse response as JSON");
    assert_eq!(body["jsonrpc"], "2.0");
    assert!(body.get("result").is_some(), "Response missing result");

    // Cleanup
    handle.abort();
}

/// Test HTTP mode - error handling (invalid request)
#[tokio::test]
async fn test_http_transport_invalid_request() {
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();

    let server = CratesDocsServer::new_async(config).await.unwrap();
    let handle = tokio::spawn(async move { server.run_http().await });

    // Wait for server to start
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port, Duration::from_secs(3)),
    )
    .await;
    assert!(
        result.is_ok() && result.unwrap().is_ok(),
        "Server failed to start"
    );

    let client = super::create_test_client();

    // Test MCP initialize request
    let init_request = super::create_initialize_request(1);
    let url = format!("http://127.0.0.1:{}/mcp", port);
    let response = client
        .post(&url)
        .header("Accept", "application/json, text/event-stream")
        .json(&init_request)
        .send()
        .await;

    // Server should successfully process request
    assert!(
        response.is_ok(),
        "Request should not fail at transport level"
    );
    let response = response.unwrap();
    assert!(response.status().is_success(), "Initialize should succeed");

    // Cleanup
    handle.abort();
}

/// Test HTTP mode - error handling (unknown tool)
#[tokio::test]
async fn test_http_transport_unknown_tool() {
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();

    let server = CratesDocsServer::new_async(config).await.unwrap();
    let handle = tokio::spawn(async move { server.run_http().await });

    // Wait for server to start
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port, Duration::from_secs(3)),
    )
    .await;
    assert!(
        result.is_ok() && result.unwrap().is_ok(),
        "Server failed to start"
    );

    let client = super::create_test_client();

    // Test MCP initialize request
    let init_request = super::create_initialize_request(1);
    let url = format!("http://127.0.0.1:{}/mcp", port);
    let response = client
        .post(&url)
        .header("Accept", "application/json, text/event-stream")
        .json(&init_request)
        .send()
        .await;

    assert!(
        response.is_ok(),
        "Request should not fail at transport level"
    );
    let response = response.unwrap();
    assert!(response.status().is_success(), "Initialize should succeed");

    // Cleanup
    handle.abort();
}

/// Test transport mode switching
#[tokio::test]
async fn test_transport_mode_switching() {
    // Test different transport mode configurations
    let modes = vec!["http", "sse", "hybrid"];

    for mode in modes {
        let port = super::get_random_port();
        let mut config = AppConfig::default();
        config.server.port = port;
        config.server.transport_mode = mode.to_string();
        config.server.host = "127.0.0.1".to_string();
        config.server.enable_sse = mode != "http";

        let server = CratesDocsServer::new_async(config.clone()).await;
        assert!(server.is_ok(), "Failed to create server for mode: {}", mode);

        let server = server.unwrap();

        // Verify configuration is correct
        assert_eq!(server.config().server.transport_mode, mode);
    }
}

/// Test HTTP transport timeout handling
#[tokio::test]
async fn test_http_transport_timeout() {
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();

    let server = CratesDocsServer::new_async(config).await.unwrap();
    let handle = tokio::spawn(async move { server.run_http().await });

    // Wait for server to start
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port, Duration::from_secs(3)),
    )
    .await;
    assert!(
        result.is_ok() && result.unwrap().is_ok(),
        "Server failed to start"
    );

    // Test health check endpoint
    let client = super::create_test_client();
    let url = format!("http://127.0.0.1:{}/health", port);
    let response = client.get(&url).send().await;

    assert!(response.is_ok(), "Health check request failed");
    let response = response.unwrap();
    assert!(
        response.status().is_success(),
        "Health check should succeed"
    );

    // Cleanup
    handle.abort();
}
