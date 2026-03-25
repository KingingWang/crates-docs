//! Authentication middleware for HTTP requests
//!
//! Provides API Key authentication middleware for HTTP/SSE transports.
//!
//! # Example
//!
//! ```rust,no_run
//! use crates_docs::server::auth_middleware::ApiKeyMiddleware;
//! use crates_docs::server::auth::ApiKeyConfig;
//!
//! let config = ApiKeyConfig::default();
//! let middleware = ApiKeyMiddleware::new(config);
//! ```

#[cfg(feature = "api-key")]
use crate::server::auth::ApiKeyConfig;

/// API Key authentication middleware
#[cfg(feature = "api-key")]
pub struct ApiKeyMiddleware {
    config: ApiKeyConfig,
}

#[cfg(feature = "api-key")]
impl ApiKeyMiddleware {
    /// Create a new API Key middleware
    #[must_use]
    pub fn new(config: ApiKeyConfig) -> Self {
        Self { config }
    }

    /// Validate API key from headers or query parameters
    ///
    /// # Arguments
    ///
    /// * `headers` - HTTP request headers
    /// * `query_params` - URL query parameters (optional)
    ///
    /// # Returns
    ///
    /// Returns `true` if authentication is disabled or key is valid
    #[must_use]
    pub fn validate_request(
        &self,
        headers: &std::collections::HashMap<String, String>,
        query_params: Option<&std::collections::HashMap<String, String>>,
    ) -> bool {
        if !self.config.enabled {
            return true;
        }

        // Try to get API key from header first
        if let Some(key) = headers.get(&self.config.header_name) {
            return self.config.is_valid_key(key);
        }

        // Fallback to query parameter if allowed
        if self.config.allow_query_param {
            if let Some(params) = query_params {
                if let Some(key) = params.get(&self.config.query_param_name) {
                    return self.config.is_valid_key(key);
                }
            }
        }

        false
    }

    /// Extract API key from request
    ///
    /// # Arguments
    ///
    /// * `headers` - HTTP request headers
    /// * `query_params` - URL query parameters (optional)
    ///
    /// # Returns
    ///
    /// Returns the API key if found
    #[must_use]
    pub fn extract_key(
        &self,
        headers: &std::collections::HashMap<String, String>,
        query_params: Option<&std::collections::HashMap<String, String>>,
    ) -> Option<String> {
        // Try header first
        if let Some(key) = headers.get(&self.config.header_name) {
            return Some(key.clone());
        }

        // Fallback to query parameter
        if self.config.allow_query_param {
            if let Some(params) = query_params {
                if let Some(key) = params.get(&self.config.query_param_name) {
                    return Some(key.clone());
                }
            }
        }

        None
    }

    /// Check if authentication is enabled
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

/// Authentication error response
#[derive(Debug, Clone)]
pub struct AuthError {
    /// Error message
    pub message: String,
    /// WWW-Authenticate header value
    pub www_authenticate: Option<String>,
}

impl AuthError {
    /// Create a new authentication error
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            www_authenticate: None,
        }
    }

    /// Create unauthorized error
    #[must_use]
    pub fn unauthorized() -> Self {
        Self {
            message: "Unauthorized: API key required".to_string(),
            www_authenticate: Some("ApiKey realm=\"crates-docs\"".to_string()),
        }
    }

    /// Create invalid key error
    #[must_use]
    pub fn invalid_key() -> Self {
        Self {
            message: "Unauthorized: Invalid API key".to_string(),
            www_authenticate: Some(
                "ApiKey realm=\"crates-docs\" error=\"invalid_key\"".to_string(),
            ),
        }
    }
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AuthError {}

/// No-op middleware when API Key feature is disabled
#[cfg(not(feature = "api-key"))]
pub struct NoOpMiddleware;

#[cfg(not(feature = "api-key"))]
impl NoOpMiddleware {
    /// Always returns true (no authentication)
    #[must_use]
    pub fn validate_request(
        &self,
        _headers: &std::collections::HashMap<String, String>,
        _query_params: Option<&std::collections::HashMap<String, String>>,
    ) -> bool {
        true
    }

    /// Always returns true (no authentication)
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[cfg(feature = "api-key")]
mod tests {
    use super::*;

    fn create_test_config() -> (ApiKeyConfig, String) {
        let generator = ApiKeyConfig::default();
        let generated = generator
            .generate_key()
            .expect("failed to generate API key");

        (
            ApiKeyConfig {
                enabled: true,
                keys: vec![generated.hash],
                ..Default::default()
            },
            generated.key,
        )
    }

    #[test]
    fn test_middleware_disabled() {
        let config = ApiKeyConfig::default();
        let middleware = ApiKeyMiddleware::new(config);

        let headers = std::collections::HashMap::new();
        assert!(middleware.validate_request(&headers, None));
    }

    #[test]
    fn test_middleware_valid_key_header() {
        let (config, api_key) = create_test_config();
        let middleware = ApiKeyMiddleware::new(config);

        let mut headers = std::collections::HashMap::new();
        headers.insert("X-API-Key".to_string(), api_key);

        assert!(middleware.validate_request(&headers, None));
    }

    #[test]
    fn test_middleware_invalid_key_header() {
        let (config, _) = create_test_config();
        let middleware = ApiKeyMiddleware::new(config);

        let mut headers = std::collections::HashMap::new();
        headers.insert("X-API-Key".to_string(), "invalid_key".to_string());

        assert!(!middleware.validate_request(&headers, None));
    }

    #[test]
    fn test_middleware_missing_key() {
        let (config, _) = create_test_config();
        let middleware = ApiKeyMiddleware::new(config);

        let headers = std::collections::HashMap::new();
        assert!(!middleware.validate_request(&headers, None));
    }

    #[test]
    fn test_middleware_query_param_allowed() {
        let (mut config, api_key) = create_test_config();
        config.allow_query_param = true;
        let middleware = ApiKeyMiddleware::new(config);

        let headers = std::collections::HashMap::new();
        let mut query_params = std::collections::HashMap::new();
        query_params.insert("api_key".to_string(), api_key);

        assert!(middleware.validate_request(&headers, Some(&query_params)));
    }

    #[test]
    fn test_middleware_query_param_not_allowed() {
        let (config, api_key) = create_test_config();
        let middleware = ApiKeyMiddleware::new(config);

        let headers = std::collections::HashMap::new();
        let mut query_params = std::collections::HashMap::new();
        query_params.insert("api_key".to_string(), api_key);

        assert!(!middleware.validate_request(&headers, Some(&query_params)));
    }

    #[test]
    fn test_extract_key() {
        let (config, api_key) = create_test_config();
        let middleware = ApiKeyMiddleware::new(config);

        let mut headers = std::collections::HashMap::new();
        headers.insert("X-API-Key".to_string(), api_key.clone());

        let key = middleware.extract_key(&headers, None);
        assert_eq!(key, Some(api_key));
    }

    #[test]
    fn test_auth_error() {
        let error = AuthError::unauthorized();
        assert_eq!(error.message, "Unauthorized: API key required");
        assert!(error.www_authenticate.is_some());
    }
}
