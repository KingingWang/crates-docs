//! End-to-end API-key enforcement tests.
//!
//! Proves that with `api_key.enabled = true` on the HTTP transport:
//! - `/health` stays open (no authentication required, for monitoring),
//! - MCP requests without `Authorization: Bearer <key>` are rejected with 401,
//! - an invalid bearer token is rejected with 401,
//! - a valid key presented as a bearer token is accepted.
//!
//! Gated on `api-key` + `auth` (both in the default feature set); without them
//! there is no in-process enforcement to exercise.
#![cfg(all(feature = "api-key", feature = "auth"))]

use crates_docs::server::auth::ApiKeyConfig;
use crates_docs::{AppConfig, CratesDocsServer};
use std::time::Duration;

/// Build an HTTP-mode config with API-key auth enabled, returning the config
/// and the plain-text key to present as a bearer token. The stored key material
/// is the Argon2 hash, mirroring a real deployment.
fn http_config_with_api_key(port: u16) -> (AppConfig, String) {
    let generated = ApiKeyConfig::default()
        .generate_key()
        .expect("failed to generate API key");

    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();
    config.auth.api_key.enabled = true;
    config.auth.api_key.keys = vec![generated.hash];

    (config, generated.key)
}

/// Full round-trip: health open, unauthenticated/invalid rejected, valid accepted.
#[tokio::test]
async fn api_key_enforced_on_http_transport() {
    let port = super::get_random_port();
    let (config, valid_key) = http_config_with_api_key(port);

    let server = CratesDocsServer::new_async(config)
        .await
        .expect("failed to create server");
    let handle = tokio::spawn(async move { server.run_http().await });

    // `/health` must stay OPEN: this poll sends no Authorization header, so its
    // success simultaneously proves the server is up and that the health
    // endpoint bypasses the auth middleware.
    let ready = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_health_check(port, Duration::from_secs(3)),
    )
    .await;
    assert!(
        ready.is_ok() && ready.unwrap().is_ok(),
        "/health should be reachable without authentication"
    );

    let client = super::create_test_client();
    let url = format!("http://127.0.0.1:{port}/mcp");
    let init = super::create_initialize_request(1);

    // 1. No Authorization header → 401.
    let resp = client
        .post(&url)
        .header("Accept", "application/json, text/event-stream")
        .json(&init)
        .send()
        .await
        .expect("request without auth should still reach the server");
    assert_eq!(
        resp.status().as_u16(),
        401,
        "MCP request without a bearer token must be rejected with 401"
    );

    // 2. Invalid bearer token → 401.
    let resp = client
        .post(&url)
        .header("Accept", "application/json, text/event-stream")
        .header("Authorization", "Bearer definitely-not-a-valid-key")
        .json(&init)
        .send()
        .await
        .expect("request with a bad token should still reach the server");
    assert_eq!(
        resp.status().as_u16(),
        401,
        "MCP request with an invalid bearer token must be rejected with 401"
    );

    // 3. Valid key as a bearer token → accepted (passes the auth layer).
    let resp = client
        .post(&url)
        .header("Accept", "application/json, text/event-stream")
        .header("Authorization", format!("Bearer {valid_key}"))
        .json(&init)
        .send()
        .await
        .expect("request with valid auth should reach the server");
    assert_ne!(
        resp.status().as_u16(),
        401,
        "a valid API key presented as a bearer token must pass the auth layer"
    );
    assert!(
        resp.status().is_success(),
        "initialize with a valid bearer token should succeed, got {}",
        resp.status()
    );

    handle.abort();
}

/// With `api_key.enabled = false`, the transport must NOT enforce auth: an
/// unauthenticated MCP request is processed normally (no 401). This guards the
/// runtime on/off switch so enabling the feature at build time does not
/// silently lock down a server whose config left auth disabled.
#[tokio::test]
async fn no_enforcement_when_api_key_disabled() {
    let port = super::get_random_port();
    let mut config = AppConfig::default();
    config.server.port = port;
    config.server.transport_mode = "http".to_string();
    config.server.host = "127.0.0.1".to_string();
    // Explicitly disabled (also the default) — the runtime switch is off.
    config.auth.api_key.enabled = false;

    let server = CratesDocsServer::new_async(config)
        .await
        .expect("failed to create server");
    let handle = tokio::spawn(async move { server.run_http().await });

    let ready = tokio::time::timeout(
        Duration::from_secs(5),
        super::wait_for_server(port, Duration::from_secs(3)),
    )
    .await;
    assert!(
        ready.is_ok() && ready.unwrap().is_ok(),
        "server should start"
    );

    let client = super::create_test_client();
    let url = format!("http://127.0.0.1:{port}/mcp");
    let init = super::create_initialize_request(1);

    let resp = client
        .post(&url)
        .header("Accept", "application/json, text/event-stream")
        .json(&init)
        .send()
        .await
        .expect("request should reach the server");
    assert_ne!(
        resp.status().as_u16(),
        401,
        "with api_key.enabled = false, requests must not be challenged for auth"
    );
    assert!(
        resp.status().is_success(),
        "unauthenticated initialize should succeed when auth is disabled, got {}",
        resp.status()
    );

    handle.abort();
}
