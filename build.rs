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

    // Cargo rerun logic.
    //
    // `.git/HEAD` only changes when the active ref changes (branch switch or
    // detached HEAD), NOT when a new commit lands on the current branch. If we
    // only watch `.git/HEAD`, GIT_COMMIT and BUILD_TIMESTAMP go stale after
    // every same-branch commit (the binary keeps reporting an old commit via
    // `crates-docs version`). Also watch the file backing the current ref
    // (e.g. `.git/refs/heads/main`) and `.git/packed-refs` (where refs may be
    // packed) so the build metadata is refreshed whenever HEAD advances.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/packed-refs");
    if let Ok(head) = std::fs::read_to_string(".git/HEAD") {
        if let Some(reference) = head.strip_prefix("ref:") {
            let reference = reference.trim();
            if !reference.is_empty() {
                println!("cargo:rerun-if-changed=.git/{reference}");
            }
        }
    }
}
