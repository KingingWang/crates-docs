//! In-process API-key authentication provider for the HTTP/SSE transport.
//!
//! This bridges the project's [`ApiKeyConfig`] to the rust-mcp-sdk
//! [`AuthProvider`](rust_mcp_sdk::auth::AuthProvider) trait so the SDK's built-in
//! `AuthMiddleware` enforces API keys on every MCP request (the `/health`
//! endpoint is intentionally left open for monitoring).
//!
//! # Bearer only
//!
//! The SDK middleware understands **only** `Authorization: Bearer <token>` — it
//! cannot read the configured `X-API-Key` header or a query parameter. Callers
//! talking to the server directly must therefore send the key as a Bearer token:
//!
//! ```text
//! Authorization: Bearer sk_live_xxx
//! ```
//!
//! To keep using `X-API-Key` (and to add TLS), put the bundled reverse-proxy
//! config (`docs/reverse-proxy/`) in front of the server: it terminates TLS and
//! rewrites `X-API-Key: <k>` into `Authorization: Bearer <k>` for the backend.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use rust_mcp_sdk::auth::{
    AuthInfo, AuthProvider as SdkAuthProvider, AuthenticationError, OauthEndpoint,
};
use rust_mcp_sdk::mcp_http::{GenericBody, McpAppState};
use rust_mcp_sdk::mcp_server::error::TransportServerError;

use crate::server::auth::ApiKeyConfig;

/// Synthetic expiry handed to the SDK middleware for accepted API keys.
///
/// The SDK rejects any `AuthInfo` whose `expires_at` is `None` ("Token has no
/// expiration time") or already in the past. API keys are long-lived
/// credentials with no intrinsic expiry, so we report a fixed far-future
/// instant (~10 years, i.e. 24 × 365 × 10 hours). Revocation is performed by
/// removing the key from configuration and restarting the server — never by
/// token expiry — which matches the existing hot-reload security note in
/// `serve_cmd`.
const API_KEY_TTL_SECS: u64 = 87_600 * 60 * 60;

/// Opaque, non-secret identifier reported for every accepted API key.
///
/// `AuthInfo` is attached to the request/session and may be logged, so we must
/// never place the raw key here. API-key auth carries no per-user identity, so a
/// constant tag is sufficient and leaks nothing.
const API_KEY_TOKEN_ID: &str = "api-key";

/// Adapts the project's [`ApiKeyConfig`] to the rust-mcp-sdk `AuthProvider`
/// trait, enabling in-process Bearer-token enforcement of API keys.
pub struct ApiKeyAuthProvider {
    config: ApiKeyConfig,
}

impl ApiKeyAuthProvider {
    /// Create a provider backed by the given API-key configuration.
    #[must_use]
    pub fn new(config: ApiKeyConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl SdkAuthProvider for ApiKeyAuthProvider {
    async fn verify_token(&self, access_token: String) -> Result<AuthInfo, AuthenticationError> {
        // Fail closed, independent of how the provider was constructed. Today the
        // provider is only built when `enabled` is true (see
        // `transport::build_api_key_auth`), but `ApiKeyConfig::is_valid_key`
        // returns `true` for *any* token when `!enabled`. Guarding here ensures a
        // future caller that constructs the provider unconditionally (e.g. to
        // support toggling `enabled` for hot-reload) can never silently become a
        // blanket auth bypass.
        if !self.config.enabled {
            return Err(AuthenticationError::InvalidToken {
                description: "API key authentication is disabled",
            });
        }

        // Reject empty / whitespace-only bearer tokens up front. No generated API
        // key is empty, and this closes a configuration footgun: a stray
        // `keys = [""]` entry would otherwise let `Authorization: Bearer ` (an
        // empty token) authenticate via the plaintext fallback.
        if access_token.trim().is_empty() {
            return Err(AuthenticationError::InvalidToken {
                description: "Empty API key",
            });
        }

        // `is_valid_key` performs the actual (constant-time) verification against
        // the configured Argon2 / legacy / plaintext key material.
        if self.config.is_valid_key(&access_token) {
            Ok(AuthInfo {
                token_unique_id: API_KEY_TOKEN_ID.to_string(),
                client_id: None,
                user_id: None,
                scopes: None,
                expires_at: Some(SystemTime::now() + Duration::from_secs(API_KEY_TTL_SECS)),
                audience: None,
                extra: None,
            })
        } else {
            Err(AuthenticationError::InvalidToken {
                description: "Invalid API key",
            })
        }
    }

    fn auth_endpoints(&self) -> Option<&HashMap<String, OauthEndpoint>> {
        // API-key auth mounts no OAuth routes, so the SDK never invokes
        // `handle_request` below.
        None
    }

    async fn handle_request(
        &self,
        _request: http::Request<&str>,
        _state: Arc<McpAppState>,
    ) -> Result<http::Response<GenericBody>, TransportServerError> {
        // Unreachable: `auth_endpoints` returns `None`, so no route is mounted
        // that would dispatch here. Return an explicit error in case it ever is.
        Err(TransportServerError::HttpError(
            "API-key authentication exposes no OAuth endpoints".to_string(),
        ))
    }

    fn protected_resource_metadata_url(&self) -> Option<&str> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an enabled config holding the Argon2 hash of a freshly generated
    /// key, returning both the config and the plain-text key to present.
    fn config_with_key() -> (ApiKeyConfig, String) {
        let generated = ApiKeyConfig::default()
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

    #[tokio::test]
    async fn verify_token_accepts_valid_key_with_future_expiry() {
        let (config, key) = config_with_key();
        let provider = ApiKeyAuthProvider::new(config);

        let info = provider
            .verify_token(key)
            .await
            .expect("valid key should be accepted");

        // The SDK middleware requires a present, future expiry.
        let expires_at = info.expires_at.expect("expires_at must be Some");
        assert!(expires_at > SystemTime::now());
        // The raw key must never be echoed back as the token identifier.
        assert_eq!(info.token_unique_id, API_KEY_TOKEN_ID);
    }

    #[tokio::test]
    async fn verify_token_rejects_invalid_key() {
        let (config, _key) = config_with_key();
        let provider = ApiKeyAuthProvider::new(config);

        let err = provider
            .verify_token("not-a-valid-key".to_string())
            .await
            .expect_err("invalid key should be rejected");

        assert!(matches!(err, AuthenticationError::InvalidToken { .. }));
    }

    #[tokio::test]
    async fn verify_token_rejects_empty_token() {
        let (config, _key) = config_with_key();
        let provider = ApiKeyAuthProvider::new(config);

        // Empty and whitespace-only tokens must never authenticate, regardless of
        // configured key material (guards the `keys = [""]` plaintext footgun).
        for token in ["", "   "] {
            let err = provider
                .verify_token(token.to_string())
                .await
                .expect_err("empty token should be rejected");
            assert!(matches!(err, AuthenticationError::InvalidToken { .. }));
        }
    }

    #[tokio::test]
    async fn verify_token_rejects_valid_key_when_disabled() {
        // Even a genuinely valid key must be rejected when the config is disabled:
        // the provider fails closed rather than relying on its caller to never
        // build it while `enabled == false`.
        let (mut config, key) = config_with_key();
        config.enabled = false;
        let provider = ApiKeyAuthProvider::new(config);

        let err = provider
            .verify_token(key)
            .await
            .expect_err("a disabled provider must reject every token");

        assert!(matches!(err, AuthenticationError::InvalidToken { .. }));
    }

    #[test]
    fn exposes_no_oauth_surface() {
        let (config, _key) = config_with_key();
        let provider = ApiKeyAuthProvider::new(config);

        assert!(provider.auth_endpoints().is_none());
        assert!(provider.protected_resource_metadata_url().is_none());
    }
}
