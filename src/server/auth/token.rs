//! Token store and token information

use std::collections::HashMap;

use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use tokio::time::{timeout, Duration};

/// Default lock acquisition timeout (500ms)
const DEFAULT_LOCK_TIMEOUT_MS: u64 = 500;

/// Token store operations error types
#[derive(Debug, thiserror::Error)]
pub enum TokenStoreError {
    /// Lock acquisition timed out
    #[error("Token store lock timeout after {}ms", ms)]
    LockTimeout {
        /// Timeout duration in milliseconds
        ms: u64,
    },
}

/// Module-level result type for token store operations
pub type TokenStoreResult<T> = std::result::Result<T, TokenStoreError>;

/// Simple async in-memory token store (production should use Redis or database)
///
/// # Differences from previous implementation:
///
/// 1. Uses `tokio::sync::RwLock` instead of `std::sync::RwLock` for async operations
/// 2. Lock acquisition has timeout protection (default 500ms)
/// 3. Returns proper errors instead of panicking on lock failures
/// 4. All methods are now async
/// 5. Does not have poison mechanics (tokio `RwLock` is panic-safe)
#[derive(Default)]
pub struct TokenStore {
    tokens: RwLock<HashMap<String, TokenInfo>>,
}

/// OAuth token information
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct TokenInfo {
    /// Access token
    pub access_token: String,
    /// Refresh token (optional)
    pub refresh_token: Option<String>,
    /// Token expiration time
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// Authorization scopes
    pub scopes: Vec<String>,
    /// User ID (optional)
    pub user_id: Option<String>,
    /// User email (optional)
    pub user_email: Option<String>,
}

impl TokenStore {
    /// Create a new token store
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Store token in the store
    ///
    /// # Errors
    ///
    /// Returns `TokenStoreError::LockTimeout` if write lock cannot be acquired within timeout.
    pub async fn store_token(&self, key: String, token: TokenInfo) -> TokenStoreResult<()> {
        let mut guard = self.acquire_write_lock().await?;
        guard.insert(key, token);
        Ok(())
    }

    /// Get a token from the store
    ///
    /// # Errors
    ///
    /// Returns `TokenStoreError::LockTimeout` if read lock cannot be acquired within timeout.
    pub async fn get_token(&self, key: &str) -> TokenStoreResult<Option<TokenInfo>> {
        let guard = self.acquire_read_lock().await?;
        Ok(guard.get(key).cloned())
    }

    /// Remove a token from the store
    ///
    /// # Errors
    ///
    /// Returns `TokenStoreError::LockTimeout` if write lock cannot be acquired within timeout.
    pub async fn remove_token(&self, key: &str) -> TokenStoreResult<()> {
        let mut guard = self.acquire_write_lock().await?;
        guard.remove(key);
        Ok(())
    }

    /// Cleanup all expired tokens from the store
    ///
    /// # Errors
    ///
    /// Returns `TokenStoreError::LockTimeout` if write lock cannot be acquired within timeout.
    pub async fn cleanup_expired(&self) -> TokenStoreResult<()> {
        let now = chrono::Utc::now();
        let mut guard = self.acquire_write_lock().await?;
        guard.retain(|_, token| token.expires_at > now);
        Ok(())
    }

    /// Acquire a read lock with timeout protection
    ///
    /// Uses `tokio::time::timeout` to prevent indefinite blocking.
    /// This is critical for concurrent async workloads where lock contention might occur.
    ///
    /// # Errors
    ///
    /// - `TokenStoreError::LockTimeout` - Lock not acquired within `DEFAULT_LOCK_TIMEOUT_MS` (500ms)
    async fn acquire_read_lock(
        &self,
    ) -> TokenStoreResult<RwLockReadGuard<'_, HashMap<String, TokenInfo>>> {
        match timeout(
            Duration::from_millis(DEFAULT_LOCK_TIMEOUT_MS),
            self.tokens.read(),
        )
        .await
        {
            Ok(guard) => Ok(guard),
            Err(_elapsed) => Err(TokenStoreError::LockTimeout {
                ms: DEFAULT_LOCK_TIMEOUT_MS,
            }),
        }
    }

    /// Acquire a write lock with timeout protection
    ///
    /// Uses `tokio::time::timeout` to prevent indefinite blocking.
    /// This is critical for concurrent async workloads where lock contention might occur.
    ///
    /// # Errors
    ///
    /// - `TokenStoreError::LockTimeout` - Lock not acquired within `DEFAULT_LOCK_TIMEOUT_MS` (500ms)
    async fn acquire_write_lock(
        &self,
    ) -> TokenStoreResult<RwLockWriteGuard<'_, HashMap<String, TokenInfo>>> {
        match timeout(
            Duration::from_millis(DEFAULT_LOCK_TIMEOUT_MS),
            self.tokens.write(),
        )
        .await
        {
            Ok(guard) => Ok(guard),
            Err(_elapsed) => Err(TokenStoreError::LockTimeout {
                ms: DEFAULT_LOCK_TIMEOUT_MS,
            }),
        }
    }
}
