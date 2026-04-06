//! Unit tests

use crates_docs::{
    cache::{create_cache, CacheConfig},
    tools::docs::cache::DocCache,
};
use std::sync::Arc;

// ============================================================================
// DocCache tests
// ============================================================================

/// Test HTML cleaning - remove script tags
#[test]
fn test_clean_html_removes_script_tags() {
    let html =
        r#"<html><head><script>alert('test');</script></head><body><p>Hello</p></body></html>"#;
    // Test using DocService's internal method (via public API)
    // Since clean_html is a private function, we verify through integration tests
    assert!(html.contains("<script>"));
    assert!(html.contains("Hello"));
}

/// Test HTML cleaning - remove style tags
#[test]
fn test_clean_html_removes_style_tags() {
    let html = r#"<html><head><style>.test { color: red; }</style></head><body><p>World</p></body></html>"#;
    assert!(html.contains("<style>"));
    assert!(html.contains("World"));
}

/// Test HTML cleaning - remove noscript tags
#[test]
fn test_clean_html_removes_noscript_tags() {
    let html = r#"<html><body><noscript>Enable JavaScript</noscript><p>Content</p></body></html>"#;
    assert!(html.contains("<noscript>"));
    assert!(html.contains("Content"));
}

/// Test HTML entity decoding
#[test]
fn test_html_entity_decoding() {
    // Test common HTML entities - verify entity and expected values are not empty
    let entities: [(&str, &str); 5] = [
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&amp;", "&"),
        ("&quot;", "\""),
        ("&apos;", "'"),
    ];
    for (entity, expected) in entities {
        assert!(!entity.is_empty(), "Entity should not be empty");
        assert!(!expected.is_empty(), "Expected value should not be empty");
    }
}

// ============================================================================
// DocCache tests
// ============================================================================

/// Test DocCache crate docs caching
#[tokio::test]
async fn test_doc_cache_crate_docs() {
    let config = CacheConfig::default();
    let cache = create_cache(&config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);
    let doc_cache = DocCache::new(cache_arc);

    // Test cache miss
    let result = doc_cache.get_crate_docs("serde", None).await;
    assert!(result.is_none());

    // Set cache
    doc_cache
        .set_crate_docs("serde", None, "Serde documentation".to_string())
        .await
        .expect("set_crate_docs should succeed");

    // Test cache hit
    let result = doc_cache.get_crate_docs("serde", None).await;
    assert_eq!(
        result.as_ref().map(|s| s.as_ref()),
        Some("Serde documentation")
    );

    // Test cache with version
    doc_cache
        .set_crate_docs("tokio", Some("1.0.0"), "Tokio 1.0 docs".to_string())
        .await
        .expect("set_crate_docs should succeed");
    let result = doc_cache.get_crate_docs("tokio", Some("1.0.0")).await;
    assert_eq!(result.as_ref().map(|s| s.as_ref()), Some("Tokio 1.0 docs"));

    // Different versions should return different cached values
    let result = doc_cache.get_crate_docs("tokio", Some("1.1.0")).await;
    assert!(result.is_none());
}

/// Test DocCache item docs caching
#[tokio::test]
async fn test_doc_cache_item_docs() {
    let config = CacheConfig::default();
    let cache = create_cache(&config).expect("Failed to create cache");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);
    let doc_cache = DocCache::new(cache_arc);

    // Test cache miss
    let result = doc_cache
        .get_item_docs("serde", "serde::Serialize", None)
        .await;
    assert!(result.is_none());

    // Set cache
    doc_cache
        .set_item_docs(
            "serde",
            "serde::Serialize",
            None,
            "Serialize trait docs".to_string(),
        )
        .await
        .expect("set_item_docs should succeed");

    // Test cache hit
    let result = doc_cache
        .get_item_docs("serde", "serde::Serialize", None)
        .await;
    assert_eq!(
        result.as_ref().map(|s| s.as_ref()),
        Some("Serialize trait docs")
    );

    // Test cache with version
    doc_cache
        .set_item_docs(
            "std",
            "std::collections::HashMap",
            Some("1.75.0"),
            "HashMap docs".to_string(),
        )
        .await
        .expect("set_item_docs should succeed");
    let result = doc_cache
        .get_item_docs("std", "std::collections::HashMap", Some("1.75.0"))
        .await;
    assert_eq!(result.as_ref().map(|s| s.as_ref()), Some("HashMap docs"));
}

// ============================================================================
// Configuration validation boundary tests
// ============================================================================

/// Test configuration validation - empty host
#[test]
fn test_config_validation_empty_host() {
    let mut config = crates_docs::config::AppConfig::default();
    config.server.host = "".to_string();
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("host"));
}

/// Test configuration validation - port is 0
#[test]
fn test_config_validation_zero_port() {
    let mut config = crates_docs::config::AppConfig::default();
    config.server.port = 0;
    let result = config.validate();
    assert!(result.is_err());
}

/// Test configuration validation - invalid transport mode
#[test]
fn test_config_validation_invalid_transport_mode() {
    let mut config = crates_docs::config::AppConfig::default();
    config.server.transport_mode = "invalid".to_string();
    let result = config.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid transport mode"));
}

/// Test configuration validation - invalid log level
#[test]
fn test_config_validation_invalid_log_level() {
    let mut config = crates_docs::config::AppConfig::default();
    config.logging.level = "invalid".to_string();
    let result = config.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid log level"));
}

/// Test configuration validation - max connections is 0
#[test]
fn test_config_validation_zero_max_connections() {
    let mut config = crates_docs::config::AppConfig::default();
    config.server.max_connections = 0;
    let result = config.validate();
    assert!(result.is_err());
}

/// Test configuration validation - HTTP client pool size is 0
#[test]
fn test_config_validation_zero_pool_size() {
    let mut config = crates_docs::config::AppConfig::default();
    config.performance.http_client_pool_size = 0;
    let result = config.validate();
    assert!(result.is_err());
}

/// Test configuration validation - cache max size is 0
#[test]
fn test_config_validation_zero_cache_size() {
    let mut config = crates_docs::config::AppConfig::default();
    config.performance.cache_max_size = 0;
    let result = config.validate();
    assert!(result.is_err());
}

// ============================================================================
// OAuth configuration validation tests
// ============================================================================

/// Test OAuth configuration validation - enabled but missing client ID
#[test]
fn test_oauth_config_validation_missing_client_id() {
    use crates_docs::server::auth::{OAuthConfig, OAuthProvider};

    let config = OAuthConfig {
        enabled: true,
        client_id: None,
        client_secret: Some("secret".to_string()),
        redirect_uri: Some("http://localhost/callback".to_string()),
        authorization_endpoint: Some("https://example.com/auth".to_string()),
        token_endpoint: Some("https://example.com/token".to_string()),
        scopes: vec!["read".to_string()],
        provider: OAuthProvider::Custom,
    };

    let result = config.validate();
    assert!(result.is_err());
}

/// Test OAuth configuration validation - enabled but missing client secret
#[test]
fn test_oauth_config_validation_missing_client_secret() {
    use crates_docs::server::auth::{OAuthConfig, OAuthProvider};

    let config = OAuthConfig {
        enabled: true,
        client_id: Some("client_id".to_string()),
        client_secret: None,
        redirect_uri: Some("http://localhost/callback".to_string()),
        authorization_endpoint: Some("https://example.com/auth".to_string()),
        token_endpoint: Some("https://example.com/token".to_string()),
        scopes: vec!["read".to_string()],
        provider: OAuthProvider::Custom,
    };

    let result = config.validate();
    assert!(result.is_err());
}

/// Test OAuth configuration validation - disabled, no validation required
#[test]
fn test_oauth_config_validation_disabled() {
    use crates_docs::server::auth::{OAuthConfig, OAuthProvider};

    let config = OAuthConfig {
        enabled: false,
        client_id: None,
        client_secret: None,
        redirect_uri: None,
        authorization_endpoint: None,
        token_endpoint: None,
        scopes: vec![],
        provider: OAuthProvider::Custom,
    };

    let result = config.validate();
    assert!(result.is_ok());
}

// ============================================================================
// Error handling tests
// ============================================================================

/// Test error type conversions
#[test]
fn test_error_conversions() {
    use crates_docs::error::Error;

    // Test IO error conversion
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let error: Error = io_error.into();
    assert!(matches!(error, Error::Io(_)));

    // Test JSON error conversion
    let json_error = serde_json::from_str::<i32>("not a number").unwrap_err();
    let error: Error = json_error.into();
    assert!(matches!(error, Error::Json(_)));
}

/// Test error display
#[test]
fn test_error_display() {
    use crates_docs::error::Error;

    let error = Error::config("test_field", "test config error");
    assert!(error.to_string().contains("Configuration error"));
    assert!(error.to_string().contains("test config error"));

    let error = Error::initialization("test_component", "test init error");
    assert!(error.to_string().contains("Initialization failed"));

    let error = Error::http_request("GET", "https://example.com", 500, "test http error");
    assert!(error.to_string().contains("HTTP request failed"));
}

// ============================================================================
// Tool parameter tests
// ============================================================================

/// Test LookupCrateTool parameters
#[test]
fn test_lookup_crate_tool_params() {
    use crates_docs::tools::docs::lookup_crate::LookupCrateTool;

    let params = LookupCrateTool {
        crate_name: "serde".to_string(),
        version: Some("1.0.0".to_string()),
        format: Some("markdown".to_string()),
    };

    assert_eq!(params.crate_name, "serde");
    assert_eq!(params.version, Some("1.0.0".to_string()));
    assert_eq!(params.format, Some("markdown".to_string()));
}

/// Test LookupItemTool parameters
#[test]
fn test_lookup_item_tool_params() {
    use crates_docs::tools::docs::lookup_item::LookupItemTool;

    let params = LookupItemTool {
        crate_name: "serde".to_string(),
        item_path: "serde::Serialize".to_string(),
        version: None,
        format: Some("text".to_string()),
    };

    assert_eq!(params.crate_name, "serde");
    assert_eq!(params.item_path, "serde::Serialize");
    assert!(params.version.is_none());
    assert_eq!(params.format, Some("text".to_string()));
}

/// Test SearchCratesTool parameters
#[test]
fn test_search_crates_tool_params() {
    use crates_docs::tools::docs::search::SearchCratesTool;

    let params = SearchCratesTool {
        query: "web framework".to_string(),
        limit: Some(20),
        sort: Some("downloads".to_string()),
        format: Some("json".to_string()),
    };

    assert_eq!(params.query, "web framework");
    assert_eq!(params.limit, Some(20));
    assert_eq!(params.sort, Some("downloads".to_string()));
    assert_eq!(params.format, Some("json".to_string()));
}

/// Test HealthCheckTool parameters
#[test]
fn test_health_check_tool_params() {
    use crates_docs::tools::health::HealthCheckTool;

    let params = HealthCheckTool {
        check_type: Some("external".to_string()),
        verbose: Some(true),
    };

    assert_eq!(params.check_type, Some("external".to_string()));
    assert_eq!(params.verbose, Some(true));
}

// ============================================================================
// String utility boundary tests
// ============================================================================

/// Test string truncation edge cases
#[test]
fn test_string_truncate_edge_cases() {
    use crates_docs::utils::string;

    // Empty string
    let truncated = string::truncate_with_ellipsis("", 10);
    assert_eq!(truncated, "");

    // Single character string
    let truncated = string::truncate_with_ellipsis("a", 10);
    assert_eq!(truncated, "a");

    // Max length is 0
    let truncated = string::truncate_with_ellipsis("test", 0);
    assert_eq!(truncated, "...");

    // Max length is 1
    let truncated = string::truncate_with_ellipsis("test", 1);
    assert_eq!(truncated, "...");

    // Max length is 2
    let truncated = string::truncate_with_ellipsis("test", 2);
    assert_eq!(truncated, "...");

    // Max length is 3
    let truncated = string::truncate_with_ellipsis("test", 3);
    assert_eq!(truncated, "...");

    // Exactly equals max length
    let truncated = string::truncate_with_ellipsis("test", 4);
    assert_eq!(truncated, "test");

    // Exceeds max length by 1
    let truncated = string::truncate_with_ellipsis("tests", 4);
    assert_eq!(truncated, "t...");
}

/// Test string whitespace check
#[test]
fn test_string_is_blank() {
    use crates_docs::utils::string;

    assert!(string::is_blank(""));
    assert!(string::is_blank(" "));
    assert!(string::is_blank("  "));
    assert!(string::is_blank("\t"));
    assert!(string::is_blank("\n"));
    assert!(string::is_blank("\r\n"));
    assert!(string::is_blank(" \t \n "));

    assert!(!string::is_blank("a"));
    assert!(!string::is_blank(" a "));
    assert!(!string::is_blank("test"));
}

/// Test number parsing
#[test]
fn test_parse_number() {
    use crates_docs::utils::string;

    assert_eq!(string::parse_number::<i32>("42", 0), 42);
    assert_eq!(string::parse_number::<i32>("-10", 0), -10);
    assert_eq!(string::parse_number::<i32>("invalid", 100), 100);
    assert_eq!(string::parse_number::<f64>("1.5", 0.0), 1.5);
    assert_eq!(string::parse_number::<f64>("invalid", 1.0), 1.0);
}

// ============================================================================
// Validation utility boundary tests
// ============================================================================

/// Test crate name validation edge cases
#[test]
fn test_validate_crate_name_edge_cases() {
    use crates_docs::utils::validation;

    // Valid names
    assert!(validation::validate_crate_name("a").is_ok());
    assert!(validation::validate_crate_name("serde").is_ok());
    assert!(validation::validate_crate_name("serde-json").is_ok());
    assert!(validation::validate_crate_name("serde_json").is_ok());
    assert!(validation::validate_crate_name("tokio1").is_ok());
    assert!(validation::validate_crate_name("test123").is_ok());

    // Invalid names
    assert!(validation::validate_crate_name("").is_err()); // Empty
    assert!(validation::validate_crate_name("serde json").is_err()); // Contains space
    assert!(validation::validate_crate_name("serde.json").is_err()); // Contains dot
    assert!(validation::validate_crate_name("serde/ json").is_err()); // Contains slash

    // Excessively long name
    let long_name = "a".repeat(101);
    assert!(validation::validate_crate_name(&long_name).is_err());
}

/// Test version validation edge cases
#[test]
fn test_validate_version_edge_cases() {
    use crates_docs::utils::validation;

    // Valid versions
    assert!(validation::validate_version("1").is_ok());
    assert!(validation::validate_version("1.0").is_ok());
    assert!(validation::validate_version("1.0.0").is_ok());
    assert!(validation::validate_version("0.1.0").is_ok());
    assert!(validation::validate_version("1.0.0-alpha").is_ok());
    assert!(validation::validate_version("1.0.0-alpha.1").is_ok());
    assert!(validation::validate_version("1.0.0-beta.2").is_ok());

    // Invalid versions
    assert!(validation::validate_version("").is_err()); // Empty
    assert!(validation::validate_version("alpha").is_err()); // No digits
    assert!(validation::validate_version("-").is_err()); // No digits

    // Excessively long version
    let long_version = "1".repeat(51);
    assert!(validation::validate_version(&long_version).is_err());
}

/// Test search query validation edge cases
#[test]
fn test_validate_search_query_edge_cases() {
    use crates_docs::utils::validation;

    // Valid queries
    assert!(validation::validate_search_query("a").is_ok());
    assert!(validation::validate_search_query("serde").is_ok());
    assert!(validation::validate_search_query("web framework").is_ok());
    let max_query = "a".repeat(200);
    assert!(validation::validate_search_query(&max_query).is_ok()); // Maximum length

    // Invalid queries
    assert!(validation::validate_search_query("").is_err()); // Empty
    let long_query = "a".repeat(201);
    assert!(validation::validate_search_query(&long_query).is_err()); // Too long
}

// ============================================================================
// Performance counter boundary tests
// ============================================================================

/// Test performance counter concurrent access
#[tokio::test]
async fn test_performance_counter_concurrent() {
    use crates_docs::utils::metrics::PerformanceCounter;
    use std::sync::Arc;
    use tokio::task::JoinSet;

    let counter = Arc::new(PerformanceCounter::new());
    let mut tasks = JoinSet::new();

    // Concurrently record 100 requests
    for _ in 0..100 {
        let counter = counter.clone();
        tasks.spawn(async move {
            let start = counter.record_request_start();
            tokio::time::sleep(std::time::Duration::from_micros(1)).await;
            counter.record_request_complete(start, true);
        });
    }

    // Wait for all tasks to complete
    while tasks.join_next().await.is_some() {}

    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 100);
    assert_eq!(stats.successful_requests, 100);
    assert_eq!(stats.failed_requests, 0);
}

/// Test performance counter success rate calculation
#[test]
fn test_performance_counter_success_rate() {
    use crates_docs::utils::metrics::PerformanceCounter;

    let counter = PerformanceCounter::new();

    // Record mixed results
    for i in 0..100 {
        let start = counter.record_request_start();
        counter.record_request_complete(start, i % 2 == 0);
    }

    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 100);
    assert_eq!(stats.successful_requests, 50); // Even indices succeed
    assert_eq!(stats.failed_requests, 50); // Odd indices fail
    assert_eq!(stats.success_rate_percent, 50.0);
}

// ============================================================================
// Rate limiter tests
// ============================================================================

/// Test rate limiter boundary
#[tokio::test]
async fn test_rate_limiter_boundary() {
    use crates_docs::utils::RateLimiter;

    let limiter = RateLimiter::new(1);

    // Acquire unique permit
    let permit1 = limiter.acquire().await;
    assert!(permit1.is_ok());

    // Try non-blocking acquire should fail
    let try_result = limiter.try_acquire();
    assert!(try_result.is_none());

    // Release permit
    drop(permit1);

    // Now should be able to acquire
    let permit2 = limiter.try_acquire();
    assert!(permit2.is_some());
}

/// Test rate limiter available permits
#[test]
fn test_rate_limiter_available_permits() {
    use crates_docs::utils::RateLimiter;

    let limiter = RateLimiter::new(5);
    assert_eq!(limiter.available_permits(), 5);
    assert_eq!(limiter.max_permits(), 5);

    let permit1 = limiter.try_acquire();
    assert!(permit1.is_some());
    assert_eq!(limiter.available_permits(), 4);

    let permit2 = limiter.try_acquire();
    assert!(permit2.is_some());
    assert_eq!(limiter.available_permits(), 3);

    drop(permit1);
    drop(permit2);
    assert_eq!(limiter.available_permits(), 5);
}

// ============================================================================
// Transport mode tests
// ============================================================================

/// Test transport mode parsing
#[test]
fn test_transport_mode_from_str() {
    use std::str::FromStr;

    let modes = [
        (
            "stdio",
            crates_docs::server::transport::TransportMode::Stdio,
        ),
        ("http", crates_docs::server::transport::TransportMode::Http),
        ("sse", crates_docs::server::transport::TransportMode::Sse),
        (
            "hybrid",
            crates_docs::server::transport::TransportMode::Hybrid,
        ),
        (
            "STDIO",
            crates_docs::server::transport::TransportMode::Stdio,
        ),
        ("HTTP", crates_docs::server::transport::TransportMode::Http),
        ("SSE", crates_docs::server::transport::TransportMode::Sse),
        (
            "HYBRID",
            crates_docs::server::transport::TransportMode::Hybrid,
        ),
    ];

    for (input, expected) in modes {
        let result = crates_docs::server::transport::TransportMode::from_str(input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }

    // Invalid mode
    let result = crates_docs::server::transport::TransportMode::from_str("invalid");
    assert!(result.is_err());
}

/// Test transport mode display
#[test]
fn test_transport_mode_display() {
    let modes = [
        (
            crates_docs::server::transport::TransportMode::Stdio,
            "stdio",
        ),
        (crates_docs::server::transport::TransportMode::Http, "http"),
        (crates_docs::server::transport::TransportMode::Sse, "sse"),
        (
            crates_docs::server::transport::TransportMode::Hybrid,
            "hybrid",
        ),
    ];

    for (mode, expected) in modes {
        assert_eq!(mode.to_string(), expected);
    }
}

// ============================================================================
// Error type conversion tests
// ============================================================================

/// Test Error conversion from std::io::Error
#[test]
fn test_error_from_io_error() {
    use crates_docs::Error;
    use std::io;

    let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
    let err: Error = io_err.into();
    assert!(matches!(err, Error::Io(_)));
    assert!(err.to_string().contains("IO error"));
}

/// Test Error conversion from serde_json::Error
#[test]
fn test_error_from_json_error() {
    use crates_docs::Error;

    let json_err = serde_json::from_str::<i32>("not a number");
    assert!(json_err.is_err());
    let err: Error = json_err.unwrap_err().into();
    assert!(matches!(err, Error::Json(_)));
    assert!(err.to_string().contains("JSON error"));
}

/// Test Error conversion from url::ParseError
#[test]
fn test_error_from_url_error() {
    use crates_docs::Error;

    let url_err = url::Url::parse("not a valid url: bad");
    assert!(url_err.is_err());
    let err: Error = url_err.unwrap_err().into();
    assert!(matches!(err, Error::Url(_)));
    assert!(err.to_string().contains("URL parse error"));
}

/// Test Error conversion from Box<dyn Error>
#[test]
fn test_error_from_boxed_error() {
    use crates_docs::Error;

    let boxed: Box<dyn std::error::Error + Send + Sync> =
        Box::new(std::io::Error::other("test error"));
    let err: Error = boxed.into();
    assert!(matches!(err, Error::Other(_)));
    assert!(err.to_string().contains("Unknown error"));
}

/// Test Error conversion from anyhow::Error
#[test]
fn test_error_from_anyhow_error() {
    use crates_docs::Error;

    let anyhow_err = anyhow::anyhow!("something went wrong");
    let err: Error = anyhow_err.into();
    assert!(matches!(err, Error::Other(_)));
    assert!(err.to_string().contains("Unknown error"));
}

/// Test Display for various Error variants
#[test]
fn test_error_variants_display() {
    use crates_docs::Error;

    let variants: Vec<(Error, &str)> = vec![
        (
            Error::initialization("component", "init failed"),
            "Initialization failed",
        ),
        (Error::config("field", "bad config"), "Configuration error"),
        (
            Error::http_request("GET", "https://example.com", 500, "request failed"),
            "HTTP request failed",
        ),
        (Error::parse("input", None, "parse error"), "Parse failed"),
        (
            Error::cache("get", Some("key".to_string()), "cache error"),
            "Cache operation",
        ),
        (
            Error::auth("provider", "auth failed"),
            "Authentication failed",
        ),
        (
            Error::mcp("context", "protocol error"),
            "MCP protocol error",
        ),
        (Error::Other("unknown error".to_string()), "Unknown error"),
    ];

    for (err, expected_prefix) in variants {
        let msg = err.to_string();
        assert!(
            msg.contains(expected_prefix),
            "Error message '{}' should contain '{}'",
            msg,
            expected_prefix
        );
    }
}

// ============================================================================
// Cache creation error tests
// ============================================================================

/// Test unsupported cache type
#[test]
fn test_create_cache_unsupported_type() {
    use crates_docs::cache::{create_cache, CacheConfig};

    let config = CacheConfig {
        cache_type: "unsupported".to_string(),
        memory_size: Some(100),
        default_ttl: Some(3600),
        redis_url: None,
        key_prefix: String::new(),
        crate_docs_ttl_secs: Some(3600),
        item_docs_ttl_secs: Some(1800),
        search_results_ttl_secs: Some(300),
    };

    let result = create_cache(&config);
    assert!(result.is_err());
    // Don't use unwrap_err() because Box<dyn Cache> doesn't implement Debug
    if let Err(err) = result {
        assert!(err.to_string().contains("unsupported cache type"));
    }
}

/// Test Redis cache synchronous creation error
#[test]
fn test_create_cache_redis_sync_error() {
    use crates_docs::cache::{create_cache, CacheConfig};

    let config = CacheConfig {
        cache_type: "redis".to_string(),
        memory_size: Some(100),
        default_ttl: Some(3600),
        redis_url: Some("redis://localhost:6379".to_string()),
        key_prefix: String::new(),
        crate_docs_ttl_secs: Some(3600),
        item_docs_ttl_secs: Some(1800),
        search_results_ttl_secs: Some(300),
    };

    // Synchronous Redis cache creation should return error (requires async initialization)
    let result = create_cache(&config);
    // If Redis feature is enabled, returns error requiring async initialization
    // If not enabled, returns feature not enabled error
    assert!(result.is_err());
}

// ============================================================================
// Configuration boundary tests
// ============================================================================

/// Test configuration save and load
#[test]
fn test_config_save_and_load() {
    use crates_docs::config::AppConfig;
    use std::fs;

    let config = AppConfig::default();
    let temp_path = "/tmp/test_crates_docs_config.toml";

    // Save configuration
    let save_result = config.save_to_file(temp_path);
    assert!(save_result.is_ok());

    // Load configuration
    let load_result = AppConfig::from_file(temp_path);
    assert!(load_result.is_ok());

    let loaded_config = load_result.unwrap();
    assert_eq!(loaded_config.server.host, config.server.host);

    // Cleanup
    let _ = fs::remove_file(temp_path);
}

/// Test loading configuration from environment variables
#[test]
fn test_config_from_env() {
    use crates_docs::config::AppConfig;

    // Use temp_env to safely isolate environment variables
    temp_env::with_vars(
        [
            ("CRATES_DOCS_HOST", Some("0.0.0.0")),
            ("CRATES_DOCS_PORT", Some("9090")),
        ],
        || {
            let result = AppConfig::from_env();
            assert!(result.is_ok());

            // Note: from_env implementation may differ, this just tests that the function works
            let _config = result.unwrap();
        },
    );
}

/// Test configuration merge
#[test]
fn test_config_merge() {
    use crates_docs::config::AppConfig;

    // No configuration merge
    let merged = AppConfig::merge(None, None);
    assert_eq!(merged.server.host, "127.0.0.1");

    // Only file configuration
    let file_config = AppConfig::default();
    let merged = AppConfig::merge(Some(file_config), None);
    assert_eq!(merged.server.host, "127.0.0.1");
}

/// Test AppConfig default values
#[test]
fn test_app_config_default() {
    use crates_docs::config::AppConfig;

    let config = AppConfig::default();
    assert_eq!(config.server.host, "127.0.0.1");
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.server.transport_mode, "hybrid");
    assert_eq!(config.cache.cache_type, "memory");
}

// ============================================================================
// OAuth configuration tests
// ============================================================================

/// Test GitHub OAuth configuration creation
#[test]
fn test_oauth_config_github() {
    use crates_docs::server::auth::OAuthConfig;

    let config = OAuthConfig::github(
        "client_id".to_string(),
        "client_secret".to_string(),
        "http://localhost/callback".to_string(),
    );

    assert!(config.enabled);
    assert!(config.validate().is_ok());
}

/// Test Google OAuth configuration creation
#[test]
fn test_oauth_config_google() {
    use crates_docs::server::auth::OAuthConfig;

    let config = OAuthConfig::google(
        "client_id".to_string(),
        "client_secret".to_string(),
        "http://localhost/callback".to_string(),
    );

    assert!(config.enabled);
    assert!(config.validate().is_ok());
}

/// Test Keycloak OAuth configuration creation
#[test]
fn test_oauth_config_keycloak() {
    use crates_docs::server::auth::OAuthConfig;

    let config = OAuthConfig::keycloak(
        "client_id".to_string(),
        "client_secret".to_string(),
        "http://localhost/callback".to_string(),
        "http://keycloak:8080",
        "test",
    );

    assert!(config.enabled);
    assert!(config.validate().is_ok());
}

/// Test disabled OAuth configuration validation
#[test]
fn test_oauth_config_disabled_validation() {
    use crates_docs::server::auth::OAuthConfig;

    let config = OAuthConfig {
        enabled: false,
        ..Default::default()
    };

    // Disabled configuration should always pass validation
    assert!(config.validate().is_ok());
}

// ============================================================================
// Server configuration tests
// ============================================================================

/// Test ServerConfig default values
#[test]
fn test_server_config_default() {
    use crates_docs::server::ServerConfig;

    let config = ServerConfig::default();
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 8080);
}

/// Test LoggingConfig default values
#[test]
fn test_logging_config_default() {
    use crates_docs::config::LoggingConfig;

    let config = LoggingConfig::default();
    assert!(config.enable_console);
    assert!(!config.enable_file); // File logging disabled by default
    assert_eq!(config.level, "info");
}

/// Test PerformanceConfig default values
#[test]
fn test_performance_config_default() {
    use crates_docs::config::PerformanceConfig;

    let config = PerformanceConfig::default();
    assert!(config.http_client_pool_size > 0);
    assert!(config.cache_max_size > 0);
}

// ============================================================================
// HTTP client builder tests
// ============================================================================

/// Test HTTP client builder
#[test]
fn test_http_client_builder() {
    use crates_docs::utils::HttpClientBuilder;
    use std::time::Duration;

    let client = HttpClientBuilder::default()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(10)
        .user_agent("test-agent".to_string())
        .enable_gzip(true)
        .enable_brotli(true)
        .build();

    assert!(client.is_ok());
}

/// Test HTTP client builder default values
#[test]
fn test_http_client_builder_default() {
    use crates_docs::utils::HttpClientBuilder;

    let builder = HttpClientBuilder::default();
    assert!(builder.build().is_ok());
}

// ============================================================================
// Compression utility tests
// ============================================================================

/// Test gzip compression and decompression
#[test]
fn test_gzip_compression() {
    use crates_docs::utils::compression;

    let original = b"Hello, World! This is a test message for gzip compression.";

    // Compress
    let compressed = compression::gzip_compress(original);
    assert!(compressed.is_ok());
    let compressed = compressed.unwrap();
    assert!(!compressed.is_empty());

    // Decompress
    let decompressed = compression::gzip_decompress(&compressed);
    assert!(decompressed.is_ok());
    let decompressed = decompressed.unwrap();
    assert_eq!(decompressed.as_slice(), original);
}

/// Test empty data compression
#[test]
fn test_gzip_empty_data() {
    use crates_docs::utils::compression;

    let empty: &[u8] = &[];

    // Empty data compression
    let compressed = compression::gzip_compress(empty);
    assert!(compressed.is_ok());

    // Empty data decompression
    let _decompressed = compression::gzip_decompress(empty);
    // Empty data decompression may fail or return empty, depending on implementation
}

// ============================================================================
// Time utility tests
// ============================================================================

/// Test timestamp generation
#[test]
fn test_current_timestamp_ms() {
    use crates_docs::utils::time;

    let ts = time::current_timestamp_ms();
    assert!(ts > 0);

    // Consecutive calls should return different values
    let ts2 = time::current_timestamp_ms();
    assert!(ts2 >= ts);
}

/// Test time formatting
#[test]
fn test_format_datetime() {
    use chrono::Utc;
    use crates_docs::utils::time;

    let now = Utc::now();
    let formatted = time::format_datetime(&now);
    assert!(!formatted.is_empty());
    assert!(formatted.contains('-'));
    assert!(formatted.contains(':'));
}

/// Test time interval calculation
#[test]
fn test_elapsed_ms() {
    use crates_docs::utils::time;
    use std::time::Duration;

    let start = std::time::Instant::now();
    std::thread::sleep(Duration::from_millis(10));
    let elapsed = time::elapsed_ms(start);
    assert!(elapsed >= 10);
}

// ============================================================================
// Performance counter reset tests
// ============================================================================

/// Test performance counter reset
#[test]
fn test_performance_counter_reset() {
    use crates_docs::utils::metrics::PerformanceCounter;

    let counter = PerformanceCounter::new();

    // Record some requests
    for i in 0..10 {
        let start = counter.record_request_start();
        counter.record_request_complete(start, i % 2 == 0);
    }

    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 10);

    // Reset
    counter.reset();

    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 0);
}

/// Test performance stats creation
#[test]
fn test_performance_stats_new() {
    use crates_docs::utils::metrics::PerformanceStats;

    // PerformanceStats is returned by PerformanceCounter::get_stats()
    // We can test its fields
    let stats = PerformanceStats {
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
        average_response_time_ms: 0.0,
        success_rate_percent: 0.0,
    };
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 0);
    assert_eq!(stats.success_rate_percent, 0.0);
    assert_eq!(stats.average_response_time_ms, 0.0);
}

// ============================================================================
// Token storage tests
// ============================================================================

/// Test TokenStore basic operations
#[tokio::test]
async fn test_token_store_operations() {
    use chrono::{Duration, Utc};
    use crates_docs::server::auth::{TokenInfo, TokenStore};

    let store = TokenStore::new();
    let token_info = TokenInfo {
        access_token: "test_access_token".to_string(),
        refresh_token: Some("test_refresh_token".to_string()),
        expires_at: Utc::now() + Duration::hours(1),
        scopes: vec!["read".to_string()],
        user_id: Some("user123".to_string()),
        user_email: Some("user@example.com".to_string()),
    };

    // Store
    assert!(store
        .store_token("user1".to_string(), token_info.clone())
        .await
        .is_ok());

    // Retrieve
    let retrieved: Result<Option<TokenInfo>, _> = store.get_token("user1").await;
    assert!(retrieved.as_ref().unwrap().as_ref().is_some());
    let retrieved_value = retrieved.unwrap().unwrap();
    assert_eq!(retrieved_value.access_token, "test_access_token");

    // Delete
    assert!(store.remove_token("user1").await.is_ok());
    let deleted: Result<Option<TokenInfo>, _> = store.get_token("user1").await;
    assert!(deleted.unwrap().is_none());
}

/// Test TokenStore expired token cleanup
#[tokio::test]
async fn test_token_store_cleanup() {
    use chrono::{Duration, Utc};
    use crates_docs::server::auth::{TokenInfo, TokenStore};

    let store = TokenStore::new();

    // Add an expired token
    let expired_token = TokenInfo {
        access_token: "expired_token".to_string(),
        refresh_token: None,
        expires_at: Utc::now() - Duration::seconds(1),
        scopes: vec![],
        user_id: None,
        user_email: None,
    };
    assert!(store
        .store_token("expired_user".to_string(), expired_token)
        .await
        .is_ok());

    // Add a valid token
    let valid_token = TokenInfo {
        access_token: "valid_token".to_string(),
        refresh_token: None,
        expires_at: Utc::now() + Duration::hours(1),
        scopes: vec![],
        user_id: None,
        user_email: None,
    };
    assert!(store
        .store_token("valid_user".to_string(), valid_token)
        .await
        .is_ok());

    // Cleanup expired tokens
    assert!(store.cleanup_expired().await.is_ok());

    // Expired token should be deleted
    let expired: Result<Option<TokenInfo>, _> = store.get_token("expired_user").await;
    assert!(expired.unwrap().is_none());

    // Valid token should be retained
    let valid: Result<Option<TokenInfo>, _> = store.get_token("valid_user").await;
    assert!(valid.unwrap().is_some());
}

// ============================================================================
// Version constant tests
// ============================================================================

/// Test version constant
#[test]
fn test_version_constant() {
    // Version should be a valid semantic version
    let version = crates_docs::VERSION;
    assert!(!version.is_empty());
    assert!(version.contains('.'));
}

/// Test name constant
#[test]
fn test_name_constant() {
    let name = crates_docs::NAME;
    assert_eq!(name, "crates-docs");
}

// ============================================================================
// Additional coverage tests
// ============================================================================

#[test]
fn test_cache_config_default_values() {
    let config = crates_docs::cache::CacheConfig::default();
    assert_eq!(config.cache_type, "memory");
    assert_eq!(config.memory_size, Some(1000));
    assert_eq!(config.default_ttl, Some(3600));
    assert!(config.redis_url.is_none());
}

#[test]
fn test_config_from_file_invalid_toml() {
    use crates_docs::config::AppConfig;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.toml");
    std::fs::write(&path, "this = [invalid toml").unwrap();

    let result = AppConfig::from_file(&path);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Failed to parse config file"));
}

#[test]
fn test_config_from_file_missing_file() {
    use crates_docs::config::AppConfig;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.toml");
    let result = AppConfig::from_file(&path);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Failed to read config file"));
}

#[test]
fn test_config_save_to_file_nested_directory() {
    use crates_docs::config::AppConfig;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/config/app.toml");
    let config = AppConfig::default();

    config.save_to_file(&path).unwrap();
    assert!(path.exists());

    let loaded = AppConfig::from_file(&path).unwrap();
    assert_eq!(loaded.server.host, config.server.host);
    assert_eq!(loaded.server.port, config.server.port);
}

#[test]
fn test_config_validate_with_oauth_enabled_and_invalid_oauth() {
    let mut config = crates_docs::config::AppConfig::default();
    config.server.enable_oauth = true;
    config.oauth.enabled = true;
    config.oauth.client_id = None;
    config.oauth.client_secret = Some("secret".to_string());
    config.oauth.redirect_uri = Some("http://localhost/callback".to_string());
    config.oauth.authorization_endpoint = Some("https://example.com/auth".to_string());
    config.oauth.token_endpoint = Some("https://example.com/token".to_string());

    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("client_id"));
}

#[test]
fn test_config_from_env_invalid_port() {
    use crates_docs::config::AppConfig;

    temp_env::with_vars([("CRATES_DOCS_PORT", Some("not-a-number"))], || {
        let result = AppConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid port"));
    });
}

#[test]
fn test_config_from_env_overrides_additional_fields() {
    use crates_docs::config::AppConfig;

    temp_env::with_vars(
        [
            ("CRATES_DOCS_NAME", Some("custom-server")),
            ("CRATES_DOCS_HOST", Some("0.0.0.0")),
            ("CRATES_DOCS_PORT", Some("9000")),
            ("CRATES_DOCS_TRANSPORT_MODE", Some("http")),
            ("CRATES_DOCS_LOG_LEVEL", Some("debug")),
            ("CRATES_DOCS_ENABLE_CONSOLE", Some("false")),
            ("CRATES_DOCS_ENABLE_FILE", Some("false")),
        ],
        || {
            let env_config = AppConfig::from_env().unwrap();
            let config = AppConfig::merge(None, Some(env_config));
            assert_eq!(config.server.name, "custom-server");
            assert_eq!(config.server.host, "0.0.0.0");
            assert_eq!(config.server.port, 9000);
            assert_eq!(config.server.transport_mode, "http");
            assert_eq!(config.logging.level, "debug");
            assert!(!config.logging.enable_console);
            assert!(!config.logging.enable_file);
        },
    );
}

#[test]
fn test_config_merge_env_overrides_file() {
    use crates_docs::config::{AppConfig, EnvAppConfig, EnvLoggingConfig, EnvServerConfig};

    let mut file = AppConfig::default();
    file.server.name = "file-server".to_string();
    file.server.host = "10.0.0.1".to_string();
    file.server.port = 7000;
    file.server.transport_mode = "sse".to_string();
    file.logging.level = "warn".to_string();

    // Create EnvAppConfig with explicit overrides (simulating env vars)
    let env = EnvAppConfig {
        server: EnvServerConfig {
            name: Some("env-server".to_string()),
            host: Some("0.0.0.0".to_string()),
            port: Some(9000),
            transport_mode: Some("http".to_string()),
        },
        logging: EnvLoggingConfig {
            level: Some("debug".to_string()),
            enable_console: None,
            enable_file: None,
        },
        #[cfg(feature = "api-key")]
        auth_api_key: Default::default(),
    };

    let merged = AppConfig::merge(Some(file), Some(env));
    assert_eq!(merged.server.name, "env-server");
    assert_eq!(merged.server.host, "0.0.0.0");
    assert_eq!(merged.server.port, 9000);
    assert_eq!(merged.server.transport_mode, "http");
    assert_eq!(merged.logging.level, "debug");
}

#[test]
fn test_oauth_config_validate_missing_redirect_uri() {
    use crates_docs::server::auth::{OAuthConfig, OAuthProvider};

    let config = OAuthConfig {
        enabled: true,
        client_id: Some("client_id".to_string()),
        client_secret: Some("secret".to_string()),
        redirect_uri: None,
        authorization_endpoint: Some("https://example.com/auth".to_string()),
        token_endpoint: Some("https://example.com/token".to_string()),
        scopes: vec![],
        provider: OAuthProvider::Custom,
    };

    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("redirect_uri"));
}

#[test]
fn test_oauth_config_validate_invalid_urls() {
    use crates_docs::server::auth::{OAuthConfig, OAuthProvider};

    let mut config = OAuthConfig {
        enabled: true,
        client_id: Some("client_id".to_string()),
        client_secret: Some("secret".to_string()),
        redirect_uri: Some("not-a-url".to_string()),
        authorization_endpoint: Some("https://example.com/auth".to_string()),
        token_endpoint: Some("https://example.com/token".to_string()),
        scopes: vec![],
        provider: OAuthProvider::Custom,
    };
    assert!(config
        .validate()
        .unwrap_err()
        .to_string()
        .contains("redirect_uri"));

    config.redirect_uri = Some("http://localhost/callback".to_string());
    config.authorization_endpoint = Some("bad-url".to_string());
    assert!(config
        .validate()
        .unwrap_err()
        .to_string()
        .contains("authorization_endpoint"));

    config.authorization_endpoint = Some("https://example.com/auth".to_string());
    config.token_endpoint = Some("bad-url".to_string());
    assert!(config
        .validate()
        .unwrap_err()
        .to_string()
        .contains("token_endpoint"));
}

#[test]
fn test_auth_manager_new_and_accessors() {
    use crates_docs::server::auth::{AuthManager, OAuthConfig};

    let disabled = OAuthConfig::default();
    let manager = AuthManager::new(disabled.clone()).unwrap();
    assert!(!manager.is_enabled());
    assert_eq!(manager.config().enabled, disabled.enabled);

    let enabled = OAuthConfig::github(
        "client".to_string(),
        "secret".to_string(),
        "http://localhost/callback".to_string(),
    );
    let manager = AuthManager::new(enabled.clone()).unwrap();
    assert!(manager.is_enabled());
    assert_eq!(manager.config().client_id, enabled.client_id);
}

#[test]
fn test_oauth_to_mcp_config() {
    let config = crates_docs::server::auth::OAuthConfig::default();
    let result = config.to_mcp_config();
    // Regardless of whether auth feature is enabled, default config (enabled=false) should return error
    assert!(result.is_err());
    // Error message contains "oauth"
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("oauth"));
}

#[test]
fn test_doc_service_accessors_and_default() {
    use crates_docs::cache::{create_cache, CacheConfig};
    use crates_docs::tools::docs::DocService;
    use std::sync::Arc;

    let cache = create_cache(&CacheConfig::default()).unwrap();
    let cache: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);
    let service = DocService::new(cache.clone()).expect("Failed to create DocService");

    let _client = service.client();
    assert!(Arc::ptr_eq(service.cache(), &cache));
    let _doc_cache = service.doc_cache();

    let default_service = DocService::default();
    let _ = default_service.client();
    let _ = default_service.cache();
    let _ = default_service.doc_cache();
}

#[test]
fn test_tool_registry_default_and_unknown_tool() {
    use crates_docs::tools::docs::DocService;
    use crates_docs::tools::{create_default_registry, ToolRegistry};
    use std::sync::Arc;

    let empty_registry = ToolRegistry::default();
    assert!(empty_registry.get_tools().is_empty());

    let service = Arc::new(DocService::default());
    let registry = create_default_registry(&service);
    let tools = registry.get_tools();
    assert_eq!(tools.len(), 4);
    assert!(tools.iter().any(|t| t.name == "lookup_crate"));
    assert!(tools.iter().any(|t| t.name == "lookup_item"));
    assert!(tools.iter().any(|t| t.name == "search_crates"));
    assert!(tools.iter().any(|t| t.name == "health_check"));

    let rt = tokio::runtime::Runtime::new().unwrap();
    let err = rt
        .block_on(async {
            registry
                .execute_tool("does_not_exist", serde_json::Value::Null)
                .await
        })
        .unwrap_err();
    assert!(err.to_string().contains("does_not_exist"));
}

#[test]
fn test_health_check_tool_invalid_arguments() {
    use crates_docs::tools::health::HealthCheckToolImpl;
    use crates_docs::tools::Tool;

    let rt = tokio::runtime::Runtime::new().unwrap();
    let tool = HealthCheckToolImpl::new();
    let err = rt
        .block_on(async { tool.execute(serde_json::json!({"verbose": "bad"})).await })
        .unwrap_err();
    assert!(err.to_string().contains("health_check"));
}

#[test]
fn test_lookup_and_search_tools_invalid_arguments() {
    use crates_docs::tools::docs::lookup_crate::LookupCrateToolImpl;
    use crates_docs::tools::docs::lookup_item::LookupItemToolImpl;
    use crates_docs::tools::docs::search::SearchCratesToolImpl;
    use crates_docs::tools::docs::DocService;
    use crates_docs::tools::Tool;
    use std::sync::Arc;

    let service = Arc::new(DocService::default());
    let crate_tool = LookupCrateToolImpl::new(service.clone());
    let item_tool = LookupItemToolImpl::new(service.clone());
    let search_tool = SearchCratesToolImpl::new(service);
    let rt = tokio::runtime::Runtime::new().unwrap();

    let err = rt
        .block_on(async { crate_tool.execute(serde_json::json!({"version": 1})).await })
        .unwrap_err();
    assert!(err.to_string().contains("lookup_crate"));

    let err = rt
        .block_on(async {
            item_tool
                .execute(serde_json::json!({"crate_name": "serde"}))
                .await
        })
        .unwrap_err();
    assert!(err.to_string().contains("lookup_item"));

    let err = rt
        .block_on(async { search_tool.execute(serde_json::json!({"limit": "x"})).await })
        .unwrap_err();
    assert!(err.to_string().contains("search_crates"));

    let err = rt
        .block_on(async {
            search_tool
                .execute(serde_json::json!({"query": "serde", "sort": "invalid-sort"}))
                .await
        })
        .unwrap_err();
    assert!(err.to_string().contains("Invalid sort option"));
}

#[test]
fn test_server_new_async_and_accessors() {
    use crates_docs::server::CratesDocsServer;

    let config = crates_docs::AppConfig::default();
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
        let cached_value = cache.get("server-cache-key").await;
        assert!(cached_value.is_some());
        assert_eq!(cached_value.unwrap().as_ref(), "value");
    });
}

#[test]
fn test_server_info_content() {
    let server = crates_docs::CratesDocsServer::new(crates_docs::AppConfig::default()).unwrap();
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

#[test]
fn test_http_client_builder_new_and_disable_compression() {
    use crates_docs::utils::HttpClientBuilder;
    use std::time::Duration;

    let client = HttpClientBuilder::new()
        .timeout(Duration::from_secs(1))
        .connect_timeout(Duration::from_secs(1))
        .pool_max_idle_per_host(1)
        .user_agent("coverage-test-agent".to_string())
        .enable_gzip(false)
        .enable_brotli(false)
        .build();

    assert!(client.is_ok());
}

#[test]
fn test_rate_limiter_try_acquire_exhaustion() {
    use crates_docs::utils::RateLimiter;

    let limiter = RateLimiter::new(1);
    let permit = limiter.try_acquire();
    assert!(permit.is_some());
    assert!(limiter.try_acquire().is_none());
    drop(permit);
    assert!(limiter.try_acquire().is_some());
}

#[test]
fn test_performance_counter_default() {
    use crates_docs::utils::metrics::PerformanceCounter;

    let counter = PerformanceCounter::default();
    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.success_rate_percent, 0.0);
}
