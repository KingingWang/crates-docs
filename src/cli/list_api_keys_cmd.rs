//! List API keys command implementation

use crate::config::AppConfig;
use std::path::Path;

/// List API keys from configuration file.
///
/// Reads the configuration file and displays all configured API key hashes.
/// This helps operators audit which keys are currently active.
///
/// # Errors
///
/// Returns an error if the configuration file cannot be read or parsed.
pub fn run_list_api_keys_command(config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Check if config file exists
    if !config_path.exists() {
        eprintln!("Configuration file not found: {}", config_path.display());
        eprintln!("No API keys are configured.");
        return Ok(());
    }

    // Load configuration
    let config = AppConfig::from_file(config_path).map_err(|e| {
        format!(
            "Failed to load configuration from {}: {}",
            config_path.display(),
            e
        )
    })?;

    println!("API Key Configuration");
    println!("=====================");
    println!();

    if !config.auth.api_key.enabled {
        println!("Status: DISABLED");
        println!();
        println!("API key authentication is not enabled.");
        println!("Set enabled = true in [auth.api_key] section to enable.");
        return Ok(());
    }

    println!("Status: ENABLED");
    println!();

    if config.auth.api_key.keys.is_empty() {
        println!("No API keys configured.");
        println!("Use 'crates-docs generate-api-key' to create a new key.");
    } else {
        println!("Configured API keys ({}):", config.auth.api_key.keys.len());
        println!();

        for (index, key_hash) in config.auth.api_key.keys.iter().enumerate() {
            let key_type = if key_hash.starts_with("legacy:") {
                "Legacy Hash"
            } else if key_hash.starts_with("$argon2") {
                "Argon2 Hash"
            } else {
                "Plaintext (Insecure)"
            };

            println!("  [{}] {}", index + 1, key_type);

            // Show a truncated version of the hash for identification
            let display_hash = if key_hash.len() > 60 {
                format!("{}...{}", &key_hash[..30], &key_hash[key_hash.len() - 20..])
            } else {
                key_hash.clone()
            };
            println!("      {display_hash}");
            println!();
        }

        println!("Configuration:");
        println!("  Header name: {}", config.auth.api_key.header_name);
        println!("  Query param: {}", config.auth.api_key.query_param_name);
        println!(
            "  Allow query param: {}",
            config.auth.api_key.allow_query_param
        );
        println!("  Key prefix: {}", config.auth.api_key.key_prefix);
    }

    println!();
    println!("File: {}", config_path.display());

    Ok(())
}
