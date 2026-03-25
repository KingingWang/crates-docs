//! 缓存模块单元测试

use crates_docs::{
    cache::{create_cache, CacheConfig},
    tools::docs::cache::DocCache,
};
use std::sync::Arc;

// ============================================================================
// CacheConfig 测试
// ============================================================================

#[test]
fn test_cache_config_default_values() {
    let config = CacheConfig::default();
    assert_eq!(config.cache_type, "memory");
    assert_eq!(config.memory_size, Some(1000));
    assert_eq!(config.default_ttl, Some(3600));
    assert!(config.redis_url.is_none());
}

// ============================================================================
// DocCache 测试
// ============================================================================

#[tokio::test]
async fn test_doc_cache_crate_docs() {
    let config = CacheConfig::default();
    let cache = create_cache(&config).expect("创建缓存失败");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);
    let doc_cache = DocCache::new(cache_arc);

    // 测试缓存未命中
    let result = doc_cache.get_crate_docs("serde", None).await;
    assert!(result.is_none());

    // 设置缓存
    doc_cache
        .set_crate_docs("serde", None, "Serde documentation".to_string())
        .await
        .expect("set_crate_docs should succeed");

    // 测试缓存命中
    let result = doc_cache.get_crate_docs("serde", None).await;
    assert_eq!(result, Some("Serde documentation".to_string()));

    // 测试带版本的缓存
    doc_cache
        .set_crate_docs("tokio", Some("1.0.0"), "Tokio 1.0 docs".to_string())
        .await
        .expect("set_crate_docs should succeed");
    let result = doc_cache.get_crate_docs("tokio", Some("1.0.0")).await;
    assert_eq!(result, Some("Tokio 1.0 docs".to_string()));

    // 不同版本应该返回不同的缓存
    let result = doc_cache.get_crate_docs("tokio", Some("1.1.0")).await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_doc_cache_item_docs() {
    let config = CacheConfig::default();
    let cache = create_cache(&config).expect("创建缓存失败");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);
    let doc_cache = DocCache::new(cache_arc);

    // 测试缓存未命中
    let result = doc_cache
        .get_item_docs("serde", "serde::Serialize", None)
        .await;
    assert!(result.is_none());

    // 设置缓存
    doc_cache
        .set_item_docs(
            "serde",
            "serde::Serialize",
            None,
            "Serialize trait docs".to_string(),
        )
        .await
        .expect("set_item_docs should succeed");

    // 测试缓存命中
    let result = doc_cache
        .get_item_docs("serde", "serde::Serialize", None)
        .await;
    assert_eq!(result, Some("Serialize trait docs".to_string()));

    // 测试带版本的缓存
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
    assert_eq!(result, Some("HashMap docs".to_string()));
}

// ============================================================================
// create_cache 错误路径测试
// ============================================================================

#[test]
fn test_create_cache_unsupported_type() {
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
    // 检查错误消息包含预期内容（小写的 "unsupported"）
    if let Err(e) = result {
        assert!(e.to_string().contains("unsupported cache type"));
    }
}

#[cfg(feature = "cache-redis")]
#[test]
fn test_create_cache_redis_sync_error() {
    let config = CacheConfig {
        cache_type: "redis".to_string(),
        memory_size: Some(100),
        default_ttl: Some(3600),
        redis_url: Some("redis://invalid:6379".to_string()),
        key_prefix: String::new(),
        crate_docs_ttl_secs: Some(3600),
        item_docs_ttl_secs: Some(1800),
        search_results_ttl_secs: Some(300),
    };

    let result = create_cache(&config);
    assert!(result.is_err());

    if let Err(e) = result {
        assert!(e.to_string().contains("async initialization"));
    }
}

#[cfg(not(feature = "cache-redis"))]
#[test]
fn test_create_cache_redis_sync_error() {
    let config = CacheConfig {
        cache_type: "redis".to_string(),
        memory_size: Some(100),
        default_ttl: Some(3600),
        redis_url: Some("redis://invalid:6379".to_string()),
        key_prefix: String::new(),
        crate_docs_ttl_secs: Some(3600),
        item_docs_ttl_secs: Some(1800),
        search_results_ttl_secs: Some(300),
    };

    let result = create_cache(&config);
    assert!(result.is_err());

    if let Err(e) = result {
        assert!(e.to_string().contains("feature is not enabled"));
    }
}

// ============================================================================
// TTL 抖动测试
// ============================================================================

use crates_docs::tools::docs::cache::DocCacheTtl;

#[test]
fn test_doc_cache_ttl_default_includes_jitter() {
    let ttl = DocCacheTtl::default();
    assert_eq!(ttl.crate_docs_secs, 3600);
    assert_eq!(ttl.search_results_secs, 300);
    assert_eq!(ttl.item_docs_secs, 1800);
    // 默认 jitter 为 10%
    assert!((ttl.jitter_ratio - 0.1).abs() < f64::EPSILON);
}

#[test]
fn test_apply_jitter_zero_ratio_returns_base_ttl() {
    let ttl = DocCacheTtl {
        crate_docs_secs: 3600,
        search_results_secs: 300,
        item_docs_secs: 1800,
        jitter_ratio: 0.0,
    };

    // jitter_ratio 为 0 时应该返回原始值
    assert_eq!(ttl.apply_jitter(3600), 3600);
    assert_eq!(ttl.apply_jitter(300), 300);
}

#[test]
fn test_apply_jitter_within_expected_range() {
    let ttl = DocCacheTtl {
        crate_docs_secs: 3600,
        search_results_secs: 300,
        item_docs_secs: 1800,
        jitter_ratio: 0.1, // 10% jitter
    };

    // 多次调用确保结果在预期范围内
    for _ in 0..100 {
        let result = ttl.apply_jitter(3600);
        // 10% jitter 意味着结果应该在 [3240, 3960] 范围内
        assert!(
            result >= 3240,
            "jitter result {result} is below minimum 3240"
        );
        assert!(
            result <= 3960,
            "jitter result {result} is above maximum 3960"
        );
    }
}

#[test]
fn test_apply_jitter_clamps_to_valid_range() {
    // 测试负的 jitter_ratio（应该被 clamp 到 0）
    let ttl_negative = DocCacheTtl {
        crate_docs_secs: 3600,
        search_results_secs: 300,
        item_docs_secs: 1800,
        jitter_ratio: -0.5,
    };
    // 负值应该被当作 0 处理，返回原始值
    assert_eq!(ttl_negative.apply_jitter(3600), 3600);

    // 测试超过 1.0 的 jitter_ratio（应该被 clamp 到 1.0）
    let ttl_high = DocCacheTtl {
        crate_docs_secs: 3600,
        search_results_secs: 300,
        item_docs_secs: 1800,
        jitter_ratio: 2.0,
    };
    // 200% jitter 意味着结果可以在 [0, 7200] 范围内，但最小值为 1
    for _ in 0..100 {
        let result = ttl_high.apply_jitter(3600);
        assert!(result >= 1, "jitter result {result} should be at least 1");
        assert!(
            result <= 7200,
            "jitter result {result} should be at most 7200"
        );
    }
}

#[test]
fn test_apply_jitter_different_base_values() {
    let ttl = DocCacheTtl {
        crate_docs_secs: 3600,
        search_results_secs: 300,
        item_docs_secs: 1800,
        jitter_ratio: 0.1,
    };

    // 测试不同的基础 TTL 值
    let base_values = [60, 300, 1800, 3600, 7200];

    for &base in &base_values {
        let expected_min = (base as f64 * 0.9) as u64;
        let expected_max = (base as f64 * 1.1) as u64;

        for _ in 0..50 {
            let result = ttl.apply_jitter(base);
            assert!(
                result >= expected_min && result <= expected_max,
                "jitter result {result} for base {base} is outside expected range [{expected_min}, {expected_max}]"
            );
        }
    }
}
