//! Utility function module unit tests

use crates_docs::utils::{
    compression::{gzip_compress, gzip_decompress},
    string::{is_blank, truncate_with_ellipsis},
    time::{current_timestamp_ms, elapsed_ms, format_datetime},
    validation::{validate_crate_name, validate_search_query, validate_version},
    HttpClientBuilder, RateLimiter,
};
use std::time::{Duration, Instant};

// ============================================================================
// HttpClientBuilder tests
// ============================================================================

#[test]
fn test_http_client_builder() {
    let client = HttpClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(5)
        .user_agent("test-agent".to_string())
        .build();
    assert!(client.is_ok());
}

#[test]
fn test_http_client_builder_default() {
    let client = HttpClientBuilder::default().build();
    assert!(client.is_ok());
}

#[test]
fn test_http_client_builder_new_and_disable_compression() {
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
fn test_http_client_builder_all_methods() {
    let client = HttpClientBuilder::new()
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(45))
        .pool_max_idle_per_host(20)
        .pool_idle_timeout(Duration::from_secs(120))
        .user_agent("test-all-methods".to_string())
        .enable_gzip(true)
        .enable_brotli(true)
        .max_retries(5)
        .retry_initial_delay(Duration::from_millis(200))
        .retry_max_delay(Duration::from_secs(20))
        .build();
    assert!(client.is_ok());
}

#[test]
fn test_http_client_builder_build_plain() {
    let client = HttpClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(5)
        .pool_idle_timeout(Duration::from_secs(90))
        .user_agent("plain-client-test".to_string())
        .enable_gzip(false)
        .enable_brotli(false)
        .build_plain();
    assert!(client.is_ok());
}

#[test]
fn test_create_http_client_from_config() {
    use crates_docs::config::PerformanceConfig;
    use crates_docs::utils::create_http_client_from_config;

    let config = PerformanceConfig {
        http_client_timeout_secs: 45,
        http_client_connect_timeout_secs: 15,
        http_client_read_timeout_secs: 40,
        http_client_pool_size: 15,
        http_client_pool_idle_timeout_secs: 100,
        http_client_max_retries: 4,
        http_client_retry_initial_delay_ms: 150,
        http_client_retry_max_delay_ms: 15000,
        cache_max_size: 1000,
        cache_default_ttl_secs: 3600,
        rate_limit_per_second: 10,
        concurrent_request_limit: 100,
        enable_response_compression: true,
        enable_metrics: false,
        metrics_port: 0,
    };

    let client = create_http_client_from_config(&config).build();
    assert!(client.is_ok());
}

// ============================================================================
// RateLimiter tests
// ============================================================================

#[test]
fn test_rate_limiter_boundary() {
    let limiter = RateLimiter::new(2);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let permit1 = limiter.acquire().await.unwrap();
        let permit2 = limiter.acquire().await.unwrap();
        drop(permit1);
        drop(permit2);
    });
}

#[test]
fn test_rate_limiter_available_permits() {
    let limiter = RateLimiter::new(5);
    assert_eq!(limiter.available_permits(), 5);

    let permit = limiter.try_acquire();
    assert!(permit.is_some());
    assert_eq!(limiter.available_permits(), 4);
}

#[test]
fn test_rate_limiter_try_acquire_exhaustion() {
    let limiter = RateLimiter::new(1);
    let permit = limiter.try_acquire();
    assert!(permit.is_some());
    assert!(limiter.try_acquire().is_none());
    drop(permit);
    assert!(limiter.try_acquire().is_some());
}

#[test]
fn test_rate_limiter_max_permits() {
    let limiter = RateLimiter::new(10);
    assert_eq!(limiter.max_permits(), 10);

    let limiter2 = RateLimiter::new(1);
    assert_eq!(limiter2.max_permits(), 1);

    let limiter3 = RateLimiter::new(100);
    assert_eq!(limiter3.max_permits(), 100);
}

#[tokio::test]
async fn test_rate_limiter_acquire_success() {
    let limiter = RateLimiter::new(3);
    assert_eq!(limiter.available_permits(), 3);

    let permit = limiter.acquire().await.unwrap();
    assert_eq!(limiter.available_permits(), 2);
    drop(permit);
    assert_eq!(limiter.available_permits(), 3);
}

// ============================================================================
// Compression utility tests
// ============================================================================

#[test]
fn test_gzip_compression() {
    let data = b"Hello, World! This is a test of gzip compression.";
    let compressed = gzip_compress(data).unwrap();
    assert!(!compressed.is_empty());

    let decompressed = gzip_decompress(&compressed).unwrap();
    assert_eq!(data.to_vec(), decompressed);
}

#[test]
fn test_gzip_empty_data() {
    let data = b"";
    let compressed = gzip_compress(data).unwrap();
    let decompressed = gzip_decompress(&compressed).unwrap();
    assert!(decompressed.is_empty());
}

#[test]
fn test_gzip_decompress_invalid_data() {
    // Try to decompress invalid gzip data
    let invalid_data = b"this is not gzipped data";
    let result = gzip_decompress(invalid_data);
    assert!(result.is_err());
}

#[test]
fn test_gzip_compress_large_data() {
    // Test compression with larger data
    let data = vec![0u8; 10000];
    let compressed = gzip_compress(&data).unwrap();
    assert!(compressed.len() < data.len()); // Should be smaller after compression

    let decompressed = gzip_decompress(&compressed).unwrap();
    assert_eq!(data, decompressed);
}

#[test]
fn test_gzip_roundtrip_various_data() {
    // Test with various data patterns
    let test_cases = vec![
        b"short".to_vec(),
        b"a".repeat(100),
        (0..=255u8).collect::<Vec<_>>(),
        "Unicode: 你好世界 🎉 𝕏𝕏𝕏".as_bytes().to_vec(),
    ];

    for data in test_cases {
        let compressed = gzip_compress(&data).unwrap();
        let decompressed = gzip_decompress(&compressed).unwrap();
        assert_eq!(data, decompressed);
    }
}

// ============================================================================
// String utility tests
// ============================================================================

#[test]
fn test_string_truncate_edge_cases() {
    // Normal truncation
    let result = truncate_with_ellipsis("Hello, World!", 5);
    assert_eq!(result, "He...");

    // No truncation needed
    let result = truncate_with_ellipsis("Hi", 10);
    assert_eq!(result, "Hi");

    // Empty string
    let result = truncate_with_ellipsis("", 5);
    assert_eq!(result, "");

    // Exact length
    let result = truncate_with_ellipsis("Hello", 5);
    assert_eq!(result, "Hello");

    // max_len is 0 - when max_len <= 3, returns "..."
    let result = truncate_with_ellipsis("Hello", 0);
    assert_eq!(result, "...");

    // max_len less than 3 - returns "..."
    let result = truncate_with_ellipsis("Hello", 2);
    assert_eq!(result, "...");

    // max_len equals 3 - returns "..."
    let result = truncate_with_ellipsis("Hello", 3);
    assert_eq!(result, "...");

    // max_len is 4 - minimal truncation with ellipsis
    let result = truncate_with_ellipsis("Hello World", 4);
    assert_eq!(result, "H...");

    // UTF-8 multi-byte characters
    let result = truncate_with_ellipsis("你好世界", 3);
    assert_eq!(result, "...");

    let result = truncate_with_ellipsis("你好世界", 4);
    assert_eq!(result, "你好世界");

    let result = truncate_with_ellipsis("你好世界测试", 5);
    assert_eq!(result, "你好...");

    let result = truncate_with_ellipsis("你好世界测试", 6);
    assert_eq!(result, "你好世界测试");

    let result = truncate_with_ellipsis("你好世界测试数据", 7);
    assert_eq!(result, "你好世界...");

    let result = truncate_with_ellipsis("你好世界测试数据", 8);
    assert_eq!(result, "你好世界测试数据");

    let result = truncate_with_ellipsis("你好世界测试数据更多", 9);
    assert_eq!(result, "你好世界测试...");

    // Long string truncation
    let long_str = "a".repeat(100);
    let result = truncate_with_ellipsis(&long_str, 20);
    assert_eq!(result.len(), 20);
    assert!(result.ends_with("..."));
}

#[test]
fn test_string_is_blank() {
    assert!(is_blank(""));
    assert!(is_blank("   "));
    assert!(is_blank("\t\n"));
    assert!(!is_blank("hello"));
    assert!(!is_blank("  hello  "));
}

#[test]
fn test_parse_number() {
    use crates_docs::utils::string::parse_number;
    // parse_number accepts string and default value, returns parsed result or default
    assert_eq!(parse_number("42", 0), 42);
    assert_eq!(parse_number("2.5", 0.0), 2.5);
    // Returns default value when parsing fails
    assert_eq!(parse_number("not a number", 99), 99);
    assert_eq!(parse_number("", 99), 99);
}

// ============================================================================
// Time utility tests
// ============================================================================

#[test]
fn test_current_timestamp_ms() {
    let ts = current_timestamp_ms();
    assert!(ts > 0);

    let ts2 = current_timestamp_ms();
    assert!(ts2 >= ts);
}

#[test]
fn test_format_datetime() {
    use chrono::{TimeZone, Utc};
    let dt = Utc.timestamp_millis_opt(1700000000000).single().unwrap();
    let result = format_datetime(&dt);
    assert!(!result.is_empty());
    assert!(result.contains("2023"));
}

#[test]
fn test_elapsed_ms() {
    let start = Instant::now();
    std::thread::sleep(Duration::from_millis(10));
    let elapsed = elapsed_ms(start);
    assert!(elapsed >= 10);
}

// ============================================================================
// Validation utility tests
// ============================================================================

#[test]
fn test_validate_crate_name_edge_cases() {
    // Valid names
    assert!(validate_crate_name("serde").is_ok());
    assert!(validate_crate_name("tokio").is_ok());
    assert!(validate_crate_name("my-crate").is_ok());
    assert!(validate_crate_name("a").is_ok());
    assert!(validate_crate_name("123crate").is_ok()); // Starting with digits is allowed
    assert!(validate_crate_name("my_crate_name").is_ok());
    assert!(validate_crate_name("crate-name-123").is_ok());

    // Invalid names
    assert!(validate_crate_name("").is_err());
    assert!(validate_crate_name("crate@name").is_err()); // @ is invalid character
    assert!(validate_crate_name("crate name").is_err()); // space is invalid
    assert!(validate_crate_name("crate.name").is_err()); // . is invalid
    assert!(validate_crate_name("crate/name").is_err()); // / is invalid
    assert!(validate_crate_name("crate\\name").is_err()); // \ is invalid
    assert!(validate_crate_name("crate:name").is_err()); // : is invalid
    assert!(validate_crate_name("crate!name").is_err()); // ! is invalid

    // Too long name (> 100 chars)
    let long_name = "a".repeat(101);
    assert!(validate_crate_name(&long_name).is_err());

    // Exactly 100 chars should be valid
    let max_len_name = "a".repeat(100);
    assert!(validate_crate_name(&max_len_name).is_ok());
}

#[test]
fn test_validate_version_edge_cases() {
    // Valid versions - valid as long as contains digits
    assert!(validate_version("1.0.0").is_ok());
    assert!(validate_version("0.1.0").is_ok());
    assert!(validate_version("1.2.3-beta").is_ok());
    assert!(validate_version("v1.0.0").is_ok()); // Contains digits, valid
    assert!(validate_version("1.0").is_ok()); // Contains digits, valid
    assert!(validate_version("2").is_ok());
    assert!(validate_version("0.0.0").is_ok());
    assert!(validate_version("2023.12.01").is_ok());

    // Invalid versions
    assert!(validate_version("").is_err());
    assert!(validate_version("beta").is_err()); // No digits
    assert!(validate_version("alpha-beta").is_err()); // No digits
    assert!(validate_version("v").is_err()); // No digits

    // Too long version (> 50 chars)
    let long_version = format!("{}.{}.{}", "1".repeat(20), "2".repeat(20), "3".repeat(20));
    assert!(validate_version(&long_version).is_err());

    // Exactly 50 chars should be valid
    let max_len_version = "1.".to_string() + &"0".repeat(48);
    assert!(validate_version(&max_len_version).is_ok());
}

#[test]
fn test_validate_search_query_edge_cases() {
    // Valid queries - only checks for empty string
    assert!(validate_search_query("web framework").is_ok());
    assert!(validate_search_query("serde").is_ok());
    assert!(validate_search_query("   ").is_ok()); // Not empty string, valid
    assert!(validate_search_query("tokio async runtime").is_ok());
    assert!(validate_search_query("http client").is_ok());

    // Invalid queries
    assert!(validate_search_query("").is_err());

    // Too long query (> 200 chars)
    let long_query = "a".repeat(201);
    assert!(validate_search_query(&long_query).is_err());

    // Exactly 200 chars should be valid
    let max_len_query = "a".repeat(200);
    assert!(validate_search_query(&max_len_query).is_ok());
}

// ============================================================================
// Performance counter tests
// ============================================================================

#[test]
fn test_performance_counter_concurrent() {
    use crates_docs::utils::metrics::PerformanceCounter;
    use std::sync::Arc;
    use std::thread;

    let counter = Arc::new(PerformanceCounter::new());
    let mut handles = vec![];

    for _ in 0..10 {
        let counter_clone = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let start = counter_clone.record_request_start();
                counter_clone.record_request_complete(start, true);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 1000);
    assert_eq!(stats.successful_requests, 1000);
}

#[test]
fn test_performance_counter_success_rate() {
    use crates_docs::utils::metrics::PerformanceCounter;

    let counter = PerformanceCounter::new();

    // Record 5 successes, 5 failures
    for i in 0..10 {
        let start = counter.record_request_start();
        counter.record_request_complete(start, i < 5);
    }

    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 10);
    assert_eq!(stats.successful_requests, 5);
    assert_eq!(stats.failed_requests, 5);
    assert!((stats.success_rate_percent - 50.0).abs() < 0.01);
}

#[test]
fn test_performance_counter_reset() {
    use crates_docs::utils::metrics::PerformanceCounter;

    let counter = PerformanceCounter::new();
    let start = counter.record_request_start();
    counter.record_request_complete(start, true);

    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 1);

    counter.reset();
    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 0);
}

#[test]
fn test_performance_stats_default() {
    use crates_docs::utils::metrics::PerformanceStats;

    // PerformanceStats has no new method, use default values
    let stats = PerformanceStats {
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
        success_rate_percent: 0.0,
        average_response_time_ms: 0.0,
    };
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 0);
    assert_eq!(stats.success_rate_percent, 0.0);
}

#[test]
fn test_performance_counter_default() {
    use crates_docs::utils::metrics::PerformanceCounter;

    let counter = PerformanceCounter::default();
    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.success_rate_percent, 0.0);
}

#[test]
fn test_performance_counter_zero_requests() {
    use crates_docs::utils::metrics::PerformanceCounter;

    let counter = PerformanceCounter::new();
    let stats = counter.get_stats();

    // When no requests, average response time and success rate should be 0
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 0);
    assert_eq!(stats.average_response_time_ms, 0.0);
    assert_eq!(stats.success_rate_percent, 0.0);
}

#[test]
fn test_performance_counter_all_failed() {
    use crates_docs::utils::metrics::PerformanceCounter;

    let counter = PerformanceCounter::new();

    for _ in 0..10 {
        let start = counter.record_request_start();
        counter.record_request_complete(start, false);
    }

    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 10);
    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 10);
    assert_eq!(stats.success_rate_percent, 0.0);
}

#[test]
fn test_performance_counter_all_success() {
    use crates_docs::utils::metrics::PerformanceCounter;

    let counter = PerformanceCounter::new();

    for _ in 0..10 {
        let start = counter.record_request_start();
        counter.record_request_complete(start, true);
    }

    let stats = counter.get_stats();
    assert_eq!(stats.total_requests, 10);
    assert_eq!(stats.successful_requests, 10);
    assert_eq!(stats.failed_requests, 0);
    assert_eq!(stats.success_rate_percent, 100.0);
}

#[test]
fn test_performance_counter_clone() {
    use crates_docs::utils::metrics::PerformanceCounter;

    let counter1 = PerformanceCounter::new();
    let counter2 = counter1.clone();

    let start = counter1.record_request_start();
    counter1.record_request_complete(start, true);

    // Cloned counter shares the same state
    let stats1 = counter1.get_stats();
    let stats2 = counter2.get_stats();
    assert_eq!(stats1.total_requests, stats2.total_requests);
}
