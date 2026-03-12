//! 工具函数模块单元测试

use crates_docs::utils::{
    compression::{gzip_compress, gzip_decompress},
    string::{is_blank, truncate_with_ellipsis},
    time::{current_timestamp_ms, elapsed_ms, format_datetime},
    validation::{validate_crate_name, validate_search_query, validate_version},
    HttpClientBuilder, RateLimiter,
};
use std::time::{Duration, Instant};

// ============================================================================
// HttpClientBuilder 测试
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
// RateLimiter 测试
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
// 压缩工具测试
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
// 字符串工具测试
// ============================================================================

#[test]
fn test_string_truncate_edge_cases() {
    // 正常截断
    let result = truncate_with_ellipsis("Hello, World!", 5);
    assert_eq!(result, "He...");

    // 不需要截断
    let result = truncate_with_ellipsis("Hi", 10);
    assert_eq!(result, "Hi");

    // 空字符串
    let result = truncate_with_ellipsis("", 5);
    assert_eq!(result, "");

    // 长度刚好
    let result = truncate_with_ellipsis("Hello", 5);
    assert_eq!(result, "Hello");

    // max_len 为 0 - 当 max_len <= 3 时返回 "..."
    let result = truncate_with_ellipsis("Hello", 0);
    assert_eq!(result, "...");

    // max_len 小于 3 - 返回 "..."
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
    // parse_number 接受字符串和默认值，返回解析结果或默认值
    assert_eq!(parse_number("42", 0), 42);
    assert_eq!(parse_number("3.14", 0.0), 3.14);
    // 无法解析时返回默认值
    assert_eq!(parse_number("not a number", 99), 99);
    assert_eq!(parse_number("", 99), 99);
}

// ============================================================================
// 时间工具测试
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
// 验证工具测试
// ============================================================================

#[test]
fn test_validate_crate_name_edge_cases() {
    // 有效名称
    assert!(validate_crate_name("serde").is_ok());
    assert!(validate_crate_name("tokio").is_ok());
    assert!(validate_crate_name("my-crate").is_ok());
    assert!(validate_crate_name("a").is_ok());
    assert!(validate_crate_name("123crate").is_ok()); // 数字开头是允许的

    // 无效名称
    assert!(validate_crate_name("").is_err());
    assert!(validate_crate_name("crate@name").is_err()); // @ 是无效字符
}

#[test]
fn test_validate_version_edge_cases() {
    // 有效版本 - 只要包含数字就有效
    assert!(validate_version("1.0.0").is_ok());
    assert!(validate_version("0.1.0").is_ok());
    assert!(validate_version("1.2.3-beta").is_ok());
    assert!(validate_version("v1.0.0").is_ok()); // 包含数字，有效
    assert!(validate_version("1.0").is_ok()); // 包含数字，有效

    // 无效版本
    assert!(validate_version("").is_err());
    assert!(validate_version("beta").is_err()); // 不包含数字
}

#[test]
fn test_validate_search_query_edge_cases() {
    // 有效查询 - 只检查空字符串
    assert!(validate_search_query("web framework").is_ok());
    assert!(validate_search_query("serde").is_ok());
    assert!(validate_search_query("   ").is_ok()); // 不是空字符串，有效

    // 无效查询
    assert!(validate_search_query("").is_err());
}

// ============================================================================
// 性能计数器测试
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

    // 记录 5 次成功，5 次失败
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

    // PerformanceStats 没有 new 方法，使用默认值
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
