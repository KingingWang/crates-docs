//! Authentication manager

use crate::error::Result;

use super::config::{ApiKeyConfig, AuthConfig, OAuthConfig};
use super::types::GeneratedApiKey;

/// Authentication manager
#[derive(Default)]
pub struct AuthManager {
    config: OAuthConfig,
    #[cfg(feature = "api-key")]
    api_key_config: ApiKeyConfig,
}

impl AuthManager {
    /// Create a new authentication manager
    pub fn new(config: OAuthConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            #[cfg(feature = "api-key")]
            api_key_config: ApiKeyConfig::default(),
        })
    }

    /// Create a new authentication manager with full config
    #[cfg(feature = "api-key")]
    pub fn with_config(config: AuthConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config: config.oauth,
            api_key_config: config.api_key,
        })
    }

    /// Check if authentication is enabled
    #[must_use]
    #[cfg(feature = "api-key")]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled || self.api_key_config.enabled
    }

    /// Check if authentication is enabled
    #[must_use]
    #[cfg(not(feature = "api-key"))]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get OAuth configuration
    #[must_use]
    pub fn config(&self) -> &OAuthConfig {
        &self.config
    }

    /// Get API key configuration
    #[cfg(feature = "api-key")]
    #[must_use]
    pub fn api_key_config(&self) -> &ApiKeyConfig {
        &self.api_key_config
    }

    /// Validate API key
    #[cfg(feature = "api-key")]
    #[must_use]
    pub fn validate_api_key(&self, key: &str) -> bool {
        self.api_key_config.is_valid_key(key)
    }

    /// Generate a new API key and hash pair
    ///
    /// # Errors
    ///
    /// Returns an error if key generation fails
    #[cfg(feature = "api-key")]
    pub fn generate_api_key(&self) -> Result<GeneratedApiKey> {
        self.api_key_config.generate_key()
    }

    /// Extract API key from request headers
    #[cfg(feature = "api-key")]
    #[must_use]
    pub fn extract_api_key_from_headers(
        &self,
        headers: &std::collections::HashMap<String, String>,
    ) -> Option<String> {
        headers.get(&self.api_key_config.header_name).cloned()
    }
}
