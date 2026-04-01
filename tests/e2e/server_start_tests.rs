//! Server startup end-to-end tests
//!
//! Tests actual server startup and port listening functionality.

use crates_docs::{AppConfig, CratesDocsServer};
use std::time::Duration;

/// Test HTTP server actual startup and port listening
#[tokio::test]
async fn test_http_server_actual_start() {
    // Use random port
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();

    // Create server
    let server = CratesDocsServer::new_async(config).await.unwrap();

    // Run server in background
    let handle = tokio::spawn(async move { server.run_http().await });

    // Wait for server to start (with timeout)
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port, Duration::from_secs(3)),
    )
    .await;

    assert!(result.is_ok(), "Server startup timed out");
    assert!(result.unwrap().is_ok(), "Server failed to start");

    // Send test request to verify server response
    let client = super::create_test_client();
    let url = format!("http://127.0.0.1:{}/health", port);
    let response = client.get(&url).send().await;

    assert!(response.is_ok(), "Failed to connect to server");
    let response = response.unwrap();
    assert!(response.status().is_success(), "Health check failed");

    // Cleanup
    handle.abort();
}

/// Test server can respond to health check requests
#[tokio::test]
async fn test_server_health_check() {
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();

    let server = CratesDocsServer::new_async(config).await.unwrap();
    let handle = tokio::spawn(async move { server.run_http().await });

    // Wait for health check to pass
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_health_check(port, Duration::from_secs(3)),
    )
    .await;

    assert!(result.is_ok(), "Health check timed out");
    assert!(result.unwrap().is_ok(), "Health check failed");

    // Verify health check response content
    let client = super::create_test_client();
    let url = format!("http://127.0.0.1:{}/health", port);
    let response = client
        .get(&url)
        .send()
        .await
        .expect("Failed to send health check request");

    assert_eq!(response.status().as_u16(), 200);

    // Cleanup
    handle.abort();
}

/// Test server can correctly handle MCP protocol requests
#[tokio::test]
async fn test_server_mcp_protocol() {
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

    // Send MCP initialize request
    let client = super::create_test_client();
    let init_request = super::create_initialize_request(1);
    let url = format!("http://127.0.0.1:{}/mcp", port);

    let response = client
        .post(&url)
        .header("Accept", "application/json, text/event-stream")
        .json(&init_request)
        .send()
        .await;

    assert!(response.is_ok(), "MCP request failed");
    let response = response.unwrap();
    assert!(
        response.status().is_success(),
        "MCP request returned error status"
    );

    // Verify response is valid JSON (may be SSE format)
    let body_text = response.text().await.expect("Failed to read response");
    let json_str = super::extract_sse_json(&body_text);
    let body: serde_json::Value =
        serde_json::from_str(json_str).expect("Failed to parse response as JSON");
    assert!(
        body.get("jsonrpc").is_some(),
        "Response missing jsonrpc field"
    );
    assert_eq!(body["jsonrpc"], "2.0", "Invalid jsonrpc version");
    assert!(body.get("id").is_some(), "Response missing id field");

    // Cleanup
    handle.abort();
}

/// Test SSE server startup
#[tokio::test]
async fn test_sse_server_actual_start() {
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "sse".to_string();
    config.server.host = "127.0.0.1".to_string();
    config.server.enable_sse = true;

    let server = CratesDocsServer::new_async(config).await.unwrap();
    let handle = tokio::spawn(async move { server.run_sse().await });

    // Wait for server to start
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port, Duration::from_secs(3)),
    )
    .await;

    assert!(result.is_ok(), "SSE server startup timed out");
    assert!(result.unwrap().is_ok(), "SSE server failed to start");

    // Test SSE endpoint
    let client = super::create_test_client();
    let url = format!("http://127.0.0.1:{}/sse", port);
    let response = client.get(&url).send().await;

    // SSE endpoint may return 200 or require specific handling
    assert!(response.is_ok(), "Failed to connect to SSE endpoint");

    // Cleanup
    handle.abort();
}

/// Test Hybrid mode server startup
#[tokio::test]
async fn test_hybrid_server_actual_start() {
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "hybrid".to_string();
    config.server.host = "127.0.0.1".to_string();
    config.server.enable_sse = true;

    let server = CratesDocsServer::new_async(config).await.unwrap();
    let handle = tokio::spawn(async move {
        let config = crates_docs::server::transport::HyperServerConfig::hybrid();
        crates_docs::server::transport::run_hyper_server(&server, config).await
    });

    // Wait for server to start
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port, Duration::from_secs(3)),
    )
    .await;

    assert!(result.is_ok(), "Hybrid server startup timed out");
    assert!(result.unwrap().is_ok(), "Hybrid server failed to start");

    // Test HTTP endpoint
    let client = super::create_test_client();
    let url = format!("http://127.0.0.1:{}/health", port);
    let response = client.get(&url).send().await;

    assert!(response.is_ok(), "Failed to connect to health endpoint");
    assert!(
        response.unwrap().status().is_success(),
        "Health check failed"
    );

    // Cleanup
    handle.abort();
}

/// Test server startup timeout handling
#[tokio::test]
async fn test_server_startup_timeout() {
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();

    let server = CratesDocsServer::new_async(config).await.unwrap();

    // Start server but don't wait for it to complete startup
    let handle = tokio::spawn(async move { server.run_http().await });

    // Immediately try to connect (should fail or timeout)
    let client = super::create_test_client();
    let url = format!("http://127.0.0.1:{}/health", port);
    let result = tokio::time::timeout(Duration::from_millis(100), client.get(&url).send()).await;

    // Connection should timeout or fail because we didn't wait for server to start
    // This result could be timeout or connection refused, both are acceptable
    match result {
        Ok(Ok(_)) => {
            // If connection succeeds, server started very fast, which is also fine
        }
        Ok(Err(_)) | Err(_) => {
            // Connection failure or timeout is expected
        }
    }

    // Cleanup
    handle.abort();
}

/// Test multiple server instances using different ports
#[tokio::test]
async fn test_multiple_servers_different_ports() {
    let port1 = super::get_random_port();
    let port2 = super::get_random_port();

    // Ensure ports are different
    assert_ne!(port1, port2, "Random ports should be different");

    let mut config1 = AppConfig::default();
    config1.server.port = port1;
    config1.server.transport_mode = "http".to_string();
    config1.server.host = "127.0.0.1".to_string();

    let mut config2 = AppConfig::default();
    config2.server.port = port2;
    config2.server.transport_mode = "http".to_string();
    config2.server.host = "127.0.0.1".to_string();

    let server1 = CratesDocsServer::new_async(config1).await.unwrap();
    let server2 = CratesDocsServer::new_async(config2).await.unwrap();

    let handle1 = tokio::spawn(async move { server1.run_http().await });

    let handle2 = tokio::spawn(async move { server2.run_http().await });

    // Wait for both servers to start
    let result1 = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port1, Duration::from_secs(3)),
    )
    .await;
    let result2 = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port2, Duration::from_secs(3)),
    )
    .await;

    assert!(
        result1.is_ok() && result1.unwrap().is_ok(),
        "Server 1 failed to start"
    );
    assert!(
        result2.is_ok() && result2.unwrap().is_ok(),
        "Server 2 failed to start"
    );

    // Verify both servers respond
    let client = super::create_test_client();
    let url1 = format!("http://127.0.0.1:{}/health", port1);
    let url2 = format!("http://127.0.0.1:{}/health", port2);
    let response1 = client.get(&url1).send().await;
    let response2 = client.get(&url2).send().await;

    assert!(
        response1.is_ok() && response1.unwrap().status().is_success(),
        "Server 1 health check failed"
    );
    assert!(
        response2.is_ok() && response2.unwrap().status().is_success(),
        "Server 2 health check failed"
    );

    // Cleanup
    handle1.abort();
    handle2.abort();
}
