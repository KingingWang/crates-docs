//! Revoke API key command implementation

use std::fs;
use std::path::Path;

/// Revoke an API key from configuration file.
///
/// Removes the specified key hash or key ID from the configuration file.
/// The configuration file format and comments are preserved.
///
/// # Errors
///
/// Returns an error if:
/// - The configuration file cannot be read or written
/// - The specified key is not found
/// - The configuration file format is invalid
pub fn run_revoke_api_key_command(
    config_path: &Path,
    key_to_revoke: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if config file exists
    if !config_path.exists() {
        return Err(format!("Configuration file not found: {}", config_path.display()).into());
    }

    // Read the configuration file content
    let content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read configuration file: {e}"))?;

    // Parse as TOML document (preserves comments and formatting)
    let mut doc = content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("Failed to parse configuration file: {e}"))?;

    // Get the api_key table
    let api_key_table = doc
        .get_mut("api_key")
        .and_then(|item| item.as_table_mut())
        .ok_or("API key configuration section not found in config file")?;

    // Get the keys array
    let keys_array = api_key_table
        .get_mut("keys")
        .and_then(|item| item.as_array_mut())
        .ok_or("API keys array not found in configuration")?;

    // Find and remove the key
    let mut found = false;
    let mut indices_to_remove = Vec::new();

    for (index, item) in keys_array.iter().enumerate() {
        if let Some(key_value) = item.as_str() {
            // Check if this is the key we want to revoke
            // Match by:
            // 1. Exact hash match
            // 2. Key ID match (extracted from hash)
            // 3. Partial match (for convenience)

            if key_value == key_to_revoke {
                indices_to_remove.push(index);
                found = true;
                break;
            }

            // Try to extract key_id from the hash
            // Argon2 hashes have format: $argon2id$v=19$m=...,t=...,p=...$<salt>$<hash>
            // We can try to match by partial content
            if key_value.contains(key_to_revoke) {
                indices_to_remove.push(index);
                found = true;
                break;
            }

            // Try to match by salt portion (which might be used as key_id)
            // Extract salt from Argon2 PHC format
            if key_value.starts_with("$argon2") {
                let parts: Vec<&str> = key_value.split('$').collect();
                if parts.len() >= 4 {
                    let salt = parts[3];
                    if salt == key_to_revoke {
                        indices_to_remove.push(index);
                        found = true;
                        break;
                    }
                }
            }
        }
    }

    if !found {
        println!("Key not found in configuration: {key_to_revoke}");
        println!();
        println!("Tip: Use 'crates-docs list-api-keys' to see all configured keys.");
        return Err("Key not found".into());
    }

    // Remove the key(s) - remove from highest index first to maintain validity
    indices_to_remove.sort_unstable();
    for index in indices_to_remove.iter().rev() {
        keys_array.remove(*index);
    }

    // Write back to file
    let new_content = doc.to_string();
    fs::write(config_path, new_content)
        .map_err(|e| format!("Failed to write configuration file: {e}"))?;

    println!("API key revoked successfully!");
    println!();
    println!(
        "Removed {} key(s) from: {}",
        indices_to_remove.len(),
        config_path.display()
    );
    println!();
    println!(
        "Note: If the server is running, you may need to restart it for changes to take effect."
    );
    println!("      Or use hot-reload feature if available.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_revoke_api_key_removes_key() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#"
[server]
host = "127.0.0.1"
port = 8080

[api_key]
enabled = true
keys = ["$argon2id$v=19$m=47104,t=1,p=1$c2FsdA$hash1", "$argon2id$v=19$m=47104,t=1,p=1$c2FsdA$hash2"]
header_name = "X-API-Key"
"#;
        temp_file.write_all(content.as_bytes()).unwrap();

        let path = temp_file.path();
        let result =
            run_revoke_api_key_command(path, "$argon2id$v=19$m=47104,t=1,p=1$c2FsdA$hash1");
        assert!(result.is_ok());

        // Verify the key was removed
        let new_content = std::fs::read_to_string(path).unwrap();
        assert!(new_content.contains("hash2"));
        assert!(!new_content.contains("hash1"));
    }

    #[test]
    fn test_revoke_api_key_preserves_comments() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#"
[server]
# Server configuration
host = "127.0.0.1"
port = 8080

[api_key]
# API key configuration
enabled = true
# List of API key hashes
keys = ["$argon2id$v=19$m=47104,t=1,p=1$c2FsdA$hash1", "$argon2id$v=19$m=47104,t=1,p=1$c2FsdA$hash2"]
header_name = "X-API-Key"
"#;
        temp_file.write_all(content.as_bytes()).unwrap();

        let path = temp_file.path();
        let result =
            run_revoke_api_key_command(path, "$argon2id$v=19$m=47104,t=1,p=1$c2FsdA$hash1");
        assert!(result.is_ok());

        // Verify comments are preserved
        let new_content = std::fs::read_to_string(path).unwrap();
        assert!(new_content.contains("# Server configuration"));
        assert!(new_content.contains("# API key configuration"));
        assert!(new_content.contains("# List of API key hashes"));
    }

    #[test]
    fn test_revoke_api_key_not_found() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#"
[api_key]
enabled = true
keys = ["$argon2id$v=19$m=47104,t=1,p=1$c2FsdA$hash1"]
"#;
        temp_file.write_all(content.as_bytes()).unwrap();

        let path = temp_file.path();
        let result = run_revoke_api_key_command(path, "nonexistent_key");
        assert!(result.is_err());
    }

    #[test]
    fn test_revoke_api_key_file_not_found() {
        let result = run_revoke_api_key_command(Path::new("/nonexistent/config.toml"), "key");
        assert!(result.is_err());
    }
}
