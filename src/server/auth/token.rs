//! Token store and token information

/// Simple in-memory token store (production should use Redis or database)
#[derive(Default)]
pub struct TokenStore {
    tokens: std::sync::RwLock<std::collections::HashMap<String, TokenInfo>>,
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

    /// Store token
    pub fn store_token(&self, key: String, token: TokenInfo) {
        let mut tokens = self.tokens.write().unwrap();
        tokens.insert(key, token);
    }

    /// Get token
    pub fn get_token(&self, key: &str) -> Option<TokenInfo> {
        let tokens = self.tokens.read().unwrap();
        tokens.get(key).cloned()
    }

    /// Remove token
    pub fn remove_token(&self, key: &str) {
        let mut tokens = self.tokens.write().unwrap();
        tokens.remove(key);
    }

    /// Cleanup expired tokens
    pub fn cleanup_expired(&self) {
        let now = chrono::Utc::now();
        let mut tokens = self.tokens.write().unwrap();
        tokens.retain(|_, token| token.expires_at > now);
    }
}
