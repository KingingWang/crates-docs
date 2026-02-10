//! Redis 缓存实现
//!
//! 提供 Redis 后端的缓存支持。

use async_trait::async_trait;
use std::time::Duration;

use crate::error::Error;

/// Redis 缓存实现
pub struct RedisCache {
    /// Redis 客户端
    client: redis::Client,
}

impl RedisCache {
    /// 创建新的 Redis 缓存实例
    ///
    /// # Errors
    ///
    /// 如果连接 Redis 失败，返回错误
    pub async fn new(url: &str) -> Result<Self, Error> {
        let client =
            redis::Client::open(url).map_err(|e| Error::Cache(format!("Redis 连接失败: {e}")))?;

        // 测试连接
        let mut conn = client
            .get_async_connection()
            .await
            .map_err(|e| Error::Cache(format!("Redis 连接测试失败: {e}")))?;

        // 简单的 ping 测试
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Cache(format!("Redis ping 失败: {e}")))?;

        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl super::Cache for RedisCache {
    async fn get(&self, key: &str) -> Option<String> {
        let mut conn = self.client.get_async_connection().await.ok()?;

        redis::cmd("GET").arg(key).query_async(&mut conn).await.ok()
    }

    async fn set(&self, key: String, value: String, ttl: Option<Duration>) {
        let mut conn = match self.client.get_async_connection().await {
            Ok(conn) => conn,
            Err(_) => return,
        };

        let result = if let Some(ttl) = ttl {
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
        let mut conn = match self.client.get_async_connection().await {
            Ok(conn) => conn,
            Err(_) => return,
        };

        let _: () = redis::cmd("DEL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .unwrap_or(());
    }

    async fn clear(&self) {
        let mut conn = match self.client.get_async_connection().await {
            Ok(conn) => conn,
            Err(_) => return,
        };

        let _: () = redis::cmd("FLUSHDB")
            .query_async(&mut conn)
            .await
            .unwrap_or(());
    }

    async fn exists(&self, key: &str) -> bool {
        let mut conn = match self.client.get_async_connection().await {
            Ok(conn) => conn,
            Err(_) => return false,
        };

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
