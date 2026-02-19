//! OAuth authentication module
//!
//! Provides OAuth 2.0 authentication support.

use crate::error::{Error, Result};
use url::Url;

/// OAuth configuration
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OAuthConfig {
    /// Whether OAuth is enabled
    pub enabled: bool,
    /// Client ID
    pub client_id: Option<String>,
    /// Client secret
    pub client_secret: Option<String>,
    /// Redirect URI
    pub redirect_uri: Option<String>,
    /// Authorization endpoint
    pub authorization_endpoint: Option<String>,
    /// Token endpoint
    pub token_endpoint: Option<String>,
    /// Scopes
    pub scopes: Vec<String>,
    /// Authentication provider type
    pub provider: OAuthProvider,
}

/// OAuth provider type
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum OAuthProvider {
    /// Custom OAuth provider
    Custom,
    /// GitHub OAuth
    GitHub,
    /// Google OAuth
    Google,
    /// Keycloak
    Keycloak,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            client_id: None,
            client_secret: None,
            redirect_uri: None,
            authorization_endpoint: None,
            token_endpoint: None,
            scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            provider: OAuthProvider::Custom,
        }
    }
}

impl OAuthConfig {
    /// Create GitHub OAuth configuration
    #[must_use]
    pub fn github(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            enabled: true,
            client_id: Some(client_id),
            client_secret: Some(client_secret),
            redirect_uri: Some(redirect_uri),
            authorization_endpoint: Some("https://github.com/login/oauth/authorize".to_string()),
            token_endpoint: Some("https://github.com/login/oauth/access_token".to_string()),
            scopes: vec!["read:user".to_string(), "user:email".to_string()],
            provider: OAuthProvider::GitHub,
        }
    }

    /// Create Google OAuth configuration
    #[must_use]
    pub fn google(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            enabled: true,
            client_id: Some(client_id),
            client_secret: Some(client_secret),
            redirect_uri: Some(redirect_uri),
            authorization_endpoint: Some(
                "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            ),
            token_endpoint: Some("https://oauth2.googleapis.com/token".to_string()),
            scopes: vec![
                "openid".to_string(),
                "https://www.googleapis.com/auth/userinfo.profile".to_string(),
                "https://www.googleapis.com/auth/userinfo.email".to_string(),
            ],
            provider: OAuthProvider::Google,
        }
    }

    /// Create Keycloak OAuth configuration
    #[must_use]
    pub fn keycloak(
        client_id: String,
        client_secret: String,
        redirect_uri: String,
        base_url: &str,
        realm: &str,
    ) -> Self {
        let base = base_url.trim_end_matches('/');
        Self {
            enabled: true,
            client_id: Some(client_id),
            client_secret: Some(client_secret),
            redirect_uri: Some(redirect_uri),
            authorization_endpoint: Some(format!(
                "{base}/realms/{realm}/protocol/openid-connect/auth"
            )),
            token_endpoint: Some(format!(
                "{base}/realms/{realm}/protocol/openid-connect/token"
            )),
            scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            provider: OAuthProvider::Keycloak,
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if self.client_id.is_none() {
            return Err(Error::Config("client_id is required".to_string()));
        }

        if self.client_secret.is_none() {
            return Err(Error::Config("client_secret is required".to_string()));
        }

        if self.redirect_uri.is_none() {
            return Err(Error::Config("redirect_uri is required".to_string()));
        }

        if self.authorization_endpoint.is_none() {
            return Err(Error::Config("authorization_endpoint is required".to_string()));
        }

        if self.token_endpoint.is_none() {
            return Err(Error::Config("token_endpoint is required".to_string()));
        }

        // Validate URLs
        if let Some(uri) = &self.redirect_uri {
            Url::parse(uri).map_err(|e| Error::Config(format!("Invalid redirect_uri: {e}")))?;
        }

        if let Some(endpoint) = &self.authorization_endpoint {
            Url::parse(endpoint)
                .map_err(|e| Error::Config(format!("Invalid authorization_endpoint: {e}")))?;
        }

        if let Some(endpoint) = &self.token_endpoint {
            Url::parse(endpoint)
                .map_err(|e| Error::Config(format!("Invalid token_endpoint: {e}")))?;
        }

        Ok(())
    }

    /// Convert to rust-mcp-sdk `OAuthConfig`
    #[cfg(feature = "auth")]
    pub fn to_mcp_config(&self) -> Result<()> {
        if !self.enabled {
            return Err(Error::Config("OAuth is not enabled".to_string()));
        }

        // Temporarily return empty result, to be implemented when OAuth feature is complete
        Ok(())
    }

    /// Convert to rust-mcp-sdk `OAuthConfig`
    #[cfg(not(feature = "auth"))]
    pub fn to_mcp_config(&self) -> Result<()> {
        Err(Error::Config("OAuth feature is not enabled".to_string()))
    }
}

/// Authentication manager
#[derive(Default)]
pub struct AuthManager {
    config: OAuthConfig,
}

impl AuthManager {
    /// Create a new authentication manager
    pub fn new(config: OAuthConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Check if authentication is enabled
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get configuration
    #[must_use]
    pub fn config(&self) -> &OAuthConfig {
        &self.config
    }
}

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