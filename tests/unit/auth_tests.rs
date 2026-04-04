//! OAuth and authentication module unit tests

use crates_docs::server::auth::{AuthManager, OAuthConfig, OAuthProvider, TokenInfo, TokenStore};

// ============================================================================
// OAuth configuration tests
// ============================================================================

#[test]
fn test_oauth_config_github() {
    let config = OAuthConfig::github(
        "client_id".to_string(),
        "secret".to_string(),
        "http://localhost/callback".to_string(),
    );
    assert!(config.enabled);
    assert_eq!(config.client_id, Some("client_id".to_string()));
    assert!(matches!(config.provider, OAuthProvider::GitHub));
}

#[test]
fn test_oauth_config_google() {
    let config = OAuthConfig::google(
        "client_id".to_string(),
        "secret".to_string(),
        "http://localhost/callback".to_string(),
    );
    assert!(config.enabled);
    assert_eq!(config.client_id, Some("client_id".to_string()));
    assert!(matches!(config.provider, OAuthProvider::Google));
}

#[test]
fn test_oauth_config_keycloak() {
    let config = OAuthConfig::keycloak(
        "client_id".to_string(),
        "secret".to_string(),
        "http://localhost/callback".to_string(),
        "https://keycloak.example.com",
        "test",
    );
    assert!(config.enabled);
    assert_eq!(config.client_id, Some("client_id".to_string()));
    assert!(matches!(config.provider, OAuthProvider::Keycloak));
}

#[test]
fn test_oauth_config_keycloak_trailing_slash() {
    let config1 = OAuthConfig::keycloak(
        "client".to_string(),
        "secret".to_string(),
        "http://localhost/callback".to_string(),
        "https://keycloak.example.com/",
        "realm",
    );
    let config2 = OAuthConfig::keycloak(
        "client".to_string(),
        "secret".to_string(),
        "http://localhost/callback".to_string(),
        "https://keycloak.example.com",
        "realm",
    );
    assert_eq!(
        config1.authorization_endpoint,
        config2.authorization_endpoint
    );
}

#[test]
fn test_oauth_config_validation_missing_client_id() {
    let config = OAuthConfig {
        enabled: true,
        client_id: None,
        client_secret: Some("secret".to_string()),
        redirect_uri: Some("http://localhost/callback".to_string()),
        authorization_endpoint: Some("https://example.com/auth".to_string()),
        token_endpoint: Some("https://example.com/token".to_string()),
        scopes: vec!["read".to_string()],
        provider: OAuthProvider::Custom,
    };
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_oauth_config_validation_missing_client_secret() {
    let config = OAuthConfig {
        enabled: true,
        client_id: Some("client_id".to_string()),
        client_secret: None,
        redirect_uri: Some("http://localhost/callback".to_string()),
        authorization_endpoint: Some("https://example.com/auth".to_string()),
        token_endpoint: Some("https://example.com/token".to_string()),
        scopes: vec!["read".to_string()],
        provider: OAuthProvider::Custom,
    };
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_oauth_config_validation_disabled() {
    let config = OAuthConfig {
        enabled: false,
        client_id: None,
        client_secret: None,
        redirect_uri: None,
        authorization_endpoint: None,
        token_endpoint: None,
        scopes: vec![],
        provider: OAuthProvider::Custom,
    };
    let result = config.validate();
    assert!(result.is_ok());
}

#[test]
fn test_oauth_config_validate_missing_redirect_uri() {
    let config = OAuthConfig {
        enabled: true,
        client_id: Some("client_id".to_string()),
        client_secret: Some("secret".to_string()),
        redirect_uri: None,
        authorization_endpoint: Some("https://example.com/auth".to_string()),
        token_endpoint: Some("https://example.com/token".to_string()),
        scopes: vec![],
        provider: OAuthProvider::Custom,
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("redirect_uri"));
}

#[test]
fn test_oauth_config_validate_invalid_urls() {
    let mut config = OAuthConfig {
        enabled: true,
        client_id: Some("client_id".to_string()),
        client_secret: Some("secret".to_string()),
        redirect_uri: Some("not-a-url".to_string()),
        authorization_endpoint: Some("https://example.com/auth".to_string()),
        token_endpoint: Some("https://example.com/token".to_string()),
        scopes: vec![],
        provider: OAuthProvider::Custom,
    };
    assert!(config
        .validate()
        .unwrap_err()
        .to_string()
        .contains("redirect_uri"));

    config.redirect_uri = Some("http://localhost/callback".to_string());
    config.authorization_endpoint = Some("bad-url".to_string());
    assert!(config
        .validate()
        .unwrap_err()
        .to_string()
        .contains("authorization_endpoint"));

    config.authorization_endpoint = Some("https://example.com/auth".to_string());
    config.token_endpoint = Some("not\\valid".to_string());
    assert!(config
        .validate()
        .unwrap_err()
        .to_string()
        .contains("token_endpoint"));
}

// ============================================================================
// AuthManager tests
// ============================================================================

#[test]
fn test_auth_manager_new_and_accessors() {
    let disabled = OAuthConfig::default();
    let manager = AuthManager::new(disabled.clone()).unwrap();
    assert!(!manager.is_enabled());
    assert_eq!(manager.config().enabled, disabled.enabled);

    let enabled = OAuthConfig::github(
        "client".to_string(),
        "secret".to_string(),
        "http://localhost/callback".to_string(),
    );
    let manager = AuthManager::new(enabled.clone()).unwrap();
    assert!(manager.is_enabled());
    assert_eq!(manager.config().client_id, enabled.client_id);
}

#[cfg(feature = "auth")]
#[test]
fn test_oauth_to_mcp_config_without_feature() {
    let config = OAuthConfig::default();
    let result = config.to_mcp_config();
    // Default enabled=false, should return error
    assert!(result.is_err());
    if let Err(e) = result {
        // Structured error message format: "Configuration error for 'oauth': is not enabled"
        let err_msg = e.to_string();
        assert!(
            err_msg.contains("oauth") && err_msg.contains("not enabled"),
            "Expected error message to contain 'oauth' and 'not enabled', got: {err_msg}"
        );
    }
}

#[cfg(not(feature = "auth"))]
#[test]
fn test_oauth_to_mcp_config_without_feature() {
    let config = OAuthConfig::default();
    let result = config.to_mcp_config();
    // feature not enabled, should return error
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("oauth"));
    }
}

// ============================================================================
// TokenStore tests
// ============================================================================

#[tokio::test]
async fn test_token_store_operations() {
    let store = TokenStore::new();

    // Test storage and retrieval
    let token = TokenInfo {
        access_token: "access123".to_string(),
        refresh_token: Some("refresh456".to_string()),
        expires_at: chrono::Utc::now() + chrono::Duration::seconds(3600),
        scopes: vec!["read".to_string(), "write".to_string()],
        user_id: Some("user123".to_string()),
        user_email: Some("user@example.com".to_string()),
    };

    assert!(store
        .store_token("user1".to_string(), token.clone())
        .await
        .is_ok());

    let retrieved: Result<Option<TokenInfo>, _> = store.get_token("user1").await;
    assert!(retrieved.as_ref().unwrap().is_some());

    let retrieved_value = retrieved.unwrap().unwrap();
    assert_eq!(retrieved_value.access_token, "access123");

    // Test deletion
    assert!(store.remove_token("user1").await.is_ok());

    let deleted: Result<Option<TokenInfo>, _> = store.get_token("user1").await;
    assert!(deleted.unwrap().is_none());
}

#[tokio::test]
async fn test_token_store_cleanup() {
    let store = TokenStore::new();

    // Store an expired token
    let expired_token = TokenInfo {
        access_token: "expired".to_string(),
        refresh_token: None,
        expires_at: chrono::Utc::now() - chrono::Duration::seconds(1),
        scopes: vec![],
        user_id: None,
        user_email: None,
    };

    // Store a valid token
    let valid_token = TokenInfo {
        access_token: "valid".to_string(),
        refresh_token: None,
        expires_at: chrono::Utc::now() + chrono::Duration::seconds(3600),
        scopes: vec![],
        user_id: None,
        user_email: None,
    };

    assert!(store
        .store_token("expired_user".to_string(), expired_token)
        .await
        .is_ok());
    assert!(store
        .store_token("valid_user".to_string(), valid_token)
        .await
        .is_ok());

    // Perform cleanup
    assert!(store.cleanup_expired().await.is_ok());

    // Expired token should be deleted
    let expired: Result<Option<TokenInfo>, _> = store.get_token("expired_user").await;
    assert!(expired.unwrap().is_none());

    // Valid token should be retained
    let valid: Result<Option<TokenInfo>, _> = store.get_token("valid_user").await;
    assert!(valid.unwrap().is_some());
}

// ============================================================================
// API Key configuration tests (requires api-key feature)
// ============================================================================

#[cfg(feature = "api-key")]
mod api_key_tests {
    use crates_docs::server::auth::{ApiKeyConfig, AuthConfig, AuthContext, AuthProvider};

    fn generate_hashed_key() -> (String, String) {
        let generator = ApiKeyConfig::default();
        let generated = generator
            .generate_key()
            .expect("failed to generate API key");
        (generated.key, generated.hash)
    }

    #[test]
    fn test_api_key_config_default() {
        let config = ApiKeyConfig::default();
        assert!(!config.enabled);
        assert!(config.keys.is_empty());
        assert_eq!(config.header_name, "X-API-Key");
        assert_eq!(config.query_param_name, "api_key");
        assert!(!config.allow_query_param);
        assert_eq!(config.key_prefix, "sk");
    }

    #[test]
    fn test_api_key_config_validate_disabled() {
        let config = ApiKeyConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_api_key_config_validate_enabled_no_keys() {
        let config = ApiKeyConfig {
            enabled: true,
            ..Default::default()
        };
        // Should succeed but log a warning
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_api_key_config_validate_empty_header_name() {
        let config = ApiKeyConfig {
            enabled: true,
            keys: vec!["test_key".to_string()],
            header_name: String::new(),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_api_key_is_valid_disabled() {
        let config = ApiKeyConfig::default();
        // When disabled, all keys should be accepted
        assert!(config.is_valid_key("any_key"));
        assert!(config.is_valid_key("invalid_key"));
    }

    #[test]
    fn test_api_key_is_valid_enabled() {
        let (plain_key_1, hashed_key_1) = generate_hashed_key();
        let (plain_key_2, hashed_key_2) = generate_hashed_key();

        let config = ApiKeyConfig {
            enabled: true,
            keys: vec![hashed_key_1, hashed_key_2],
            ..Default::default()
        };

        assert!(config.is_valid_key(&plain_key_1));
        assert!(config.is_valid_key(&plain_key_2));
        assert!(!config.is_valid_key("invalid_key"));
        assert!(!config.is_valid_key("sk_wrong"));
    }

    #[test]
    fn test_auth_config_with_api_key() {
        let mut config = AuthConfig::default();
        assert!(!config.is_enabled());

        #[cfg(feature = "api-key")]
        {
            let (_, hashed_key) = generate_hashed_key();
            config.api_key.enabled = true;
            config.api_key.keys = vec![hashed_key];
            assert!(config.is_enabled());
        }
    }

    #[test]
    fn test_auth_context_with_api_key() {
        let ctx = AuthContext::new(AuthProvider::ApiKey);
        assert!(ctx.is_authenticated());
        assert!(ctx.api_key_id.is_none());
    }

    #[test]
    fn test_auth_context_none() {
        let ctx = AuthContext::new(AuthProvider::None);
        assert!(!ctx.is_authenticated());
    }
}

// ============================================================================
// API Key middleware tests (requires api-key feature)
// ============================================================================

#[cfg(feature = "api-key")]
mod auth_middleware_tests {
    use crates_docs::server::auth::ApiKeyConfig;
    use crates_docs::server::auth_middleware::{ApiKeyMiddleware, AuthError};
    use std::collections::HashMap;

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

        let headers = HashMap::new();
        assert!(middleware.validate_request(&headers, None));
    }

    #[test]
    fn test_middleware_valid_key_header() {
        let (config, valid_key) = create_test_config();
        let middleware = ApiKeyMiddleware::new(config);

        let mut headers = HashMap::new();
        headers.insert("X-API-Key".to_string(), valid_key);

        assert!(middleware.validate_request(&headers, None));
    }

    #[test]
    fn test_middleware_invalid_key_header() {
        let (config, _) = create_test_config();
        let middleware = ApiKeyMiddleware::new(config);

        let mut headers = HashMap::new();
        headers.insert("X-API-Key".to_string(), "invalid_key".to_string());

        assert!(!middleware.validate_request(&headers, None));
    }

    #[test]
    fn test_middleware_missing_key() {
        let (config, _) = create_test_config();
        let middleware = ApiKeyMiddleware::new(config);

        let headers = HashMap::new();
        assert!(!middleware.validate_request(&headers, None));
    }

    #[test]
    fn test_middleware_query_param_allowed() {
        let (mut config, valid_key) = create_test_config();
        config.allow_query_param = true;
        let middleware = ApiKeyMiddleware::new(config);

        let headers = HashMap::new();
        let mut query_params = HashMap::new();
        query_params.insert("api_key".to_string(), valid_key);

        assert!(middleware.validate_request(&headers, Some(&query_params)));
    }

    #[test]
    fn test_middleware_query_param_not_allowed() {
        let (config, valid_key) = create_test_config();
        let middleware = ApiKeyMiddleware::new(config);

        let headers = HashMap::new();
        let mut query_params = HashMap::new();
        query_params.insert("api_key".to_string(), valid_key);

        assert!(!middleware.validate_request(&headers, Some(&query_params)));
    }

    #[test]
    fn test_middleware_extract_key_from_header() {
        let (config, valid_key) = create_test_config();
        let middleware = ApiKeyMiddleware::new(config);

        let mut headers = HashMap::new();
        headers.insert("X-API-Key".to_string(), valid_key.clone());

        let key = middleware.extract_key(&headers, None);
        assert_eq!(key, Some(valid_key));
    }

    #[test]
    fn test_middleware_extract_key_from_query() {
        let (mut config, valid_key) = create_test_config();
        config.allow_query_param = true;
        let middleware = ApiKeyMiddleware::new(config);

        let headers = HashMap::new();
        let mut query_params = HashMap::new();
        query_params.insert("api_key".to_string(), valid_key.clone());

        let key = middleware.extract_key(&headers, Some(&query_params));
        assert_eq!(key, Some(valid_key));
    }

    #[test]
    fn test_middleware_is_enabled() {
        let (config, _) = create_test_config();
        let middleware = ApiKeyMiddleware::new(config);
        assert!(middleware.is_enabled());

        let disabled_config = ApiKeyConfig::default();
        let disabled_middleware = ApiKeyMiddleware::new(disabled_config);
        assert!(!disabled_middleware.is_enabled());
    }

    #[test]
    fn test_auth_error_unauthorized() {
        let error = AuthError::unauthorized();
        assert_eq!(error.message, "Unauthorized: API key required");
        assert!(error.www_authenticate.is_some());
    }

    #[test]
    fn test_auth_error_invalid_key() {
        let error = AuthError::invalid_key();
        assert_eq!(error.message, "Unauthorized: Invalid API key");
        assert!(error.www_authenticate.is_some());
    }

    #[test]
    fn test_auth_error_display() {
        let error = AuthError::new("Test error");
        assert_eq!(format!("{error}"), "Test error");
    }
}
