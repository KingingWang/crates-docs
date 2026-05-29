//! API key command implementation

#[cfg(feature = "api-key")]
use crate::server::auth::ApiKeyConfig;

/// Generate a new API key and corresponding hash for secure storage.
///
/// Prints the plain-text key once for the operator to copy, along with the
/// stable key ID and the Argon2 PHC hash that should be stored in config or
/// secret storage.
///
/// # Errors
///
/// Returns an error if key generation fails or if the prefix is invalid.
#[cfg(feature = "api-key")]
pub fn run_generate_api_key_command(prefix: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config = ApiKeyConfig {
        key_prefix: prefix.to_string(),
        ..Default::default()
    };

    let generated = config
        .generate_key()
        .map_err(|e| format!("Failed to generate API key: {e}"))?;

    println!("Generated API key successfully.");
    println!();
    println!("Plain-text key (show once and store securely):");
    println!("{}", generated.key);
    println!();
    println!("Key ID:");
    println!("{}", generated.key_id);
    println!();
    println!("Store this hash in configuration or secret storage:");
    println!("{}", generated.hash);
    println!();
    println!("Example config:");
    println!("[auth.api_key]");
    println!("enabled = true");
    println!("keys = [\"{}\"]", generated.hash);
    println!("header_name = \"X-API-Key\"");
    println!("query_param_name = \"api_key\"");
    println!("allow_query_param = false");
    println!("key_prefix = \"{prefix}\"");

    Ok(())
}

/// Fallback implementation when the `api-key` feature is disabled.
///
/// # Errors
///
/// Always returns an error because API key support is not compiled in.
#[cfg(not(feature = "api-key"))]
pub fn run_generate_api_key_command(_prefix: &str) -> Result<(), Box<dyn std::error::Error>> {
    Err("API key support is not enabled in this build".into())
}
