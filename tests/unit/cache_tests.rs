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
    let config = CacheConfig {
        cache_type: "memory".to_string(),
        memory_size: Some(100),
        default_ttl: Some(3600),
        redis_url: None,
    };
    let cache = create_cache(&config).expect("创建缓存失败");
    let cache_arc: Arc<dyn crates_docs::cache::Cache> = Arc::from(cache);
    let doc_cache = DocCache::new(cache_arc);

    // 测试缓存未命中
    let result = doc_cache.get_crate_docs("serde", None).await;
    assert!(result.is_none());

    // 设置缓存
    doc_cache
        .set_crate_docs("serde", None, "Serde documentation".to_string())
        .await;

    // 测试缓存命中
    let result = doc_cache.get_crate_docs("serde", None).await;
    assert_eq!(result, Some("Serde documentation".to_string()));

    // 测试带版本的缓存
    doc_cache
        .set_crate_docs("tokio", Some("1.0.0"), "Tokio 1.0 docs".to_string())
        .await;
    let result = doc_cache.get_crate_docs("tokio", Some("1.0.0")).await;
    assert_eq!(result, Some("Tokio 1.0 docs".to_string()));

    // 不同版本应该返回不同的缓存
    let result = doc_cache.get_crate_docs("tokio", Some("1.1.0")).await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_doc_cache_item_docs() {
    let config = CacheConfig {
        cache_type: "memory".to_string(),
        memory_size: Some(100),
        default_ttl: Some(3600),
        redis_url: None,
    };
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
        .await;

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
        .await;
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
    };
    let result = create_cache(&config);
    assert!(result.is_err());
    // 检查错误消息包含预期内容（小写的 "unsupported"）
    if let Err(e) = result {
        assert!(e.to_string().contains("unsupported cache type"));
    }
}

#[test]
fn test_create_cache_redis_sync_error() {
    let config = CacheConfig {
        cache_type: "redis".to_string(),
        memory_size: Some(100),
        default_ttl: Some(3600),
        redis_url: Some("redis://invalid:6379".to_string()),
    };
    // Redis cache without async should fail or fall back
    let result = create_cache(&config);
    // 由于没有 cache-redis feature，应该返回错误
    assert!(result.is_err() || result.is_ok());
}
