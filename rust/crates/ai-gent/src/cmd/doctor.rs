//! `ai-gent doctor` (throwaway in 2.x; spec 034 replaces it).
use std::path::Path;

use ai_gent_core::{exit::Exit, output, paths, version::VersionInfo};
use serde::Serialize;

/// Status of one check.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Passed.
    Ok,
    /// Passed with a remark (part of the contract; no check warns yet).
    #[allow(dead_code)]
    Warn,
    /// Failed.
    Fail,
    /// Not applicable.
    Skip,
}

/// One check result.
#[derive(Debug, Serialize)]
pub struct Check {
    /// Stable id.
    pub id: &'static str,
    /// Result.
    pub status: Status,
    /// Human detail.
    pub detail: String,
}

#[derive(Serialize)]
struct Report {
    checks: Vec<Check>,
}

fn check(id: &'static str, status: Status, detail: impl Into<String>) -> Check {
    Check {
        id,
        status,
        detail: detail.into(),
    }
}

fn binary_check() -> Check {
    match std::env::current_exe() {
        Ok(p) if p.is_file() => check("binary", Status::Ok, p.display().to_string()),
        Ok(p) => check(
            "binary",
            Status::Fail,
            format!("{} is not a file", p.display()),
        ),
        Err(e) => check("binary", Status::Fail, e.to_string()),
    }
}

fn os_check() -> Check {
    let os = std::env::consts::OS;
    if matches!(os, "linux" | "macos" | "windows") {
        check("os", Status::Ok, format!("{os} {}", std::env::consts::ARCH))
    } else {
        check("os", Status::Fail, format!("unsupported OS {os}"))
    }
}

fn install_root_check(root: Option<&Path>) -> Check {
    match root {
        None => check(
            "install_root",
            Status::Fail,
            "no install root (AI_GENT_HOME, HOME or LOCALAPPDATA unset)",
        ),
        Some(r) if r.join("current").exists() => {
            check("install_root", Status::Ok, r.display().to_string())
        }
        Some(r) => check(
            "install_root",
            Status::Fail,
            format!("{} has no `current`", r.display()),
        ),
    }
}

fn version_match_check(root: Option<&Path>, release: &str) -> Check {
    let Some(file) = root
        .map(|r| r.join("current").join("VERSION"))
        .filter(|f| f.is_file())
    else {
        return check("version_match", Status::Skip, "not under an install root");
    };
    if release == "dev" {
        return check("version_match", Status::Skip, "development build");
    }
    match std::fs::read_to_string(&file) {
        Ok(v) if v.trim() == release => check("version_match", Status::Ok, release),
        Ok(v) => check(
            "version_match",
            Status::Fail,
            format!("binary {release}, install {}", v.trim()),
        ),
        Err(e) => check("version_match", Status::Fail, e.to_string()),
    }
}

/// Run all checks against `root`.
fn collect(root: Option<&Path>, release: &str) -> Vec<Check> {
    vec![
        binary_check(),
        os_check(),
        install_root_check(root),
        version_match_check(root, release),
    ]
}

/// Run `doctor`.
pub fn run(json: bool) -> Exit {
    let root = paths::install_root();
    let info = VersionInfo::current(env!("CARGO_PKG_VERSION"));
    let checks = collect(root.as_deref(), &info.release);
    let failed = checks.iter().any(|c| c.status == Status::Fail);
    if json {
        println!("{}", output::ok_json(1, Report { checks }));
    } else {
        for c in &checks {
            println!("{:<14} {:?}  {}", c.id, c.status, c.detail);
        }
    }
    if failed { Exit::Runtime } else { Exit::Success }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_root_fails() {
        assert_eq!(install_root_check(None).status, Status::Fail);
    }

    #[test]
    fn version_mismatch_fails() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("current")).unwrap();
        std::fs::write(d.path().join("current/VERSION"), "2.0.0\n").unwrap();
        assert_eq!(
            version_match_check(Some(d.path()), "2.1.0").status,
            Status::Fail
        );
        assert_eq!(
            version_match_check(Some(d.path()), "2.0.0").status,
            Status::Ok
        );
        assert_eq!(
            version_match_check(Some(d.path()), "dev").status,
            Status::Skip
        );
    }
}
