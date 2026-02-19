//! Redis cache implementation
//!
//! Provides Redis backend cache support.

use std::time::Duration;

use crate::error::Error;

/// Redis cache implementation
///
/// Uses multiplexed connection (`MultiplexedConnection`) to avoid creating new connections for each operation.
/// Multiplexed connections can be safely cloned and shared across multiple tasks.
pub struct RedisCache {
    /// Multiplexed connection (cloneable, shared across multiple operations)
    conn: redis::aio::MultiplexedConnection,
}

impl RedisCache {
    /// Create a new Redis cache instance
    ///
    /// Uses multiplexed connection, reusing connections for better performance.
    ///
    /// # Errors
    ///
    /// Returns an error if Redis connection fails
    pub async fn new(url: &str) -> Result<Self, Error> {
        let client = redis::Client::open(url)
            .map_err(|e| Error::Cache(format!("Redis connection failed: {e}")))?;

        // Create multiplexed connection (can be shared across multiple operations)
        let conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| Error::Cache(format!("Redis connection creation failed: {e}")))?;

        // Simple ping test
        let mut ping_conn = conn.clone();
        let _: String = redis::cmd("PING")
            .query_async(&mut ping_conn)
            .await
            .map_err(|e| Error::Cache(format!("Redis ping failed: {e}")))?;

        Ok(Self { conn })
    }
}

#[async_trait::async_trait]
impl super::Cache for RedisCache {
    async fn get(&self, key: &str) -> Option<String> {
        let mut conn = self.conn.clone();
        redis::cmd("GET").arg(key).query_async(&mut conn).await.ok()
    }

    async fn set(&self, key: String, value: String, ttl: Option<Duration>) {
        let mut conn = self.conn.clone();

        let result: redis::RedisResult<()> = if let Some(ttl) = ttl {
            let secs = ttl.as_secs();
            redis::cmd("SETEX")
                .arg(key)
                .arg(secs)
                .arg(value)
                .query_async(&mut conn)
                .await
        } else {
            redis::cmd("SET")
                .arg(key)
                .arg(value)
                .query_async(&mut conn)
                .await
        };

        // Ignore errors, in production may need to log
        let _ = result;
    }

    async fn delete(&self, key: &str) {
        let mut conn = self.conn.clone();
        let _: () = redis::cmd("DEL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .unwrap_or(());
    }

    async fn clear(&self) {
        let mut conn = self.conn.clone();
        let _: () = redis::cmd("FLUSHDB")
            .query_async(&mut conn)
            .await
            .unwrap_or(());
    }

    async fn exists(&self, key: &str) -> bool {
        let mut conn = self.conn.clone();
        redis::cmd("EXISTS")
            .arg(key)
            .query_async(&mut conn)
            .await
            .unwrap_or(0)
            > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Cache;

    #[tokio::test]
    #[ignore = "需要 Redis 服务器"]
    async fn test_redis_cache() {
        // 这个测试需要运行中的 Redis 服务器
        // 在 CI 环境中应该被忽略

        let cache = RedisCache::new("redis://localhost:6379").await;
        assert!(cache.is_ok());

        let cache = cache.unwrap();

        // 测试设置和获取
        cache
            .set("test_key".to_string(), "test_value".to_string(), None)
            .await;
        let value = cache.get("test_key").await;
        assert_eq!(value, Some("test_value".to_string()));

        // 测试删除
        cache.delete("test_key").await;
        let value = cache.get("test_key").await;
        assert_eq!(value, None);

        // 测试存在性检查
        cache
            .set("exists_key".to_string(), "exists_value".to_string(), None)
            .await;
        assert!(cache.exists("exists_key").await);
        assert!(!cache.exists("non_exists_key").await);

        // 清理
        cache.clear().await;
    }
}
