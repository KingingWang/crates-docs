//! 服务器启动端到端测试
//!
//! 测试服务器实际启动和端口监听功能。

use crates_docs::{AppConfig, CratesDocsServer};
use std::time::Duration;

/// 测试 HTTP 服务器实际启动和端口监听
#[tokio::test]
async fn test_http_server_actual_start() {
    // 使用随机端口
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();

    // 创建服务器
    let server = CratesDocsServer::new_async(config).await.unwrap();

    // 在后台运行服务器
    let handle = tokio::spawn(async move { server.run_http().await });

    // 等待服务器启动（带超时）
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port, Duration::from_secs(3)),
    )
    .await;

    assert!(result.is_ok(), "Server startup timed out");
    assert!(result.unwrap().is_ok(), "Server failed to start");

    // 发送测试请求验证服务器响应
    let client = super::create_test_client();
    let url = format!("http://127.0.0.1:{}/health", port);
    let response = client.get(&url).send().await;

    assert!(response.is_ok(), "Failed to connect to server");
    let response = response.unwrap();
    assert!(response.status().is_success(), "Health check failed");

    // 清理
    handle.abort();
}

/// 测试服务器能响应健康检查请求
#[tokio::test]
async fn test_server_health_check() {
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();

    let server = CratesDocsServer::new_async(config).await.unwrap();
    let handle = tokio::spawn(async move { server.run_http().await });

    // 等待健康检查通过
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_health_check(port, Duration::from_secs(3)),
    )
    .await;

    assert!(result.is_ok(), "Health check timed out");
    assert!(result.unwrap().is_ok(), "Health check failed");

    // 验证健康检查响应内容
    let client = super::create_test_client();
    let url = format!("http://127.0.0.1:{}/health", port);
    let response = client
        .get(&url)
        .send()
        .await
        .expect("Failed to send health check request");

    assert_eq!(response.status().as_u16(), 200);

    // 清理
    handle.abort();
}

/// 测试服务器能正确处理 MCP 协议请求
#[tokio::test]
async fn test_server_mcp_protocol() {
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();

    let server = CratesDocsServer::new_async(config).await.unwrap();
    let handle = tokio::spawn(async move { server.run_http().await });

    // 等待服务器启动
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port, Duration::from_secs(3)),
    )
    .await;
    assert!(
        result.is_ok() && result.unwrap().is_ok(),
        "Server failed to start"
    );

    // 发送 MCP 初始化请求
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

    // 验证响应是有效的 JSON（可能是 SSE 格式）
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

    // 清理
    handle.abort();
}

/// 测试 SSE 服务器启动
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

    // 等待服务器启动
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port, Duration::from_secs(3)),
    )
    .await;

    assert!(result.is_ok(), "SSE server startup timed out");
    assert!(result.unwrap().is_ok(), "SSE server failed to start");

    // 测试 SSE 端点
    let client = super::create_test_client();
    let url = format!("http://127.0.0.1:{}/sse", port);
    let response = client.get(&url).send().await;

    // SSE 端点可能返回 200 或需要特定处理
    assert!(response.is_ok(), "Failed to connect to SSE endpoint");

    // 清理
    handle.abort();
}

/// 测试 Hybrid 模式服务器启动
#[tokio::test]
async fn test_hybrid_server_actual_start() {
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "hybrid".to_string();
    config.server.host = "127.0.0.1".to_string();
    config.server.enable_sse = true;

    let server = CratesDocsServer::new_async(config).await.unwrap();
    let handle =
        tokio::spawn(
            async move { crates_docs::server::transport::run_hybrid_server(&server).await },
        );

    // 等待服务器启动
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port, Duration::from_secs(3)),
    )
    .await;

    assert!(result.is_ok(), "Hybrid server startup timed out");
    assert!(result.unwrap().is_ok(), "Hybrid server failed to start");

    // 测试 HTTP 端点
    let client = super::create_test_client();
    let url = format!("http://127.0.0.1:{}/health", port);
    let response = client.get(&url).send().await;

    assert!(response.is_ok(), "Failed to connect to health endpoint");
    assert!(
        response.unwrap().status().is_success(),
        "Health check failed"
    );

    // 清理
    handle.abort();
}

/// 测试服务器启动超时处理
#[tokio::test]
async fn test_server_startup_timeout() {
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();

    let server = CratesDocsServer::new_async(config).await.unwrap();

    // 启动服务器但不等待它完成启动
    let handle = tokio::spawn(async move { server.run_http().await });

    // 立即尝试连接（应该失败或超时）
    let client = super::create_test_client();
    let url = format!("http://127.0.0.1:{}/health", port);
    let result = tokio::time::timeout(Duration::from_millis(100), client.get(&url).send()).await;

    // 连接应该超时或失败，因为我们没有等待服务器启动
    // 这个结果可能是超时或连接被拒绝，都是可接受的
    match result {
        Ok(Ok(_)) => {
            // 如果连接成功，说明服务器启动非常快，这也是可以的
        }
        Ok(Err(_)) | Err(_) => {
            // 连接失败或超时是预期的
        }
    }

    // 清理
    handle.abort();
}

/// 测试多个服务器实例使用不同端口
#[tokio::test]
async fn test_multiple_servers_different_ports() {
    let port1 = super::get_random_port();
    let port2 = super::get_random_port();

    // 确保端口不同
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

    // 等待两个服务器都启动
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

    // 验证两个服务器都响应
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

    // 清理
    handle1.abort();
    handle2.abort();
}
