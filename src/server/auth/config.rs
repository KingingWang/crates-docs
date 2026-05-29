//! Authentication configuration

use crate::error::{Error, Result};
use url::Url;

#[cfg(feature = "api-key")]
use api_keys_simplified::{
    ApiKey, ApiKeyManagerV0, Environment, ExposeSecret, HashConfig, KeyConfig, KeyStatus,
    SecureString,
};

use super::types::{GeneratedApiKey, OAuthProvider};

/// OAuth configuration
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OAuthConfig {
    /// Whether OAuth is enabled
    #[serde(default)]
    pub enabled: bool,
    /// Client ID
    #[serde(default)]
    pub client_id: Option<String>,
    /// Client secret
    #[serde(default)]
    pub client_secret: Option<String>,
    /// Redirect URI
    #[serde(default)]
    pub redirect_uri: Option<String>,
    /// Authorization endpoint
    #[serde(default)]
    pub authorization_endpoint: Option<String>,
    /// Token endpoint
    #[serde(default)]
    pub token_endpoint: Option<String>,
    /// Scopes
    #[serde(default = "default_oauth_scopes")]
    pub scopes: Vec<String>,
    /// Authentication provider type
    #[serde(default)]
    pub provider: OAuthProvider,
}

fn default_oauth_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "profile".to_string(),
        "email".to_string(),
    ]
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
            return Err(Error::config("client_id", "is required"));
        }

        if self.client_secret.is_none() {
            return Err(Error::config("client_secret", "is required"));
        }

        if self.redirect_uri.is_none() {
            return Err(Error::config("redirect_uri", "is required"));
        }

        if self.authorization_endpoint.is_none() {
            return Err(Error::config("authorization_endpoint", "is required"));
        }

        if self.token_endpoint.is_none() {
            return Err(Error::config("token_endpoint", "is required"));
        }

        // Validate URLs
        if let Some(uri) = &self.redirect_uri {
            Url::parse(uri)
                .map_err(|e| Error::config("redirect_uri", format!("Invalid URL: {e}")))?;
        }

        if let Some(endpoint) = &self.authorization_endpoint {
            Url::parse(endpoint).map_err(|e| {
                Error::config("authorization_endpoint", format!("Invalid URL: {e}"))
            })?;
        }

        if let Some(endpoint) = &self.token_endpoint {
            Url::parse(endpoint)
                .map_err(|e| Error::config("token_endpoint", format!("Invalid URL: {e}")))?;
        }

        Ok(())
    }

    /// Convert to rust-mcp-sdk `OAuthConfig`
    #[cfg(feature = "auth")]
    pub fn to_mcp_config(&self) -> Result<()> {
        if !self.enabled {
            return Err(Error::config("oauth", "is not enabled"));
        }

        // Temporarily return empty result, to be implemented when OAuth feature is complete
        Ok(())
    }

    /// Convert to rust-mcp-sdk `OAuthConfig`
    #[cfg(not(feature = "auth"))]
    pub fn to_mcp_config(&self) -> Result<()> {
        Err(Error::config("oauth", "feature is not enabled"))
    }
}

/// API Key configuration
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[cfg(feature = "api-key")]
pub struct ApiKeyConfig {
    /// Whether API key authentication is enabled
    #[serde(default)]
    pub enabled: bool,
    /// List of valid API key hashes in PHC format.
    ///
    /// For backward compatibility, plain-text keys are also accepted and will be
    /// verified with a constant-time string comparison fallback.
    /// New deployments should store only hashed keys generated by
    /// `ApiKeyConfig::generate_key()`.
    #[serde(default)]
    pub keys: Vec<String>,
    /// Header name for API key (default: "X-API-Key")
    #[serde(default = "default_header_name")]
    pub header_name: String,
    /// Query parameter name for API key (default: `api_key`)
    #[serde(default = "default_query_param_name")]
    pub query_param_name: String,
    /// Whether to allow API key in query parameters (less secure)
    #[serde(default)]
    pub allow_query_param: bool,
    /// API key prefix used by generated keys (e.g., "sk")
    #[serde(default = "default_key_prefix")]
    pub key_prefix: String,
}

#[cfg(feature = "api-key")]
fn default_header_name() -> String {
    "X-API-Key".to_string()
}

#[cfg(feature = "api-key")]
fn default_query_param_name() -> String {
    "api_key".to_string()
}

#[cfg(feature = "api-key")]
fn default_key_prefix() -> String {
    "sk".to_string()
}

#[cfg(feature = "api-key")]
impl Default for ApiKeyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            keys: Vec::new(),
            header_name: default_header_name(),
            query_param_name: default_query_param_name(),
            allow_query_param: false,
            key_prefix: default_key_prefix(),
        }
    }
}

#[cfg(feature = "api-key")]
impl ApiKeyConfig {
    fn manager(&self) -> Result<ApiKeyManagerV0> {
        ApiKeyManagerV0::init_default_config(self.key_prefix.clone())
            .map_err(|e| Error::initialization("api_key_manager", e.to_string()))
    }

    fn legacy_manager(&self) -> Result<ApiKeyManagerV0> {
        ApiKeyManagerV0::init(
            self.key_prefix.clone(),
            KeyConfig::default().disable_checksum(),
            HashConfig::default(),
            std::time::Duration::from_secs(10),
        )
        .map_err(|e| Error::initialization("api_key_manager", e.to_string()))
    }

    fn looks_like_hash(value: &str) -> bool {
        value.starts_with("$argon2")
    }

    fn looks_like_legacy_hash(value: &str) -> bool {
        value.starts_with("legacy:$argon2")
    }

    fn verify_plaintext_fallback(key: &str, stored_key: &str) -> bool {
        use api_keys_simplified::SecureStringExt;

        let provided = SecureString::from(key.to_string());
        let expected = SecureString::from(stored_key.to_string());

        provided.eq(&expected)
    }

    fn hash_legacy_key(&self, key: &str) -> Result<String> {
        let manager = self.legacy_manager()?;
        let seed = self.generate_key()?;
        let secure = SecureString::from(key.to_string());
        let hasher = manager.hasher();
        let api_key = ApiKey::new(secure)
            .into_hashed_with_phc(hasher, &seed.hash)
            .map_err(|e| Error::initialization("api_key_hashing", e.to_string()))?;
        Ok(format!("legacy:{}", api_key.expose_hash().hash()))
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if self.keys.is_empty() {
            tracing::warn!("API key authentication is enabled but no keys are configured");
        }

        if self.header_name.is_empty() {
            return Err(Error::config("header_name", "cannot be empty"));
        }

        if self.allow_query_param && self.query_param_name.is_empty() {
            return Err(Error::config(
                "query_param_name",
                "cannot be empty when allow_query_param is true",
            ));
        }

        if self.key_prefix.is_empty() {
            return Err(Error::config("key_prefix", "cannot be empty"));
        }

        let _ = self.manager()?;

        Ok(())
    }

    /// Check if a key is valid
    #[must_use]
    pub fn is_valid_key(&self, key: &str) -> bool {
        if !self.enabled {
            return true;
        }

        let manager = self.manager().ok();
        let legacy_manager = self.legacy_manager().ok();
        let provided_key = SecureString::from(key.to_string());

        self.keys.iter().any(|stored| {
            if Self::looks_like_legacy_hash(stored) {
                if let Some(legacy_manager) = &legacy_manager {
                    let stored_hash = stored.trim_start_matches("legacy:");
                    matches!(
                        legacy_manager.verify(&provided_key, stored_hash),
                        Ok(KeyStatus::Valid)
                    )
                } else {
                    false
                }
            } else if Self::looks_like_hash(stored) {
                if let Some(manager) = &manager {
                    matches!(manager.verify(&provided_key, stored), Ok(KeyStatus::Valid))
                } else {
                    false
                }
            } else {
                Self::verify_plaintext_fallback(key, stored)
            }
        })
    }

    /// Generate a new API key and corresponding hash using api-keys-simplified.
    ///
    /// The returned plain-text key should be shown once and then discarded.
    /// Persist only the returned hash.
    ///
    /// # Errors
    ///
    /// Returns an error if key generation fails
    pub fn generate_key(&self) -> Result<GeneratedApiKey> {
        let manager = self.manager()?;

        let key = manager
            .generate(Environment::production())
            .map_err(|e| Error::initialization("api_key_generation", e.to_string()))?;

        Ok(GeneratedApiKey {
            key: key.key().expose_secret().to_string(),
            key_id: key.expose_hash().key_id().to_owned(),
            hash: key.expose_hash().hash().to_owned(),
        })
    }

    /// Normalize API key material for storage.
    ///
    /// - Structured Argon2 hashes are kept as-is
    /// - Legacy plain-text keys are converted to `legacy:`-prefixed Argon2 hashes
    /// - Plain-text fallback remains supported for backward compatibility
    ///
    /// # Errors
    ///
    /// Returns an error if hashing legacy key material fails.
    pub fn normalize_key_material(&self, key: &str) -> Result<String> {
        if Self::looks_like_hash(key) || Self::looks_like_legacy_hash(key) {
            Ok(key.to_string())
        } else {
            self.hash_legacy_key(key)
        }
    }
}

/// Authentication configuration (unified for OAuth and API Key)
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
pub struct AuthConfig {
    /// OAuth configuration
    #[serde(default)]
    pub oauth: OAuthConfig,
    /// API key configuration
    #[cfg(feature = "api-key")]
    #[serde(default)]
    pub api_key: ApiKeyConfig,
}

impl AuthConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        self.oauth.validate()?;
        #[cfg(feature = "api-key")]
        self.api_key.validate()?;
        Ok(())
    }

    /// Check if any authentication is enabled
    #[must_use]
    #[cfg(feature = "api-key")]
    pub fn is_enabled(&self) -> bool {
        self.oauth.enabled || self.api_key.enabled
    }

    /// Check if any authentication is enabled
    #[must_use]
    #[cfg(not(feature = "api-key"))]
    pub fn is_enabled(&self) -> bool {
        self.oauth.enabled
    }
}
