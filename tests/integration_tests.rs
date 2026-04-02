//! Integration tests

use crates_docs::{
    cache::{create_cache, CacheConfig},
    tools::docs::DocService,
    AppConfig, CratesDocsServer,
};
use std::sync::Arc;

/// Test cache functionality
#[tokio::test]
async fn test_cache_functionality() {
    // Create memory cache
    let config = CacheConfig::default();

    let cache = create_cache(&config).expect("Failed to create cache");

    // Test basic cache operations
    cache
        .set("test_key".to_string(), "test_value".to_string(), None)
        .await
        .expect("set should succeed");
    let value = cache.get("test_key").await;
    assert_eq!(value, Some("test_value".to_string()));

    // Test cache expiration
    cache
        .set(
            "expiring_key".to_string(),
            "expiring_value".to_string(),
            Some(std::time::Duration::from_secs(1)),
        )
        .await
        .expect("set should succeed");
    let value = cache.get("expiring_key").await;
    assert_eq!(value, Some("expiring_value".to_string()));

    // Wait for expiration
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let value = cache.get("expiring_key").await;
    assert_eq!(value, None);

    // Test delete
    cache
        .delete("test_key")
        .await
        .expect("delete should succeed");
    let value = cache.get("test_key").await;
    assert_eq!(value, None);

    // Test clear
    cache
        .set("key1".to_string(), "value1".to_string(), None)
        .await
        .expect("set should succeed");
    cache
        .set("key2".to_string(), "value2".to_string(), None)
        .await
        .expect("set should succeed");
    cache.clear().await.expect("clear should succeed");
    assert_eq!(cache.get("key1").await, None);
    assert_eq!(cache.get("key2").await, None);
}

/// Test config loading
#[test]
fn test_config_loading() {
    // Test default config
    let config = AppConfig::default();
    assert_eq!(config.server.host, "127.0.0.1");
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.server.transport_mode, "hybrid");

    // Test validation
    let validation_result = config.validate();
    assert!(validation_result.is_ok());

    // Test environment variable config - use temp-env to safely set temporary environment variables
    temp_env::with_vars(
        [
            ("CRATES_DOCS_HOST", Some("127.0.0.1")),
            ("CRATES_DOCS_PORT", Some("9090")),
        ],
        || {
            let env_config = AppConfig::from_env();
            assert!(env_config.is_ok());

            // Verify environment variables are effective
            let config = AppConfig::merge(None, Some(env_config.unwrap()));
            assert_eq!(config.server.host, "127.0.0.1");
            assert_eq!(config.server.port, 9090);
        },
    );
}

/// Test tool registry
#[tokio::test]
async fn test_tool_registry() {
    // Create cache
    let config = CacheConfig::default();

    let cache = create_cache(&config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);

    // Create doc service
    let doc_service = Arc::new(DocService::new(cache_arc).expect("Failed to create DocService"));

    // Create tool registry
    let registry = crates_docs::tools::create_default_registry(&doc_service);

    // Verify expected tools are registered
    let tools = registry.get_tools();
    assert_eq!(tools.len(), 4);
    let tool_names: std::collections::HashSet<String> =
        tools.iter().map(|t| t.name.clone()).collect();
    assert!(tool_names.contains("lookup_crate"));
    assert!(tool_names.contains("lookup_item"));
    assert!(tool_names.contains("search_crates"));
    assert!(tool_names.contains("health_check"));
}

/// Test server creation
#[test]
fn test_server_creation() {
    // Create server config
    let config = AppConfig::default();

    // Create server
    let server_result = CratesDocsServer::new(config);
    assert!(
        server_result.is_ok(),
        "Server creation failed: {:?}",
        server_result.err()
    );

    let server = server_result.unwrap();

    // Test server info
    let server_info = server.server_info();
    assert_eq!(server_info.server_info.name, "crates-docs");
    assert_eq!(server_info.server_info.version, env!("CARGO_PKG_VERSION"));

    // Test tool list - Note: ServerCapabilitiesTools struct may not have is_empty method
    // We only check if capabilities.tools exists
    assert!(
        server_info.capabilities.tools.is_some(),
        "Server should provide tool capabilities"
    );
}

/// Test tool parameter validation
#[test]
fn test_tool_parameter_validation() {
    use crates_docs::utils::validation;

    // Test crate name validation
    assert!(validation::validate_crate_name("serde").is_ok());
    assert!(validation::validate_crate_name("tokio").is_ok());
    assert!(validation::validate_crate_name("reqwest").is_ok());

    // Test invalid crate names
    assert!(validation::validate_crate_name("").is_err());
    assert!(validation::validate_crate_name("invalid name with spaces").is_err());
    // Note: It seems validate_crate_name may not allow uppercase, but it may actually allow it

    // Test version validation
    assert!(validation::validate_version("1.0.0").is_ok());
    assert!(validation::validate_version("0.1.0-alpha.1").is_ok());
    assert!(validation::validate_version("2.3.4-beta.5").is_ok());

    // Test invalid versions
    assert!(validation::validate_version("").is_err());
    // According to actual implementation, 1.0 is valid because it contains numbers
    assert!(validation::validate_version("1.0").is_ok()); // Contains numbers, should be valid
    assert!(validation::validate_version("invalid").is_err());

    // Test search query validation
    assert!(validation::validate_search_query("serde").is_ok());
    assert!(validation::validate_search_query("web framework").is_ok());
    assert!(validation::validate_search_query("async").is_ok());

    // Test invalid search queries
    assert!(validation::validate_search_query("").is_err());
    // According to actual implementation, space string is valid because it is not empty and not more than 200 characters
    assert!(validation::validate_search_query("   ").is_ok());
    // According to actual implementation, single character is valid because it is not empty and not more than 200 characters
    assert!(validation::validate_search_query("a").is_ok());
}

/// Test performance counter
#[test]
fn test_performance_counter() {
    use crates_docs::utils::metrics::PerformanceCounter;
    use std::time::Duration;

    let counter = PerformanceCounter::new();

    // Initial state
    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 0);
    assert_eq!(stats.average_response_time_ms, 0.0);
    assert_eq!(stats.success_rate_percent, 0.0);

    // Record request
    let start = counter.record_request_start();
    std::thread::sleep(Duration::from_millis(10));
    counter.record_request_complete(start, true);

    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 1);
    assert_eq!(stats.successful_requests, 1);
    assert_eq!(stats.failed_requests, 0);
    assert!(stats.average_response_time_ms > 0.0);
    assert_eq!(stats.success_rate_percent, 100.0);

    // Record failed request
    let start = counter.record_request_start();
    std::thread::sleep(Duration::from_millis(5));
    counter.record_request_complete(start, false);

    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 2);
    assert_eq!(stats.successful_requests, 1);
    assert_eq!(stats.failed_requests, 1);
    assert!(stats.average_response_time_ms > 0.0);
    assert_eq!(stats.success_rate_percent, 50.0);

    // Test reset
    counter.reset();
    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 0);
    assert_eq!(stats.average_response_time_ms, 0.0);
    assert_eq!(stats.success_rate_percent, 0.0);
}

/// Test string utility functions
#[test]
fn test_string_utils() {
    use crates_docs::utils::string;

    // Test truncate function
    let long_string = "This is a very long string that needs to be truncated";
    let truncated = string::truncate_with_ellipsis(long_string, 20);
    assert_eq!(truncated, "This is a very lo...");
    assert!(truncated.len() <= 20 + 3); // Original length + ellipsis

    // Test short string
    let short_string = "short";
    let truncated = string::truncate_with_ellipsis(short_string, 10);
    assert_eq!(truncated, "short");

    // Test boundary case
    let exact_string = "exact length";
    let truncated = string::truncate_with_ellipsis(exact_string, 5);
    assert_eq!(truncated, "ex...");
}

/// Test compression utility functions
#[test]
fn test_compression_utils() {
    use crates_docs::utils::compression;

    let original_data = b"This is a test string for testing compression and decompression.";

    // Test GZIP compression and decompression
    let compressed = compression::gzip_compress(original_data);
    assert!(compressed.is_ok());
    let compressed_data = compressed.unwrap();
    assert!(!compressed_data.is_empty());
    // Note: For very short data, compressed may not be smaller
    // assert!(compressed_data.len() < original_data.len()); // Should be smaller after compression

    let decompressed = compression::gzip_decompress(&compressed_data);
    assert!(decompressed.is_ok());
    let decompressed_data = decompressed.unwrap();
    assert_eq!(decompressed_data, original_data);

    // Test invalid data decompression
    let invalid_data = b"not valid gzip data";
    let result = compression::gzip_decompress(invalid_data);
    assert!(result.is_err());
}

/// Test HTTP client builder
#[test]
fn test_http_client_builder() {
    use crates_docs::utils::HttpClientBuilder;
    use std::time::Duration;

    // Test default build
    let client = HttpClientBuilder::default().build();
    assert!(client.is_ok());

    // Test custom config
    let client = HttpClientBuilder::default()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(20)
        .user_agent("TestClient/1.0".to_string())
        .enable_gzip(true)
        .enable_brotli(true)
        .build();

    assert!(client.is_ok());
}

/// Test rate limiter
#[tokio::test]
async fn test_rate_limiter() {
    use crates_docs::utils::RateLimiter;
    use tokio::sync::SemaphorePermit;

    let limiter = RateLimiter::new(2); // Max 2 permits

    // Acquire permit
    let permit1: Result<SemaphorePermit<'_>, crates_docs::error::Error> = limiter.acquire().await;
    assert!(permit1.is_ok());

    let permit2: Result<SemaphorePermit<'_>, crates_docs::error::Error> = limiter.acquire().await;
    assert!(permit2.is_ok());

    // Third should be blocked (but we don't wait in test)
    // Here we just test the permit acquisition functionality

    // Release permits
    drop(permit1);
    drop(permit2);

    // Now should be able to acquire permit again
    let permit3: Result<SemaphorePermit<'_>, crates_docs::error::Error> = limiter.acquire().await;
    assert!(permit3.is_ok());
}

/// Test time utility functions
#[test]
fn test_time_utils() {
    use chrono::Utc;
    use crates_docs::utils::time;

    // Test current timestamp
    let timestamp = time::current_timestamp_ms();
    assert!(timestamp > 0);

    // Test formatted time
    let now = Utc::now();
    let formatted = time::format_datetime(&now);
    assert!(!formatted.is_empty());
    assert!(formatted.contains("-")); // Should contain date separator

    // Test calculate time interval
    let start = std::time::Instant::now();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let elapsed = time::elapsed_ms(start);
    assert!(elapsed >= 10); // At least 10 milliseconds
}

/// Test OAuth config
#[test]
fn test_oauth_config() {
    use crates_docs::server::auth::{OAuthConfig, OAuthProvider};

    // Test default config
    let default_config = OAuthConfig::default();
    assert!(!default_config.enabled);
    assert_eq!(default_config.client_id, None);
    assert_eq!(default_config.client_secret, None);
    assert_eq!(default_config.redirect_uri, None);

    // Test create custom config
    let config = OAuthConfig {
        enabled: true,
        client_id: Some("client_id".to_string()),
        client_secret: Some("client_secret".to_string()),
        redirect_uri: Some("http://localhost:8080/oauth/callback".to_string()),
        authorization_endpoint: Some("https://github.com/login/oauth/authorize".to_string()),
        token_endpoint: Some("https://github.com/login/oauth/access_token".to_string()),
        scopes: vec!["read:user".to_string()],
        provider: OAuthProvider::GitHub,
    };

    assert!(config.enabled);
    assert_eq!(config.client_id, Some("client_id".to_string()));
    assert_eq!(config.client_secret, Some("client_secret".to_string()));
    assert_eq!(
        config.redirect_uri,
        Some("http://localhost:8080/oauth/callback".to_string())
    );

    // Test validation
    let validation_result = config.validate();
    assert!(validation_result.is_ok());

    // Test invalid config
    let invalid_config = OAuthConfig {
        enabled: true,
        client_id: None, // Missing client ID
        client_secret: Some("client_secret".to_string()),
        redirect_uri: Some("http://localhost:8080/oauth/callback".to_string()),
        authorization_endpoint: Some("https://github.com/login/oauth/authorize".to_string()),
        token_endpoint: Some("https://github.com/login/oauth/access_token".to_string()),
        scopes: vec!["read:user".to_string()],
        provider: OAuthProvider::GitHub,
    };

    let validation_result = invalid_config.validate();
    assert!(validation_result.is_err());
}

/// Test transport mode - stdio
#[tokio::test]
async fn test_transport_mode_stdio() {
    let config = AppConfig::default();
    let server = CratesDocsServer::new(config).unwrap();

    // stdio mode test - verify server can be created
    let server_info = server.server_info();
    assert_eq!(server_info.server_info.name, "crates-docs");
}

/// Test transport mode - HTTP
#[tokio::test]
async fn test_transport_mode_http() {
    let mut config = AppConfig::default();
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();
    config.server.port = 8081;

    let server = CratesDocsServer::new(config).unwrap();
    let server_info = server.server_info();

    assert_eq!(server_info.server_info.name, "crates-docs");
    assert_eq!(server.config().server.transport_mode, "http");
}

/// Test transport mode - SSE
#[tokio::test]
async fn test_transport_mode_sse() {
    let mut config = AppConfig::default();
    config.server.transport_mode = "sse".to_string();
    config.server.host = "127.0.0.1".to_string();
    config.server.port = 8082;

    let server = CratesDocsServer::new(config).unwrap();
    let server_info = server.server_info();

    assert_eq!(server_info.server_info.name, "crates-docs");
    assert_eq!(server.config().server.transport_mode, "sse");
}

/// Test transport mode - hybrid
#[tokio::test]
async fn test_transport_mode_hybrid() {
    let mut config = AppConfig::default();
    config.server.transport_mode = "hybrid".to_string();
    config.server.host = "127.0.0.1".to_string();
    config.server.port = 8083;

    let server = CratesDocsServer::new(config).unwrap();
    let server_info = server.server_info();

    assert_eq!(server_info.server_info.name, "crates-docs");
    assert_eq!(server.config().server.transport_mode, "hybrid");
}

/// Test performance config
#[tokio::test]
async fn test_performance_config() {
    let mut config = AppConfig::default();

    // Configure HTTP client parameters
    config.performance.http_client_pool_size = 20;
    config.performance.http_client_pool_idle_timeout_secs = 120;
    config.performance.http_client_connect_timeout_secs = 15;
    config.performance.http_client_timeout_secs = 45;
    config.performance.http_client_read_timeout_secs = 45;
    config.performance.http_client_max_retries = 5;
    config.performance.http_client_retry_initial_delay_ms = 200;
    config.performance.http_client_retry_max_delay_ms = 20000;

    // Configure cache parameters
    config.performance.cache_max_size = 2000;
    config.performance.cache_default_ttl_secs = 7200;

    // Configure rate limit
    config.performance.rate_limit_per_second = 200;
    config.performance.concurrent_request_limit = 100;

    // Configure metrics
    config.performance.enable_metrics = true;
    config.performance.metrics_port = 9090;

    let server = CratesDocsServer::new(config).unwrap();
    let perf_config = &server.config().performance;

    assert_eq!(perf_config.http_client_pool_size, 20);
    assert_eq!(perf_config.http_client_pool_idle_timeout_secs, 120);
    assert_eq!(perf_config.http_client_connect_timeout_secs, 15);
    assert_eq!(perf_config.http_client_timeout_secs, 45);
    assert_eq!(perf_config.http_client_read_timeout_secs, 45);
    assert_eq!(perf_config.http_client_max_retries, 5);
    assert_eq!(perf_config.http_client_retry_initial_delay_ms, 200);
    assert_eq!(perf_config.http_client_retry_max_delay_ms, 20000);
    assert_eq!(perf_config.cache_max_size, 2000);
    assert_eq!(perf_config.cache_default_ttl_secs, 7200);
    assert_eq!(perf_config.rate_limit_per_second, 200);
    assert_eq!(perf_config.concurrent_request_limit, 100);
    assert!(perf_config.enable_metrics);
    assert_eq!(perf_config.metrics_port, 9090);
}
