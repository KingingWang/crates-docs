use std::process::Command;

fn main() {
    // Set build timestamp
    println!(
        "cargo:rustc-env=BUILD_TIMESTAMP={}",
        chrono::Utc::now().to_rfc3339()
    );

    // Get Git commit info
    let mut commit = String::from("unknown");
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if output.status.success() {
            commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    }
    println!("cargo:rustc-env=GIT_COMMIT={}", commit);

    // Get Rust version
    let mut version = String::from("unknown");
    if let Ok(output) = Command::new("rustc").args(["--version"]).output() {
        if output.status.success() {
            version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    }
    println!("cargo:rustc-env=RUST_VERSION={}", version);

    // Cargo rerun logic
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
}
