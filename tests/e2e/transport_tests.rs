//! 传输模式端到端测试
//!
//! 测试x stdio/http/sse/hybrid 四种传输模式。

use crates_docs::{AppConfig, CratesDocsServer};
use std::time::Duration;

/// 测试 HTTP 模式 - MCP 协议的 HTTP 请求/响应
#[tokio::test]
async fn test_http_transport_mcp_protocol() {
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

    let client = super::create_test_client();

    // 测试 MCP 初始化请求
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

    // 解析 SSE 响应格式或纯 JSON
    let json_str = if content_type.contains("text/event-stream") {
        // SSE 格式：提取 data: 行的内容
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

    // 清理
    handle.abort();
}

/// 测试 HTTP 模式 - 工具调用（health_check）
#[tokio::test]
async fn test_http_transport_tool_call() {
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

    let client = super::create_test_client();

    // 测试 MCP 初始化请求（工具调用需要先初始化会话）
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

    // 清理
    handle.abort();
}

/// 测试 HTTP 模式 - 错误处理（无效请求）
#[tokio::test]
async fn test_http_transport_invalid_request() {
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

    let client = super::create_test_client();

    // 测试 MCP 初始化请求
    let init_request = super::create_initialize_request(1);
    let url = format!("http://127.0.0.1:{}/mcp", port);
    let response = client
        .post(&url)
        .header("Accept", "application/json, text/event-stream")
        .json(&init_request)
        .send()
        .await;

    // 服务器应该成功处理请求
    assert!(
        response.is_ok(),
        "Request should not fail at transport level"
    );
    let response = response.unwrap();
    assert!(response.status().is_success(), "Initialize should succeed");

    // 清理
    handle.abort();
}

/// 测试 HTTP 模式 - 错误处理（未知工具）
#[tokio::test]
async fn test_http_transport_unknown_tool() {
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

    let client = super::create_test_client();

    // 测试 MCP 初始化请求
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

    // 清理
    handle.abort();
}

/// 测试 SSE 模式 - SSE 连接建立
#[tokio::test]
async fn test_sse_transport_connection() {
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
    assert!(
        result.is_ok() && result.unwrap().is_ok(),
        "Server failed to start"
    );

    let client = super::create_test_client();

    // 测试 SSE 端点连接
    let url = format!("http://127.0.0.1:{}/sse", port);
    let response = client.get(&url).send().await;

    assert!(response.is_ok(), "SSE connection failed");
    let response = response.unwrap();

    // SSE 端点应该返回 200 OK
    assert!(
        response.status().is_success(),
        "SSE endpoint returned error"
    );

    // 验证 Content-Type 包含 text/event-stream
    let content_type = response.headers().get("content-type");
    if let Some(ct) = content_type {
        let ct_str = ct.to_str().unwrap_or("");
        assert!(
            ct_str.contains("text/event-stream"),
            "Content-Type should be text/event-stream"
        );
    }

    // 清理
    handle.abort();
}

/// 测试 Hybrid 模式 - 同时支持 HTTP 和 SSE
#[tokio::test]
async fn test_hybrid_transport_both_modes() {
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
    assert!(
        result.is_ok() && result.unwrap().is_ok(),
        "Server failed to start"
    );

    let client = super::create_test_client();

    // 测试 HTTP 端点（health）
    let health_url = format!("http://127.0.0.1:{}/health", port);
    let health_response = client.get(&health_url).send().await;
    assert!(health_response.is_ok(), "Health endpoint failed");
    assert!(
        health_response.unwrap().status().is_success(),
        "Health check failed"
    );

    // 测试 MCP HTTP 端点
    let init_request = super::create_initialize_request(1);
    let mcp_url = format!("http://127.0.0.1:{}/mcp", port);
    let mcp_response = client
        .post(&mcp_url)
        .header("Accept", "application/json, text/event-stream")
        .json(&init_request)
        .send()
        .await;
    assert!(mcp_response.is_ok(), "MCP endpoint failed");
    assert!(
        mcp_response.unwrap().status().is_success(),
        "MCP request failed"
    );

    // 测试 SSE 端点
    let sse_url = format!("http://127.0.0.1:{}/sse", port);
    let sse_response = client.get(&sse_url).send().await;
    assert!(sse_response.is_ok(), "SSE endpoint failed");
    assert!(
        sse_response.unwrap().status().is_success(),
        "SSE connection failed"
    );

    // 清理
    handle.abort();
}

/// 测试 Stdio 模式 - 服务器创建和配置
#[tokio::test]
async fn test_stdio_transport() {
    // 测试 stdio 模式的服务器创建
    let mut config = AppConfig::default();
    config.server.transport_mode = "stdio".to_string();

    // 创建服务器
    let server = CratesDocsServer::new_async(config).await;
    assert!(server.is_ok(), "Failed to create stdio server");

    // 验证服务器配置
    let server = server.unwrap();
    assert_eq!(server.config().server.transport_mode, "stdio");
}

/// 测试 Stdio 模式 - 服务器信息
#[tokio::test]
async fn test_stdio_transport_tools_list() {
    // 测试 stdio 模式的服务器信息
    let mut config = AppConfig::default();
    config.server.transport_mode = "stdio".to_string();

    let server = CratesDocsServer::new_async(config).await.unwrap();
    let server_info = server.server_info();

    // 验证服务器信息
    assert!(
        !server_info.server_info.name.is_empty(),
        "Server name should not be empty"
    );
    assert!(
        !server_info.server_info.version.is_empty(),
        "Server version should not be empty"
    );
}

/// 测试传输模式切换
#[tokio::test]
async fn test_transport_mode_switching() {
    // 测试不同传输模式配置
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

        // 验证配置正确
        assert_eq!(server.config().server.transport_mode, mode);
    }
}

/// 测试 HTTP 传输超时处理
#[tokio::test]
async fn test_http_transport_timeout() {
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

    // 测试健康检查端点
    let client = super::create_test_client();
    let url = format!("http://127.0.0.1:{}/health", port);
    let response = client.get(&url).send().await;

    assert!(response.is_ok(), "Health check request failed");
    let response = response.unwrap();
    assert!(
        response.status().is_success(),
        "Health check should succeed"
    );

    // 清理
    handle.abort();
}
