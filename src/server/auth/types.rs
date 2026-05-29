//! Authentication types

/// OAuth provider type
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Default)]
pub enum OAuthProvider {
    /// Custom OAuth provider
    #[default]
    Custom,
    /// GitHub OAuth
    GitHub,
    /// Google OAuth
    Google,
    /// Keycloak
    Keycloak,
}

/// Authentication provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthProvider {
    /// No authentication
    None,
    /// OAuth authentication
    OAuth,
    /// API Key authentication
    #[cfg(feature = "api-key")]
    ApiKey,
}

/// API Key generation result.
///
/// The plain-text key should be shown once to the operator and stored securely.
/// The hash should be persisted in configuration or external secret storage.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[cfg(feature = "api-key")]
pub struct GeneratedApiKey {
    /// Plain-text API key for one-time display
    pub key: String,
    /// Stable key identifier derived from the key
    pub key_id: String,
    /// Argon2 PHC hash to store and verify against
    pub hash: String,
}

/// Authentication context
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// Authentication provider used
    pub provider: AuthProvider,
    /// User ID (if available)
    pub user_id: Option<String>,
    /// User email (if available)
    pub user_email: Option<String>,
    /// API key identifier (if API key auth)
    #[cfg(feature = "api-key")]
    pub api_key_id: Option<String>,
}

impl AuthContext {
    /// Create a new authentication context
    #[must_use]
    pub fn new(provider: AuthProvider) -> Self {
        Self {
            provider,
            user_id: None,
            user_email: None,
            #[cfg(feature = "api-key")]
            api_key_id: None,
        }
    }

    /// Check if authentication is authenticated
    #[must_use]
    pub fn is_authenticated(&self) -> bool {
        !matches!(self.provider, AuthProvider::None)
    }
}
