//! 端到端测试模块
//!
//! 提供完整的端到端测试，包括服务器启动、传输模式测试和外部 API 集成测试。

use std::net::TcpListener;
use std::time::Duration;

/// 获取随机可用端口
///
/// 通过绑定到端口 0 让操作系统分配一个可用端口，然后立即释放
pub fn get_random_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

/// 等待服务器启动
///
/// 通过尝试连接指定端口来检测服务器是否已启动
///
/// # Arguments
/// * `port` - 服务器端口
/// * `timeout` - 最大等待时间
///
/// # Returns
/// * `Ok(())` - 服务器成功启动
/// * `Err(String)` - 超时或连接失败
pub async fn wait_for_server(port: u16, timeout: Duration) -> Result<(), String> {
    let start = std::time::Instant::now();
    let addr = format!("127.0.0.1:{}", port);

    while start.elapsed() < timeout {
        match tokio::net::TcpStream::connect(&addr).await {
            Ok(_) => return Ok(()),
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }

    Err(format!(
        "Server failed to start on port {} within {:?}",
        port, timeout
    ))
}

/// 等待服务器健康检查通过
///
/// 发送 HTTP 健康检查请求直到成功或超时
///
/// # Arguments
/// * `port` - 服务器端口
/// * `timeout` - 最大等待时间
///
/// # Returns
/// * `Ok(())` - 健康检查通过
/// * `Err(String)` - 超时或检查失败
pub async fn wait_for_health_check(port: u16, timeout: Duration) -> Result<(), String> {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/health", port);
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        match client
            .get(&url)
            .timeout(Duration::from_secs(1))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => return Ok(()),
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }

    Err(format!(
        "Health check failed on port {} within {:?}",
        port, timeout
    ))
}

/// 创建测试用的 HTTP 客户端
///
/// 配置了合理的超时设置
pub fn create_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to create test client")
}

/// 生成 MCP JSON-RPC 请求
///
/// # Arguments
/// * `id` - 请求 ID
/// * `method` - 方法名
/// * `params` - 参数（可选）
///
/// # Returns
/// JSON 对象
pub fn create_mcp_request(
    id: u64,
    method: &str,
    params: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
    });

    if let Some(p) = params {
        request["params"] = p;
    }

    request
}

/// 生成 MCP 初始化请求
///
/// # Arguments
/// * `id` - 请求 ID
///
/// # Returns
/// JSON 对象
pub fn create_initialize_request(id: u64) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        }
    })
}

/// 生成 tools/list 请求
///
/// # Arguments
/// * `id` - 请求 ID
///
/// # Returns
/// JSON 对象
pub fn create_tools_list_request(id: u64) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/list"
    })
}

/// 生成 tools/call 请求
///
/// # Arguments
/// * `id` - 请求 ID
/// * `name` - 工具名称
/// * `arguments` - 工具参数
///
/// # Returns
/// JSON 对象
pub fn create_tools_call_request(
    id: u64,
    name: &str,
    arguments: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": arguments
        }
    })
}

/// 从 SSE 响应中提取 JSON 数据
///
/// SSE 格式示例：
/// ```text
/// id: xxx
/// data: {"jsonrpc":"2.0",...}
/// ```
///
/// # Arguments
/// * `body_text` - SSE 响应文本
///
/// # Returns
/// 提取的 JSON 字符串
pub fn extract_sse_json(body_text: &str) -> &str {
    body_text
        .lines()
        .find(|line| line.starts_with("data: "))
        .map(|line| line.strip_prefix("data: ").unwrap_or(line))
        .unwrap_or(body_text)
}

pub mod external_api_tests;
/// 测试模块
pub mod server_start_tests;
pub mod transport_tests;
