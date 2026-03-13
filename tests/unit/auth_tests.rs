//! OAuth 和认证模块单元测试

use crates_docs::server::auth::{AuthManager, OAuthConfig, OAuthProvider, TokenInfo, TokenStore};

// ============================================================================
// OAuth 配置测试
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
    // OAuthProvider 没有实现 PartialEq，使用 Debug 进行比较
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
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("redirect_uri is required"));
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
        .contains("Invalid redirect_uri"));

    config.redirect_uri = Some("http://localhost/callback".to_string());
    config.authorization_endpoint = Some("bad-url".to_string());
    assert!(config
        .validate()
        .unwrap_err()
        .to_string()
        .contains("Invalid authorization_endpoint"));

    config.authorization_endpoint = Some("https://example.com/auth".to_string());
    config.token_endpoint = Some("bad-url".to_string());
    assert!(config
        .validate()
        .unwrap_err()
        .to_string()
        .contains("Invalid token_endpoint"));
}

// ============================================================================
// AuthManager 测试
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
    // 默认 enabled=false，应返回错误
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("OAuth is not enabled"));
    }
}

#[cfg(not(feature = "auth"))]
#[test]
fn test_oauth_to_mcp_config_without_feature() {
    let config = OAuthConfig::default();
    let result = config.to_mcp_config();
    // feature 未启用，应返回错误
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("OAuth feature is not enabled"));
    }
}

// ============================================================================
// TokenStore 测试
// ============================================================================

#[test]
fn test_token_store_operations() {
    let store = TokenStore::new();

    // 测试存储和获取
    let token = TokenInfo {
        access_token: "access123".to_string(),
        refresh_token: Some("refresh456".to_string()),
        expires_at: chrono::Utc::now() + chrono::Duration::seconds(3600),
        scopes: vec!["read".to_string(), "write".to_string()],
        user_id: Some("user123".to_string()),
        user_email: Some("user@example.com".to_string()),
    };

    store.store_token("user1".to_string(), token.clone());
    let retrieved = store.get_token("user1");
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.access_token, "access123");

    // 测试删除
    store.remove_token("user1");
    assert!(store.get_token("user1").is_none());
}

#[test]
fn test_token_store_cleanup() {
    let store = TokenStore::new();

    // 存储一个已过期的 token
    let expired_token = TokenInfo {
        access_token: "expired".to_string(),
        refresh_token: None,
        expires_at: chrono::Utc::now() - chrono::Duration::seconds(1),
        scopes: vec![],
        user_id: None,
        user_email: None,
    };

    // 存储一个有效的 token
    let valid_token = TokenInfo {
        access_token: "valid".to_string(),
        refresh_token: None,
        expires_at: chrono::Utc::now() + chrono::Duration::seconds(3600),
        scopes: vec![],
        user_id: None,
        user_email: None,
    };

    store.store_token("expired_user".to_string(), expired_token);
    store.store_token("valid_user".to_string(), valid_token);

    // 执行清理
    store.cleanup_expired();

    // 过期的 token 应该被删除
    assert!(store.get_token("expired_user").is_none());
    // 有效的 token 应该保留
    assert!(store.get_token("valid_user").is_some());
}
