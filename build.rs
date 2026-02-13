use std::process::Command;

fn main() {
    // 设置构建时间戳
    println!(
        "cargo:rustc-env=BUILD_TIMESTAMP={}",
        chrono::Utc::now().to_rfc3339()
    );

    // 获取 Git 提交信息
    let mut commit = String::from("unknown");
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        && output.status.success() {
            commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    println!("cargo:rustc-env=GIT_COMMIT={}", commit);

    // 获取 Rust 版本
    let mut version = String::from("unknown");
    if let Ok(output) = Command::new("rustc").args(["--version"]).output()
        && output.status.success() {
            version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    println!("cargo:rustc-env=RUST_VERSION={}", version);

    // 重新 cargo rerun 逻辑
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
}
