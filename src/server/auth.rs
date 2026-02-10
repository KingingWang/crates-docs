//! OAuth 认证模块
//!
//! 提供 OAuth 2.0 认证支持。

use crate::error::{Error, Result};
use url::Url;

/// OAuth 配置
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OAuthConfig {
    /// 是否启用 OAuth
    pub enabled: bool,

    /// 客户端 ID
    pub client_id: Option<String>,

    /// 客户端密钥
    pub client_secret: Option<String>,

    /// 重定向 URI
    pub redirect_uri: Option<String>,

    /// 授权端点
    pub authorization_endpoint: Option<String>,

    /// 令牌端点
    pub token_endpoint: Option<String>,

    /// 范围
    pub scopes: Vec<String>,

    /// 认证提供者类型
    pub provider: OAuthProvider,
}

/// OAuth 提供者类型
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum OAuthProvider {
    /// 自定义 OAuth 提供者
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
    /// 创建 GitHub OAuth 配置
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

    /// 创建 Google OAuth 配置
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

    /// 创建 Keycloak OAuth 配置
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

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if self.client_id.is_none() {
            return Err(Error::Config("client_id 是必需的".to_string()));
        }

        if self.client_secret.is_none() {
            return Err(Error::Config("client_secret 是必需的".to_string()));
        }

        if self.redirect_uri.is_none() {
            return Err(Error::Config("redirect_uri 是必需的".to_string()));
        }

        if self.authorization_endpoint.is_none() {
            return Err(Error::Config("authorization_endpoint 是必需的".to_string()));
        }

        if self.token_endpoint.is_none() {
            return Err(Error::Config("token_endpoint 是必需的".to_string()));
        }

        // 验证 URL
        if let Some(uri) = &self.redirect_uri {
            Url::parse(uri).map_err(|e| Error::Config(format!("无效的 redirect_uri: {e}")))?;
        }

        if let Some(endpoint) = &self.authorization_endpoint {
            Url::parse(endpoint)
                .map_err(|e| Error::Config(format!("无效的 authorization_endpoint: {e}")))?;
        }

        if let Some(endpoint) = &self.token_endpoint {
            Url::parse(endpoint)
                .map_err(|e| Error::Config(format!("无效的 token_endpoint: {e}")))?;
        }

        Ok(())
    }

    /// 转换为 rust-mcp-sdk 的 OAuthConfig
    #[cfg(feature = "auth")]
    pub fn to_mcp_config(&self) -> Result<()> {
        if !self.enabled {
            return Err(Error::Config("OAuth 未启用".to_string()));
        }

        // 暂时返回空结果，等 OAuth 功能完善后再实现
        Ok(())
    }

    /// 转换为 rust-mcp-sdk 的 `OAuthConfig`
    #[cfg(not(feature = "auth"))]
    pub fn to_mcp_config(&self) -> Result<()> {
        Err(Error::Config("OAuth 功能未启用".to_string()))
    }
}

/// 认证管理器
#[derive(Default)]
pub struct AuthManager {
    config: OAuthConfig,
}

impl AuthManager {
    /// 创建新的认证管理器
    pub fn new(config: OAuthConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { config })
    }

    /// 检查是否启用认证
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// 获取配置
    #[must_use]
    pub fn config(&self) -> &OAuthConfig {
        &self.config
    }
}

/// 简单的内存令牌存储（生产环境应使用 Redis 或数据库）
#[derive(Default)]
pub struct TokenStore {
    tokens: std::sync::RwLock<std::collections::HashMap<String, TokenInfo>>,
}

/// 令牌信息
/// OAuth 令牌信息
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct TokenInfo {
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌（可选）
    pub refresh_token: Option<String>,
    /// 令牌过期时间
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// 授权范围
    pub scopes: Vec<String>,
    /// 用户ID（可选）
    pub user_id: Option<String>,
    /// 用户邮箱（可选）
    pub user_email: Option<String>,
}

impl TokenStore {
    /// 创建新的令牌存储
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 存储令牌
    pub fn store_token(&self, key: String, token: TokenInfo) {
        let mut tokens = self.tokens.write().unwrap();
        tokens.insert(key, token);
    }

    /// 获取令牌
    pub fn get_token(&self, key: &str) -> Option<TokenInfo> {
        let tokens = self.tokens.read().unwrap();
        tokens.get(key).cloned()
    }

    /// 删除令牌
    pub fn remove_token(&self, key: &str) {
        let mut tokens = self.tokens.write().unwrap();
        tokens.remove(key);
    }

    /// 清理过期令牌
    pub fn cleanup_expired(&self) {
        let now = chrono::Utc::now();
        let mut tokens = self.tokens.write().unwrap();
        tokens.retain(|_, token| token.expires_at > now);
    }
}
