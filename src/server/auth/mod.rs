//! Authentication module
//!
//! Provides OAuth 2.0 and API Key authentication support.
//!
//! # Authentication Methods
//!
//! - **OAuth 2.0**: Full OAuth flow with GitHub, Google, Keycloak support
//! - **API Key**: Simple API key authentication with secure hashing
//!
//! # Example
//!
//! ```rust,no_run
//! use crates_docs::server::auth::{ApiKeyConfig, AuthConfig};
//!
//! // Create API Key configuration
//! let api_key_config = ApiKeyConfig {
//!     enabled: true,
//!     keys: vec!["sk_live_xxx".to_string()],
//!     header_name: "X-API-Key".to_string(),
//!     ..Default::default()
//! };
//! ```

mod config;
mod manager;
mod token;
mod types;

#[cfg(test)]
mod tests;

#[cfg(feature = "api-key")]
pub use config::ApiKeyConfig;
pub use config::{AuthConfig, OAuthConfig};
pub use manager::AuthManager;
pub use token::{TokenInfo, TokenStore};
#[cfg(feature = "api-key")]
pub use types::GeneratedApiKey;
pub use types::{AuthContext, AuthProvider, OAuthProvider};
