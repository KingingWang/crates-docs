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

    // Invalid names
    assert!(validate_crate_name("").is_err());
    assert!(validate_crate_name("crate@name").is_err()); // @ is invalid character
}

#[test]
fn test_validate_version_edge_cases() {
    // Valid versions - valid as long as contains digits
    assert!(validate_version("1.0.0").is_ok());
    assert!(validate_version("0.1.0").is_ok());
    assert!(validate_version("1.2.3-beta").is_ok());
    assert!(validate_version("v1.0.0").is_ok()); // Contains digits, valid
    assert!(validate_version("1.0").is_ok()); // Contains digits, valid

    // Invalid versions
    assert!(validate_version("").is_err());
    assert!(validate_version("beta").is_err()); // No digits
}

#[test]
fn test_validate_search_query_edge_cases() {
    // Valid queries - only checks for empty string
    assert!(validate_search_query("web framework").is_ok());
    assert!(validate_search_query("serde").is_ok());
    assert!(validate_search_query("   ").is_ok()); // Not empty string, valid

    // Invalid queries
    assert!(validate_search_query("").is_err());
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
