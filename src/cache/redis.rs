//! Redis 缓存实现
//!
//! 提供 Redis 后端的缓存支持。

use std::time::Duration;

use crate::error::Error;

/// Redis 缓存实现
///
/// 使用 ConnectionManager 复用连接，避免每次操作都创建新连接。
pub struct RedisCache {
    /// Redis 连接管理器（自动复用连接）
    connection_manager: redis::aio::ConnectionManager,
}

impl RedisCache {
    /// 创建新的 Redis 缓存实例
    ///
    /// 使用 ConnectionManager 自动管理连接池，复用连接以提高性能。
    ///
    /// # Errors
    ///
    /// 如果连接 Redis 失败，返回错误
    pub async fn new(url: &str) -> Result<Self, Error> {
        let client =
            redis::Client::open(url).map_err(|e| Error::Cache(format!("Redis 连接失败: {e}")))?;

        // 创建连接管理器（自动复用连接）
        let connection_manager = client
            .get_connection_manager()
            .await
            .map_err(|e| Error::Cache(format!("Redis 连接管理器创建失败: {e}")))?;

        // 简单的 ping 测试
        let mut conn = connection_manager.clone();
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Cache(format!("Redis ping 失败: {e}")))?;

        Ok(Self { connection_manager })
    }
}

#[async_trait::async_trait]
impl super::Cache for RedisCache {
    async fn get(&self, key: &str) -> Option<String> {
        let mut conn = self.connection_manager.clone();
        redis::cmd("GET").arg(key).query_async(&mut conn).await.ok()
    }

    async fn set(&self, key: String, value: String, ttl: Option<Duration>) {
        let mut conn = self.connection_manager.clone();

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

        // 忽略错误，在实际应用中可能需要记录日志
        let _ = result;
    }

    async fn delete(&self, key: &str) {
        let mut conn = self.connection_manager.clone();
        let _: () = redis::cmd("DEL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .unwrap_or(());
    }

    async fn clear(&self) {
        let mut conn = self.connection_manager.clone();
        let _: () = redis::cmd("FLUSHDB")
            .query_async(&mut conn)
            .await
            .unwrap_or(());
    }

    async fn exists(&self, key: &str) -> bool {
        let mut conn = self.connection_manager.clone();
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
