//! End-to-end test module
//!
//! Provides complete end-to-end tests including server startup, transport mode tests, and external API integration tests.

use std::net::TcpListener;
use std::time::Duration;

/// Get a random available port
///
/// Binds to port 0 to let the OS assign an available port, then immediately releases it
pub fn get_random_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

/// Wait for server to start
///
/// Detects if server has started by attempting to connect to the specified port
///
/// # Arguments
/// * `port` - Server port
/// * `timeout` - Maximum wait time
///
/// # Returns
/// * `Ok(())` - Server started successfully
/// * `Err(String)` - Timeout or connection failed
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

/// Wait for server health check to pass
///
/// Sends HTTP health check requests until success or timeout
///
/// # Arguments
/// * `port` - Server port
/// * `timeout` - Maximum wait time
///
/// # Returns
/// * `Ok(())` - Health check passed
/// * `Err(String)` - Timeout or check failed
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

/// Create test HTTP client
///
/// Configured with reasonable timeout settings
pub fn create_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to create test client")
}

/// Generate MCP JSON-RPC request
///
/// # Arguments
/// * `id` - Request ID
/// * `method` - Method name
/// * `params` - Parameters (optional)
///
/// # Returns
/// JSON object
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

/// Generate MCP initialize request
///
/// # Arguments
/// * `id` - Request ID
///
/// # Returns
/// JSON object
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

/// Generate tools/list request
///
/// # Arguments
/// * `id` - Request ID
///
/// # Returns
/// JSON object
pub fn create_tools_list_request(id: u64) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/list"
    })
}

/// Generate tools/call request
///
/// # Arguments
/// * `id` - Request ID
/// * `name` - Tool name
/// * `arguments` - Tool arguments
///
/// # Returns
/// JSON object
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

/// Extract JSON data from SSE response
///
/// SSE format example:
/// ```text
/// id: xxx
/// data: {"jsonrpc":"2.0",...}
/// ```
///
/// # Arguments
/// * `body_text` - SSE response text
///
/// # Returns
/// Extracted JSON string
pub fn extract_sse_json(body_text: &str) -> &str {
    body_text
        .lines()
        .find(|line| line.starts_with("data: "))
        .map(|line| line.strip_prefix("data: ").unwrap_or(line))
        .unwrap_or(body_text)
}

pub mod external_api_tests;
/// Test modules
pub mod server_start_tests;
pub mod transport_tests;
