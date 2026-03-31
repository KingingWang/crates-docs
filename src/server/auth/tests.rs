use super::*;

#[test]
fn test_oauth_config_default() {
    let config = OAuthConfig::default();
    assert!(!config.enabled);
    assert!(config.client_id.is_none());
}

#[test]
fn test_oauth_config_github() {
    let config = OAuthConfig::github(
        "client_id".to_string(),
        "client_secret".to_string(),
        "http://localhost:8080/callback".to_string(),
    );
    assert!(config.enabled);
    assert_eq!(config.provider, OAuthProvider::GitHub);
}

#[test]
fn test_oauth_config_validate() {
    let config = OAuthConfig::default();
    assert!(config.validate().is_ok());

    let config = OAuthConfig {
        enabled: true,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[cfg(feature = "api-key")]
#[test]
fn test_api_key_config_default() {
    let config = ApiKeyConfig::default();
    assert!(!config.enabled);
    assert!(config.keys.is_empty());
    assert_eq!(config.header_name, "X-API-Key");
    assert_eq!(config.key_prefix, "sk");
}

#[cfg(feature = "api-key")]
#[test]
fn test_api_key_config_validate() {
    let config = ApiKeyConfig::default();
    assert!(config.validate().is_ok());

    let config = ApiKeyConfig {
        enabled: true,
        header_name: String::new(),
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[cfg(feature = "api-key")]
#[test]
fn test_api_key_is_valid() {
    let key_config = ApiKeyConfig {
        enabled: true,
        ..Default::default()
    };
    let generated = key_config.generate_key().unwrap();

    let config = ApiKeyConfig {
        enabled: true,
        keys: vec![generated.hash.clone()],
        ..Default::default()
    };

    assert!(config.is_valid_key(&generated.key));
    assert!(!config.is_valid_key("invalid_key"));
}

#[cfg(feature = "api-key")]
#[test]
fn test_api_key_disabled_allows_all() {
    let config = ApiKeyConfig::default();
    assert!(!config.enabled);

    // When disabled, all keys should be accepted
    assert!(config.is_valid_key("any_key"));
}

#[cfg(feature = "api-key")]
#[test]
fn test_api_key_plaintext_fallback() {
    let config = ApiKeyConfig {
        enabled: true,
        keys: vec!["legacy_plaintext_key".to_string()],
        ..Default::default()
    };

    assert!(config.is_valid_key("legacy_plaintext_key"));
    assert!(!config.is_valid_key("legacy_plaintext_key_2"));
}

#[cfg(feature = "api-key")]
#[test]
fn test_api_key_legacy_hashed_verification() {
    let config = ApiKeyConfig::default();
    let legacy_hash = config
        .normalize_key_material("legacy_plaintext_key")
        .unwrap();

    let enabled_config = ApiKeyConfig {
        enabled: true,
        keys: vec![legacy_hash],
        ..Default::default()
    };

    assert!(enabled_config.is_valid_key("legacy_plaintext_key"));
    assert!(!enabled_config.is_valid_key("legacy_plaintext_key_2"));
}

#[cfg(feature = "api-key")]
#[test]
fn test_api_key_generate_key_returns_hash_and_key() {
    let config = ApiKeyConfig::default();
    let generated = config.generate_key().unwrap();

    assert!(
        generated.key.starts_with("sk-")
            || generated.key.starts_with("sk_")
            || generated.key.starts_with("sk")
    );
    assert!(!generated.key_id.is_empty());
    assert!(generated.hash.starts_with("$argon2"));
}

#[test]
fn test_auth_config_default() {
    let config = AuthConfig::default();
    assert!(!config.is_enabled());
}

#[test]
fn test_auth_context() {
    let ctx = AuthContext::new(AuthProvider::None);
    assert!(!ctx.is_authenticated());

    let ctx = AuthContext::new(AuthProvider::OAuth);
    assert!(ctx.is_authenticated());
}

#[test]
fn test_token_store() {
    let store = TokenStore::new();
    let token = TokenInfo {
        access_token: "test_token".to_string(),
        refresh_token: None,
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        scopes: vec!["read".to_string()],
        user_id: None,
        user_email: None,
    };

    store.store_token("key".to_string(), token.clone());
    assert!(store.get_token("key").is_some());
    assert!(store.get_token("nonexistent").is_none());

    store.remove_token("key");
    assert!(store.get_token("key").is_none());
}

// ============================================================================
// AuthManager comprehensive tests
// ============================================================================

#[test]
fn test_auth_manager_new_with_valid_config() {
    let config = OAuthConfig::default();
    let manager = AuthManager::new(config).unwrap();
    assert!(!manager.is_enabled());
}

#[test]
fn test_auth_manager_new_with_enabled_oauth() {
    let config = OAuthConfig::github(
        "test_client".to_string(),
        "test_secret".to_string(),
        "http://localhost:8080/callback".to_string(),
    );
    let manager = AuthManager::new(config.clone()).unwrap();
    assert!(manager.is_enabled());
    assert_eq!(manager.config().provider, OAuthProvider::GitHub);
}

#[test]
fn test_auth_manager_new_with_invalid_config() {
    let config = OAuthConfig {
        enabled: true,
        ..Default::default()
    };
    let result = AuthManager::new(config);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("client_id"));
}

#[test]
fn test_auth_manager_config_accessor() {
    let config = OAuthConfig::github(
        "test_client".to_string(),
        "test_secret".to_string(),
        "http://localhost:8080/callback".to_string(),
    );
    let manager = AuthManager::new(config.clone()).unwrap();
    let retrieved_config = manager.config();
    assert!(retrieved_config.enabled);
    assert_eq!(retrieved_config.client_id, config.client_id);
}

#[test]
fn test_auth_manager_is_enabled_oauth_only() {
    let config = OAuthConfig::github(
        "test_client".to_string(),
        "test_secret".to_string(),
        "http://localhost:8080/callback".to_string(),
    );
    let manager = AuthManager::new(config).unwrap();
    assert!(manager.is_enabled());
}

#[test]
fn test_auth_manager_is_enabled_disabled() {
    let config = OAuthConfig::default();
    let manager = AuthManager::new(config).unwrap();
    assert!(!manager.is_enabled());
}

#[test]
fn test_token_store_multiple_tokens() {
    let store = TokenStore::new();

    let token1 = TokenInfo {
        access_token: "token1".to_string(),
        refresh_token: None,
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        scopes: vec!["read".to_string()],
        user_id: Some("user1".to_string()),
        user_email: None,
    };

    let token2 = TokenInfo {
        access_token: "token2".to_string(),
        refresh_token: None,
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        scopes: vec!["write".to_string()],
        user_id: Some("user2".to_string()),
        user_email: None,
    };

    store.store_token("key1".to_string(), token1);
    store.store_token("key2".to_string(), token2);

    assert!(store.get_token("key1").is_some());
    assert!(store.get_token("key2").is_some());
    assert!(store.get_token("key3").is_none());
}

#[test]
fn test_token_store_cleanup_removes_only_expired() {
    let store = TokenStore::new();

    let now = chrono::Utc::now();

    for i in 0..5 {
        let token = TokenInfo {
            access_token: format!("token{i}"),
            refresh_token: None,
            expires_at: now + chrono::Duration::hours(i64::from(i) + 1),
            scopes: vec![],
            user_id: None,
            user_email: None,
        };
        store.store_token(format!("key{i}"), token);
    }

    store.cleanup_expired();

    // All tokens should still be there since they're not expired (min expires_at is 1 hour from now)
    assert!(store.get_token("key0").is_some());
    assert!(store.get_token("key1").is_some());
    assert!(store.get_token("key2").is_some());
    assert!(store.get_token("key3").is_some());
    assert!(store.get_token("key4").is_some());
}

#[test]
fn test_token_store_with_refresh_token() {
    let store = TokenStore::new();

    let token = TokenInfo {
        access_token: "access_token".to_string(),
        refresh_token: Some("refresh_token".to_string()),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        scopes: vec!["openid".to_string()],
        user_id: Some("user123".to_string()),
        user_email: Some("user@example.com".to_string()),
    };

    store.store_token("session".to_string(), token.clone());
    let retrieved = store.get_token("session").unwrap();

    assert_eq!(retrieved.access_token, "access_token");
    assert_eq!(retrieved.refresh_token, Some("refresh_token".to_string()));
    assert_eq!(retrieved.scopes, vec!["openid".to_string()]);
    assert_eq!(retrieved.user_id, Some("user123".to_string()));
    assert_eq!(retrieved.user_email, Some("user@example.com".to_string()));
}

// ============================================================================
// OAuthConfig comprehensive tests
// ============================================================================

#[test]
fn test_oauth_config_google_with_all_fields() {
    let config = OAuthConfig::google(
        "google_client".to_string(),
        "google_secret".to_string(),
        "http://localhost:8080/callback".to_string(),
    );

    assert!(config.enabled);
    assert_eq!(config.client_id, Some("google_client".to_string()));
    assert_eq!(config.client_secret, Some("google_secret".to_string()));
    assert_eq!(
        config.redirect_uri,
        Some("http://localhost:8080/callback".to_string())
    );
    assert!(config
        .authorization_endpoint
        .unwrap()
        .contains("google.com"));
    assert!(config.token_endpoint.unwrap().contains("googleapis.com"));
    assert_eq!(config.provider, OAuthProvider::Google);
}

#[test]
fn test_oauth_config_keycloak_with_realm() {
    let config = OAuthConfig::keycloak(
        "keycloak_client".to_string(),
        "keycloak_secret".to_string(),
        "http://localhost:8080/callback".to_string(),
        "https://keycloak.example.com",
        "myrealm",
    );

    assert!(config.enabled);
    assert_eq!(config.client_id, Some("keycloak_client".to_string()));
    assert_eq!(config.provider, OAuthProvider::Keycloak);
    assert!(config
        .authorization_endpoint
        .unwrap()
        .contains("/realms/myrealm/"));
    assert!(config.token_endpoint.unwrap().contains("/realms/myrealm/"));
}

#[test]
fn test_oauth_config_keycloak_trailing_slash_handling() {
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
    assert_eq!(config1.token_endpoint, config2.token_endpoint);
}

#[test]
fn test_oauth_config_validate_all_fields() {
    let config = OAuthConfig {
        enabled: true,
        client_id: Some("client".to_string()),
        client_secret: Some("secret".to_string()),
        redirect_uri: Some("http://localhost/callback".to_string()),
        authorization_endpoint: Some("https://example.com/auth".to_string()),
        token_endpoint: Some("https://example.com/token".to_string()),
        scopes: vec![],
        provider: OAuthProvider::Custom,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_oauth_config_validate_missing_client_secret() {
    let config = OAuthConfig {
        enabled: true,
        client_id: Some("client".to_string()),
        client_secret: None,
        redirect_uri: Some("http://localhost/callback".to_string()),
        authorization_endpoint: Some("https://example.com/auth".to_string()),
        token_endpoint: Some("https://example.com/token".to_string()),
        scopes: vec![],
        provider: OAuthProvider::Custom,
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("client_secret"));
}

#[test]
fn test_oauth_config_validate_missing_authorization_endpoint() {
    let config = OAuthConfig {
        enabled: true,
        client_id: Some("client".to_string()),
        client_secret: Some("secret".to_string()),
        redirect_uri: Some("http://localhost/callback".to_string()),
        authorization_endpoint: None,
        token_endpoint: Some("https://example.com/token".to_string()),
        scopes: vec![],
        provider: OAuthProvider::Custom,
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("authorization_endpoint"));
}

#[test]
fn test_oauth_config_validate_missing_token_endpoint() {
    let config = OAuthConfig {
        enabled: true,
        client_id: Some("client".to_string()),
        client_secret: Some("secret".to_string()),
        redirect_uri: Some("http://localhost/callback".to_string()),
        authorization_endpoint: Some("https://example.com/auth".to_string()),
        token_endpoint: None,
        scopes: vec![],
        provider: OAuthProvider::Custom,
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("token_endpoint"));
}

#[test]
fn test_oauth_config_validate_invalid_redirect_uri() {
    let config = OAuthConfig {
        enabled: true,
        client_id: Some("client".to_string()),
        client_secret: Some("secret".to_string()),
        redirect_uri: Some("not-a-valid-url".to_string()),
        authorization_endpoint: Some("https://example.com/auth".to_string()),
        token_endpoint: Some("https://example.com/token".to_string()),
        scopes: vec![],
        provider: OAuthProvider::Custom,
    };
    let result = config.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("redirect_uri"));
    assert!(err_msg.contains("Invalid URL"));
}

#[test]
fn test_oauth_config_validate_invalid_authorization_endpoint() {
    let config = OAuthConfig {
        enabled: true,
        client_id: Some("client".to_string()),
        client_secret: Some("secret".to_string()),
        redirect_uri: Some("http://localhost/callback".to_string()),
        authorization_endpoint: Some("invalid-url".to_string()),
        token_endpoint: Some("https://example.com/token".to_string()),
        scopes: vec![],
        provider: OAuthProvider::Custom,
    };
    let result = config.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("authorization_endpoint"));
    assert!(err_msg.contains("Invalid URL"));
}

#[test]
fn test_oauth_config_validate_invalid_token_endpoint() {
    let config = OAuthConfig {
        enabled: true,
        client_id: Some("client".to_string()),
        client_secret: Some("secret".to_string()),
        redirect_uri: Some("http://localhost/callback".to_string()),
        authorization_endpoint: Some("https://example.com/auth".to_string()),
        token_endpoint: Some("not\\a\\valid\\url".to_string()),
        scopes: vec![],
        provider: OAuthProvider::Custom,
    };
    let result = config.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("token_endpoint"));
    assert!(err_msg.contains("Invalid URL"));
}

#[test]
fn test_oauth_config_default_scopes() {
    let config = OAuthConfig::default();
    assert!(config.scopes.contains(&"openid".to_string()));
    assert!(config.scopes.contains(&"profile".to_string()));
    assert!(config.scopes.contains(&"email".to_string()));
}

#[test]
fn test_oauth_config_github_default_scopes() {
    let config = OAuthConfig::github(
        "client".to_string(),
        "secret".to_string(),
        "http://localhost/callback".to_string(),
    );
    assert!(config.scopes.contains(&"read:user".to_string()));
    assert!(config.scopes.contains(&"user:email".to_string()));
}

#[test]
fn test_oauth_config_google_default_scopes() {
    let config = OAuthConfig::google(
        "client".to_string(),
        "secret".to_string(),
        "http://localhost/callback".to_string(),
    );
    assert!(config.scopes.contains(&"openid".to_string()));
    assert!(config.scopes.iter().any(|s| s.contains("userinfo.profile")));
    assert!(config.scopes.iter().any(|s| s.contains("userinfo.email")));
}

#[test]
fn test_oauth_config_disabled_bypasses_validation() {
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
    assert!(config.validate().is_ok());
}

// ============================================================================
// AuthConfig comprehensive tests
// ============================================================================

#[test]
fn test_auth_config_validate_both_disabled() {
    let config = AuthConfig::default();
    assert!(!config.is_enabled());
    assert!(config.validate().is_ok());
}

#[test]
fn test_auth_config_validate_oauth_enabled() {
    let config = AuthConfig {
        oauth: OAuthConfig::github(
            "client".to_string(),
            "secret".to_string(),
            "http://localhost/callback".to_string(),
        ),
        ..Default::default()
    };
    assert!(config.is_enabled());
    assert!(config.validate().is_ok());
}

#[test]
fn test_auth_config_validate_oauth_invalid() {
    let config = AuthConfig {
        oauth: OAuthConfig {
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

// ============================================================================
// API Key comprehensive tests (feature-gated)
// ============================================================================

#[cfg(feature = "api-key")]
mod api_key_comprehensive_tests {
    use super::*;

    #[test]
    fn test_api_key_config_with_custom_header_name() {
        let config = ApiKeyConfig {
            enabled: true,
            header_name: "Authorization".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
        assert_eq!(config.header_name, "Authorization");
    }

    #[test]
    fn test_api_key_config_validate_empty_header_name() {
        let config = ApiKeyConfig {
            enabled: true,
            header_name: String::new(),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("header_name"));
    }

    #[test]
    fn test_api_key_config_validate_empty_key_prefix() {
        let config = ApiKeyConfig {
            enabled: true,
            keys: vec!["test".to_string()],
            key_prefix: String::new(),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("key_prefix"));
    }

    #[test]
    fn test_api_key_config_validate_query_param_with_empty_name() {
        let config = ApiKeyConfig {
            enabled: true,
            keys: vec!["test".to_string()],
            allow_query_param: true,
            query_param_name: String::new(),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("query_param_name"));
    }

    #[test]
    fn test_api_key_config_validate_query_param_disabled_allows_empty() {
        let config = ApiKeyConfig {
            enabled: true,
            keys: vec!["test".to_string()],
            allow_query_param: false,
            query_param_name: String::new(),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_api_key_is_valid_with_multiple_keys() {
        let generator = ApiKeyConfig::default();
        let key1 = generator.generate_key().unwrap();
        let key2 = generator.generate_key().unwrap();
        let key3 = generator.generate_key().unwrap();

        let config = ApiKeyConfig {
            enabled: true,
            keys: vec![key1.hash.clone(), key2.hash.clone(), key3.hash.clone()],
            ..Default::default()
        };

        assert!(config.is_valid_key(&key1.key));
        assert!(config.is_valid_key(&key2.key));
        assert!(config.is_valid_key(&key3.key));
        assert!(!config.is_valid_key("invalid_key"));
    }

    #[test]
    fn test_api_key_is_valid_with_mixed_hash_and_plaintext() {
        let generator = ApiKeyConfig::default();
        let hashed_key = generator.generate_key().unwrap();

        let config = ApiKeyConfig {
            enabled: true,
            keys: vec![hashed_key.hash.clone(), "plain_key".to_string()],
            ..Default::default()
        };

        assert!(config.is_valid_key(&hashed_key.key));
        assert!(config.is_valid_key("plain_key"));
        assert!(!config.is_valid_key("wrong_key"));
    }

    #[test]
    fn test_api_key_normalization_already_hashed() {
        let generator = ApiKeyConfig::default();
        let generated = generator.generate_key().unwrap();

        let config = ApiKeyConfig::default();
        let normalized = config.normalize_key_material(&generated.hash).unwrap();
        assert_eq!(normalized, generated.hash);
    }

    #[test]
    fn test_api_key_normalization_plaintext_to_hash() {
        let config = ApiKeyConfig::default();
        let plaintext = "my_secret_key";

        let normalized = config.normalize_key_material(plaintext).unwrap();
        assert!(normalized.starts_with("legacy:"));
        assert!(normalized.contains("$argon2"));
    }

    #[test]
    fn test_api_key_normalization_legacy_hash() {
        let config = ApiKeyConfig::default();
        let legacy_hash = "legacy:$argon2id$v=19$m=8,t=1,p=1$test$test";

        let normalized = config.normalize_key_material(legacy_hash).unwrap();
        assert_eq!(normalized, legacy_hash);
    }

    #[test]
    fn test_api_key_generate_key_unique() {
        let config = ApiKeyConfig::default();

        let key1 = config.generate_key().unwrap();
        let key2 = config.generate_key().unwrap();
        let key3 = config.generate_key().unwrap();

        assert_ne!(key1.key, key2.key);
        assert_ne!(key2.key, key3.key);
        assert_ne!(key1.key, key3.key);

        assert_ne!(key1.hash, key2.hash);
        assert_ne!(key2.hash, key3.hash);
    }

    #[test]
    fn test_api_key_generate_key_structure() {
        let config = ApiKeyConfig::default();
        let generated = config.generate_key().unwrap();

        assert!(!generated.key.is_empty());
        assert!(!generated.key_id.is_empty());
        assert!(!generated.hash.is_empty());
        assert!(generated.hash.starts_with("$argon2"));
    }

    #[test]
    fn test_api_key_is_valid_empty_keys_allows_none() {
        let config = ApiKeyConfig {
            enabled: true,
            keys: vec![],
            ..Default::default()
        };

        assert!(!config.is_valid_key("any_key"));
    }
}
