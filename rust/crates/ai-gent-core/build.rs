//! Emits `AI_GENT_TARGET` (the target triple) and `AI_GENT_COMMIT` (short commit, when available).
use std::process::Command;

fn main() {
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".into());
    println!("cargo:rustc-env=AI_GENT_TARGET={target}");

    let commit = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    println!("cargo:rustc-env=AI_GENT_COMMIT={commit}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=AI_GENT_RELEASE");
}
