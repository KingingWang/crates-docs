use std::process::Command;

fn main() {
    // 设置构建时间戳
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", chrono::Utc::now().to_rfc3339());
    
    // 获取 Git 提交信息
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        && output.status.success()
    {
        let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("cargo:rustc-env=GIT_COMMIT={}", commit);
    }
    
    // 获取 Rust 版本
    if let Ok(output) = Command::new("rustc")
        .args(["--version"])
        .output()
        && output.status.success()
    {
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("cargo:rustc-env=RUST_VERSION={}", version);
    }
    
    // 如果没有获取到 Git 信息，设置默认值
    if std::env::var("GIT_COMMIT").is_err() {
        println!("cargo:rustc-env=GIT_COMMIT=unknown");
    }
    
    if std::env::var("RUST_VERSION").is_err() {
        println!("cargo:rustc-env=RUST_VERSION=unknown");
    }
}