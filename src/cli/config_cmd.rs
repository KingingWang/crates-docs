//! Config command implementation

use std::path::PathBuf;

/// Generate configuration file command
pub fn run_config_command(output: &PathBuf, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    if output.exists() && !force {
        return Err(format!(
            "Config file already exists: {}, use --force to overwrite",
            output.display()
        )
        .into());
    }

    let config = crate::config::AppConfig::default();
    config
        .save_to_file(output)
        .map_err(|e| format!("Failed to save config file: {e}"))?;

    println!("Config file generated: {}", output.display());
    println!("Please edit the config file as needed.");

    Ok(())
}
