//! Version command implementation

/// Display version information
pub fn run_version_command() {
    println!("Crates Docs MCP Server v{}", env!("CARGO_PKG_VERSION"));
    println!("Build time: {}", env!("BUILD_TIMESTAMP"));
    println!("Git commit: {}", env!("GIT_COMMIT"));
    println!("Rust version: {}", env!("RUST_VERSION"));
}
