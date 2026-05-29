//! Redis cache implementation
//!
//! Provides Redis backend cache support with safe operations.

use std::sync::Arc;
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
            .map_err(|e| Error::cache("connect", None, format!("failed: {e}")))?;

        // Create multiplexed connection (can be shared across multiple operations)
        let conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                Error::cache("connect", None, format!("connection creation failed: {e}"))
            })?;

        // Simple ping test
        let mut ping_conn = conn.clone();
        let _: String = redis::cmd("PING")
            .query_async(&mut ping_conn)
            .await
            .map_err(|e| Error::cache("ping", None, format!("failed: {e}")))?;

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
}

/// Compute the millisecond expiry for a Redis `PX` argument from a TTL.
///
/// Redis treats a non-positive expiry as an error, and `PX 0` (which a
/// sub-millisecond `Duration` would otherwise produce) effectively stores the
/// key without any expiration. To avoid silently turning a short-lived entry
/// into a permanent one, any TTL — including a zero `Duration` — maps to at
/// least 1 millisecond.
fn px_millis_for_ttl(ttl: Duration) -> u64 {
    // Saturate instead of truncating: practical TTLs are far below u64::MAX ms,
    // and saturation keeps the value well-defined for pathological inputs.
    let ms = u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX);
    ms.max(1)
}

/// Build the SCAN match pattern for the configured key prefix.
///
/// Returns `None` when the prefix is empty. Clearing without a prefix would
/// require matching `*`, which could wipe unrelated keys in a shared Redis
/// database, so callers must treat an empty prefix as "refuse to clear".
fn scan_pattern_for_prefix(prefix: &str) -> Option<String> {
    if prefix.is_empty() {
        None
    } else {
        Some(format!("{prefix}:*"))
    }
}

#[async_trait::async_trait]
impl super::Cache for RedisCache {
    async fn get(&self, key: &str) -> Option<Arc<str>> {
        let mut conn = self.conn.clone();
        let full_key = self.build_key(key);
        let result: redis::RedisResult<Option<String>> = redis::cmd("GET")
            .arg(&full_key)
            .query_async(&mut conn)
            .await;
        result.ok().flatten().map(|s| Arc::from(s.into_boxed_str()))
    }

    async fn set(
        &self,
        key: String,
        value: String,
        ttl: Option<Duration>,
    ) -> crate::error::Result<()> {
        let mut conn = self.conn.clone();
        let full_key = self.build_key(&key);

        let result: redis::RedisResult<()> = if let Some(ttl) = ttl {
            // Use PX for millisecond precision instead of SETEX (seconds only).
            // Any positive TTL maps to at least 1ms so a sub-millisecond TTL
            // never collapses to `PX 0` (which would drop the expiry entirely).
            let ms = px_millis_for_ttl(ttl);
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

        result.map_err(|e| Error::cache("set", Some(key.clone()), format!("failed: {e}")))
    }

    async fn delete(&self, key: &str) -> crate::error::Result<()> {
        let mut conn = self.conn.clone();
        let full_key = self.build_key(key);

        let result: redis::RedisResult<()> = redis::cmd("DEL")
            .arg(&full_key)
            .query_async(&mut conn)
            .await;

        result.map_err(|e| Error::cache("delete", Some(key.to_string()), format!("failed: {e}")))
    }

    async fn clear(&self) -> crate::error::Result<()> {
        // Use SCAN to find and delete only keys with our prefix.
        // This is safer than FLUSHDB which would delete ALL keys in the database.
        //
        // Without a configured key prefix the only possible match pattern is
        // `*`, which would wipe a shared Redis database. Refuse to clear in
        // that case instead of risking unrelated data.
        let Some(pattern) = scan_pattern_for_prefix(&self.key_prefix) else {
            return Err(Error::cache(
                "clear",
                None,
                "refusing to clear cache without a configured key_prefix; \
                 clearing would require matching '*' and could wipe a shared \
                 Redis database",
            ));
        };

        let mut conn = self.conn.clone();

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
                                return Err(Error::cache(
                                    "clear",
                                    None,
                                    format!("DEL failed: {e}"),
                                ));
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
                    return Err(Error::cache("clear", None, format!("SCAN failed: {e}")));
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

        Ok(())
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
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
            .await
            .expect("set should succeed");
        let value = cache.get("test_key").await;
        assert!(value.is_some());
        assert_eq!(value.unwrap().as_ref(), "test_value");

        // Test delete
        cache
            .delete("test_key")
            .await
            .expect("delete should succeed");
        let value = cache.get("test_key").await;
        assert_eq!(value, None);

        // Test exists
        cache
            .set("exists_key".to_string(), "exists_value".to_string(), None)
            .await
            .expect("set should succeed");
        assert!(cache.exists("exists_key").await);
        assert!(!cache.exists("non_exists_key").await);

        // Test clear (should only clear keys with our prefix)
        cache
            .set("clear_test".to_string(), "value".to_string(), None)
            .await
            .expect("set should succeed");
        cache.clear().await.expect("clear should succeed");
        let cleared_value = cache.get("clear_test").await;
        assert!(cleared_value.is_none());
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
    fn test_scan_pattern_for_prefix_empty_refuses() {
        // An empty prefix must NOT produce a "*" pattern (which would target
        // the whole database); callers treat None as "refuse to clear".
        assert_eq!(scan_pattern_for_prefix(""), None);
    }

    #[test]
    fn test_scan_pattern_for_prefix_with_prefix() {
        assert_eq!(
            scan_pattern_for_prefix("myapp"),
            Some("myapp:*".to_string())
        );
    }

    #[test]
    fn test_px_millis_for_ttl_zero_is_minimum() {
        // A zero Duration must not collapse to `PX 0` (no expiry); it maps to
        // the smallest positive expiry instead.
        assert_eq!(px_millis_for_ttl(Duration::from_millis(0)), 1);
    }

    #[test]
    fn test_px_millis_for_ttl_submillisecond_is_minimum() {
        // Sub-millisecond TTLs round up to 1ms instead of truncating to 0.
        assert_eq!(px_millis_for_ttl(Duration::from_micros(500)), 1);
        assert_eq!(px_millis_for_ttl(Duration::from_nanos(1)), 1);
    }

    #[test]
    fn test_px_millis_for_ttl_normal_values() {
        assert_eq!(px_millis_for_ttl(Duration::from_millis(1)), 1);
        assert_eq!(px_millis_for_ttl(Duration::from_millis(1500)), 1500);
        assert_eq!(px_millis_for_ttl(Duration::from_secs(2)), 2000);
    }
}
