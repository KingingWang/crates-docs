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
