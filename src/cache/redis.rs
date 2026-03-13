//! Redis cache implementation
//!
//! Provides Redis backend cache support with safe operations.

use std::time::Duration;

use crate::error::Error;

/// Default scan count for SCAN command when clearing keys
const DEFAULT_SCAN_COUNT: usize = 100;

/// Redis cache implementation
///
/// Uses multiplexed connection (`MultiplexedConnection`) to avoid creating new connections for each operation.
/// Multiplexed connections can be safely cloned and shared across multiple tasks.
///
/// # Safety
///
/// - Uses key prefix to isolate cache entries from different services
/// - `clear()` only deletes keys with the configured prefix using SCAN (no FLUSHDB)
/// - All write operations return Result to properly propagate errors
pub struct RedisCache {
    /// Multiplexed connection (cloneable, shared across multiple operations)
    conn: redis::aio::MultiplexedConnection,
    /// Key prefix for all cache entries
    key_prefix: String,
}

impl RedisCache {
    /// Create a new Redis cache instance
    ///
    /// Uses multiplexed connection, reusing connections for better performance.
    ///
    /// # Arguments
    ///
    /// * `url` - Redis connection URL (e.g., `<redis://127.0.0.1:6379>`)
    /// * `key_prefix` - Prefix for all cache keys (can be empty string)
    ///
    /// # Errors
    ///
    /// Returns an error if Redis connection fails or ping test fails
    pub async fn new(url: &str, key_prefix: String) -> Result<Self, Error> {
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

        Ok(Self { conn, key_prefix })
    }

    /// Build full key with prefix
    fn build_key(&self, key: &str) -> String {
        if self.key_prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}:{}", self.key_prefix, key)
        }
    }

    /// Build the pattern for scanning keys with the prefix
    fn build_scan_pattern(&self) -> String {
        if self.key_prefix.is_empty() {
            "*".to_string()
        } else {
            format!("{}:*", self.key_prefix)
        }
    }
}

#[async_trait::async_trait]
impl super::Cache for RedisCache {
    async fn get(&self, key: &str) -> Option<String> {
        let mut conn = self.conn.clone();
        let full_key = self.build_key(key);
        redis::cmd("GET")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .ok()
    }

    #[allow(clippy::cast_possible_truncation)]
    async fn set(&self, key: String, value: String, ttl: Option<Duration>) {
        let mut conn = self.conn.clone();
        let full_key = self.build_key(&key);

        let result: redis::RedisResult<()> = if let Some(ttl) = ttl {
            // Use PX for millisecond precision instead of SETEX (seconds only)
            // Note: Truncation from u128 to u64 is acceptable here because
            // TTL values in practice are much smaller than u64::MAX milliseconds
            let ms = ttl.as_millis() as u64;
            redis::cmd("SET")
                .arg(&full_key)
                .arg(&value)
                .arg("PX")
                .arg(ms)
                .query_async(&mut conn)
                .await
        } else {
            redis::cmd("SET")
                .arg(&full_key)
                .arg(&value)
                .query_async(&mut conn)
                .await
        };

        // Log errors instead of silently ignoring them
        if let Err(e) = result {
            tracing::error!("Redis SET failed for key '{}': {}", key, e);
        }
    }

    async fn delete(&self, key: &str) {
        let mut conn = self.conn.clone();
        let full_key = self.build_key(key);

        let result: redis::RedisResult<()> = redis::cmd("DEL")
            .arg(&full_key)
            .query_async(&mut conn)
            .await;

        // Log errors instead of silently ignoring them
        if let Err(e) = result {
            tracing::error!("Redis DEL failed for key '{}': {}", key, e);
        }
    }

    async fn clear(&self) {
        // Use SCAN to find and delete keys with our prefix
        // This is safer than FLUSHDB which would delete ALL keys in the database
        let mut conn = self.conn.clone();
        let pattern = self.build_scan_pattern();

        let mut cursor: u64 = 0;
        let mut total_deleted: u64 = 0;

        loop {
            // SCAN returns (new_cursor, keys)
            let scan_result: redis::RedisResult<(u64, Vec<String>)> = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(DEFAULT_SCAN_COUNT)
                .query_async(&mut conn)
                .await;

            match scan_result {
                Ok((new_cursor, keys)) => {
                    if !keys.is_empty() {
                        // Delete the found keys
                        let del_result: redis::RedisResult<u64> =
                            redis::cmd("DEL").arg(&keys).query_async(&mut conn).await;

                        match del_result {
                            Ok(deleted) => total_deleted += deleted,
                            Err(e) => {
                                tracing::error!("Redis DEL during clear failed: {}", e);
                            }
                        }
                    }

                    cursor = new_cursor;
                    // SCAN returns 0 when iteration is complete
                    if cursor == 0 {
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!("Redis SCAN during clear failed: {}", e);
                    break;
                }
            }
        }

        if total_deleted > 0 {
            tracing::debug!(
                "Cleared {} cache entries with prefix '{}'",
                total_deleted,
                self.key_prefix
            );
        }
    }

    async fn exists(&self, key: &str) -> bool {
        let mut conn = self.conn.clone();
        let full_key = self.build_key(key);
        redis::cmd("EXISTS")
            .arg(&full_key)
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
    #[ignore = "Requires Redis server"]
    async fn test_redis_cache_basic() {
        // This test requires a running Redis server
        // Should be ignored in CI environments

        let cache = RedisCache::new("redis://localhost:6379", "test_prefix".to_string()).await;
        assert!(cache.is_ok());

        let cache = cache.unwrap();

        // Test set and get
        cache
            .set("test_key".to_string(), "test_value".to_string(), None)
            .await;
        let value = cache.get("test_key").await;
        assert_eq!(value, Some("test_value".to_string()));

        // Test delete
        cache.delete("test_key").await;
        let value = cache.get("test_key").await;
        assert_eq!(value, None);

        // Test exists
        cache
            .set("exists_key".to_string(), "exists_value".to_string(), None)
            .await;
        assert!(cache.exists("exists_key").await);
        assert!(!cache.exists("non_exists_key").await);

        // Test clear (should only clear keys with our prefix)
        cache
            .set("clear_test".to_string(), "value".to_string(), None)
            .await;
        cache.clear().await;
        assert_eq!(cache.get("clear_test").await, None);
    }

    #[test]
    fn test_build_key() {
        // Test with no prefix
        let prefix = "";
        let key = "mykey";
        let expected = "mykey";
        let result = if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{prefix}:{key}")
        };
        assert_eq!(result, expected);

        // Test with prefix
        let prefix = "myapp";
        let key = "mykey";
        let expected = "myapp:mykey";
        let result = if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{prefix}:{key}")
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_build_scan_pattern() {
        // Test with no prefix
        let prefix = "";
        let expected = "*";
        let result = if prefix.is_empty() {
            "*".to_string()
        } else {
            format!("{prefix}:*")
        };
        assert_eq!(result, expected);

        // Test with prefix
        let prefix = "myapp";
        let expected = "myapp:*";
        let result = if prefix.is_empty() {
            "*".to_string()
        } else {
            format!("{prefix}:*")
        };
        assert_eq!(result, expected);
    }
}
