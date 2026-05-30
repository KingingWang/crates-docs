//! List API keys command implementation

use crate::config::AppConfig;
use std::path::Path;

/// Truncate a long hash to `<prefix>...<suffix>` for display, counting by
/// characters so the slice never lands inside a multi-byte UTF-8 sequence.
///
/// Byte-index slicing (`&hash[..30]`) panicked when a key hash loaded from a
/// user-supplied `config.toml` contained a multi-byte character straddling the
/// cut point. Hashes are normally ASCII (argon2/hex), where this is identical
/// to the previous behaviour, but operator-edited config must never crash the
/// audit command.
fn truncate_hash_for_display(hash: &str) -> String {
    const PREFIX_CHARS: usize = 30;
    const SUFFIX_CHARS: usize = 20;
    let char_count = hash.chars().count();
    if char_count > PREFIX_CHARS + SUFFIX_CHARS + 10 {
        let prefix: String = hash.chars().take(PREFIX_CHARS).collect();
        let suffix: String = hash.chars().skip(char_count - SUFFIX_CHARS).collect();
        format!("{prefix}...{suffix}")
    } else {
        hash.to_string()
    }
}

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
            println!("      {}", truncate_hash_for_display(key_hash));
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

#[cfg(test)]
mod tests {
    use super::truncate_hash_for_display;

    #[test]
    fn short_hash_is_unchanged() {
        let h = "$argon2id$v=19$short";
        assert_eq!(truncate_hash_for_display(h), h);
    }

    #[test]
    fn long_ascii_hash_is_elided() {
        let h = "a".repeat(80);
        let out = truncate_hash_for_display(&h);
        assert_eq!(out, format!("{}...{}", "a".repeat(30), "a".repeat(20)));
        assert!(out.contains("..."));
    }

    #[test]
    fn long_multibyte_hash_does_not_panic_and_stays_valid_utf8() {
        // A multi-byte char straddling the old byte cut points (30 / len-20)
        // previously panicked with "not a char boundary".
        let h = format!(
            "{}{}{}",
            "a".repeat(28),
            "\u{597d}".repeat(20),
            "b".repeat(40)
        );
        let out = truncate_hash_for_display(&h);
        // No panic; result is valid UTF-8 and elided.
        assert!(out.contains("..."));
        assert!(out.starts_with("aaaa"));
        assert!(out.ends_with("bbbb"));
    }
}
