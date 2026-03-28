//! Cache module unit tests

use crates_docs::{
    cache::{create_cache, CacheConfig},
    tools::docs::cache::DocCache,
};
use std::sync::Arc;

// ============================================================================
// CacheConfig tests
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
// DocCache tests
// ============================================================================

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
    assert_eq!(result, Some("Serde documentation".to_string()));

    // Test cache with version
    doc_cache
        .set_crate_docs("tokio", Some("1.0.0"), "Tokio 1.0 docs".to_string())
        .await
        .expect("set_crate_docs should succeed");
    let result = doc_cache.get_crate_docs("tokio", Some("1.0.0")).await;
    assert_eq!(result, Some("Tokio 1.0 docs".to_string()));

    // Different versions should return different cached values
    let result = doc_cache.get_crate_docs("tokio", Some("1.1.0")).await;
    assert!(result.is_none());
}

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
    assert_eq!(result, Some("Serialize trait docs".to_string()));

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
    assert_eq!(result, Some("HashMap docs".to_string()));
}

// ============================================================================
// create_cache error path tests
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
    // Check that error message contains expected content (lowercase "unsupported")
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
// TTL jitter tests
// ============================================================================

use crates_docs::tools::docs::cache::DocCacheTtl;

#[test]
fn test_doc_cache_ttl_default_includes_jitter() {
    let ttl = DocCacheTtl::default();
    assert_eq!(ttl.crate_docs_secs, 3600);
    assert_eq!(ttl.search_results_secs, 300);
    assert_eq!(ttl.item_docs_secs, 1800);
    // Default jitter is 10%
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

    // When jitter_ratio is 0, should return original value
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

    // Call multiple times to ensure results are within expected range
    for _ in 0..100 {
        let result = ttl.apply_jitter(3600);
        // 10% jitter means result should be in [3240, 3960] range
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
    // Test negative jitter_ratio (should be clamped to 0)
    let ttl_negative = DocCacheTtl {
        crate_docs_secs: 3600,
        search_results_secs: 300,
        item_docs_secs: 1800,
        jitter_ratio: -0.5,
    };
    // Negative values should be treated as 0, returning original value
    assert_eq!(ttl_negative.apply_jitter(3600), 3600);

    // Test jitter_ratio above 1.0 (should be clamped to 1.0)
    let ttl_high = DocCacheTtl {
        crate_docs_secs: 3600,
        search_results_secs: 300,
        item_docs_secs: 1800,
        jitter_ratio: 2.0,
    };
    // 200% jitter means result can be in [0, 7200] range, but minimum is 1
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

    // Test different base TTL values
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
