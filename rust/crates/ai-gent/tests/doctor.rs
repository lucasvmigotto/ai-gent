//! `ai-gent doctor` (spec 034 US4) over fixture install trees: a real package installed by the real
//! `install.sh` into a throwaway HOME, then broken one way at a time.
#![cfg(unix)]
#![allow(clippy::many_single_char_names)]
mod common;
use common::{build_releases, cmd, have};
use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Instant;

const TARGET: &str = "x86_64-unknown-linux-musl";

struct Fx {
    d: tempfile::TempDir,
    home: PathBuf,
}

impl Fx {
    fn h(&self) -> PathBuf {
        self.d.path().join("h")
    }
    fn exe(&self) -> PathBuf {
        self.home.join("9.9.0/bin/ai-gent")
    }
    fn doctor(&self, args: &[&str]) -> std::process::Output {
        let h = self.h();
        let path = format!(
            "{}:{}",
            h.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut c = cmd(&self.exe(), &self.d.path().join("state"));
        c.env("HOME", &h)
            .env("PATH", path)
            .env("CLAUDE_SKILLS_DIR", h.join("claude"))
            .env("OPENCODE_SKILLS_DIR", h.join("opencode"))
            .env("AGENTS_SKILLS_DIR", h.join("agents"))
            .env("KIRO_SKILLS_DIR", h.join("kiro"))
            .env("WINDSURF_SKILLS_DIR", h.join("windsurf"))
            .env("ANTIGRAVITY_SKILLS_DIR", h.join("antigravity"))
            .env("AI_GENT_BIN_DIR", h.join("bin"))
            .arg("doctor")
            .args(args);
        c.output().unwrap()
    }
    fn report(&self, args: &[&str]) -> (i32, Value) {
        let mut a = vec!["--json"];
        a.extend_from_slice(args);
        let o = self.doctor(&a);
        let v: Value = serde_json::from_slice(&o.stdout)
            .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&o.stdout)));
        (o.status.code().unwrap(), v)
    }
}

fn status(v: &Value, id: &str) -> String {
    v["data"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no check {id}"))["status"]
        .as_str()
        .unwrap()
        .to_string()
}

fn detail(v: &Value, id: &str) -> String {
    v["data"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap()["detail"]
        .as_str()
        .unwrap()
        .to_string()
}

fn fixture() -> Option<Fx> {
    if !have("python3") || !have("tar") {
        return None;
    }
    let d = tempfile::tempdir().unwrap();
    let p = d.path().canonicalize().unwrap();
    let base = p.join("base");
    build_releases(&base, &["9.9.0"], TARGET, &common::cargo_bin());
    let home = p.join("home");
    let h = p.join("h");
    // Built directly rather than by install.sh: its setup.sh needs bash 4, and macOS ships 3.2.
    let ver = home.join("9.9.0");
    std::fs::create_dir_all(&ver).unwrap();
    let archive = base.join(format!("9.9.0/ai-gent-9.9.0-{TARGET}.tar.gz"));
    let o = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(&ver)
        .args(["--strip-components=1"])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    common::put_binary(&ver.join("bin/ai-gent"));
    std::os::unix::fs::symlink("9.9.0", home.join("current")).unwrap();
    // what setup.sh links: the binary into the bin dir, each skill into the agent's directory
    std::fs::create_dir_all(h.join("bin")).unwrap();
    std::os::unix::fs::symlink(home.join("current/bin/ai-gent"), h.join("bin/ai-gent")).unwrap();
    std::fs::create_dir_all(h.join("claude")).unwrap();
    for plugin in ["git", "db"] {
        for e in std::fs::read_dir(ver.join("plugins").join(plugin).join("skills"))
            .unwrap()
            .flatten()
        {
            let name = format!("{plugin}-{}", e.file_name().to_string_lossy());
            let target = home
                .join("current/plugins")
                .join(plugin)
                .join("skills")
                .join(e.file_name());
            std::os::unix::fs::symlink(target, h.join("claude").join(name)).unwrap();
        }
    }
    Some(Fx {
        d: tempfile::Builder::new().tempdir().map(|_| d).unwrap(),
        home,
    })
}

#[test]
fn healthy_install_is_all_ok_and_fast() {
    let Some(f) = fixture() else { return };
    let t0 = Instant::now();
    let (code, v) = f.report(&["--strict"]);
    let took = t0.elapsed();
    eprintln!("doctor took {took:?}");
    // 2 s is the contract for a release build; the test profile is unoptimized
    let limit = if cfg!(debug_assertions) { 10.0 } else { 2.0 };
    assert!(took.as_secs_f32() < limit, "{took:?}");
    let s = &v["data"]["summary"];
    assert_eq!(
        (s["fail"].as_u64(), s["warn"].as_u64()),
        (Some(0), Some(0)),
        "{v}"
    );
    assert_eq!(code, 0);
    assert_eq!(v["schema"], 1);
    assert_eq!(v["data"]["checks"].as_array().unwrap().len(), 13);
    for id in [
        "binary",
        "install_root",
        "install_integrity",
        "path_link",
        "agent_links",
        "hooks",
        "guard_selftest",
        "tools",
        "environment",
    ] {
        assert_eq!(status(&v, id), "ok", "{id}: {}", detail(&v, id));
    }
    assert_eq!(
        status(&v, "version_match"),
        "skip",
        "a dev-profile binary has release `dev`"
    );
    // text output: one line per check and a summary
    let text = String::from_utf8(f.doctor(&[]).stdout).unwrap();
    assert!(
        text.lines().count() >= 14 && text.contains("guard_selftest"),
        "{text}"
    );
}

#[test]
fn fast_skips_the_manifest() {
    let Some(f) = fixture() else { return };
    let (_, v) = f.report(&["--fast"]);
    assert_eq!(status(&v, "install_integrity"), "skip");
}

#[test]
fn a_tampered_file_fails_integrity_naming_it() {
    let Some(f) = fixture() else { return };
    let file = f.home.join("9.9.0/plugins/git/hooks/guard.sh");
    std::fs::write(&file, "#!/bin/sh\nexit 0\n").unwrap();
    let (code, v) = f.report(&[]);
    assert_eq!(code, 1);
    assert_eq!(status(&v, "install_integrity"), "fail");
    assert!(detail(&v, "install_integrity").contains("plugins/git/hooks/guard.sh (differs)"));
    assert_eq!(
        status(&v, "hooks"),
        "ok",
        "the file exists, only its content changed"
    );
}

#[test]
fn a_missing_hook_target_fails_hooks() {
    let Some(f) = fixture() else { return };
    std::fs::remove_file(f.home.join("9.9.0/plugins/git/hooks/guard.sh")).unwrap();
    let (code, v) = f.report(&["--fast"]);
    assert_eq!(code, 1);
    assert_eq!(status(&v, "hooks"), "fail");
    assert!(
        detail(&v, "hooks").contains("git: hooks/guard.sh"),
        "{}",
        detail(&v, "hooks")
    );
}

#[test]
fn a_dangling_current_fails_install_root() {
    let Some(f) = fixture() else { return };
    std::fs::remove_file(f.home.join("current")).unwrap();
    std::os::unix::fs::symlink("9.9.7", f.home.join("current")).unwrap();
    let (code, v) = f.report(&[]);
    assert_eq!(code, 1);
    assert_eq!(status(&v, "install_root"), "fail");
    assert!(detail(&v, "install_root").contains("9.9.7"));
}

#[test]
fn dangling_previous_only_warns() {
    let Some(f) = fixture() else { return };
    std::os::unix::fs::symlink("9.9.7", f.home.join("previous")).unwrap();
    let (code, v) = f.report(&[]);
    assert_eq!(code, 0);
    assert_eq!(status(&v, "install_root"), "warn");
    assert_eq!(f.doctor(&["--strict"]).status.code(), Some(1));
}

#[test]
fn links_into_an_old_clone_warn_and_dangling_ones_fail() {
    let Some(f) = fixture() else { return };
    let clone = f.d.path().join("old-clone/plugins/git/skills/workflow");
    std::fs::create_dir_all(&clone).unwrap();
    let link = f.h().join("claude/old-skill");
    std::os::unix::fs::symlink(&clone, &link).unwrap();
    let (code, v) = f.report(&["--fast"]);
    assert_eq!(code, 0);
    assert_eq!(status(&v, "agent_links"), "warn");
    let hint = v["data"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "agent_links")
        .unwrap()["hint"]
        .clone();
    assert_eq!(hint, "setup.sh --migrate");
    std::fs::remove_dir_all(f.d.path().join("old-clone")).unwrap();
    let (code, v) = f.report(&["--fast"]);
    assert_eq!(code, 1);
    assert_eq!(status(&v, "agent_links"), "fail");
}

#[test]
fn stale_variables_warn() {
    let Some(f) = fixture() else { return };
    let h = f.h();
    let mut c = cmd(&f.exe(), &f.d.path().join("state"));
    c.env("HOME", &h)
        .env("AI_GENT_MODE", "git")
        .env("AI_GENT_BRANCH", "main")
        .args(["doctor", "--json", "--fast"]);
    let o = c.output().unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(status(&v, "environment"), "warn");
    assert!(detail(&v, "environment").contains("AI_GENT_MODE, AI_GENT_BRANCH"));
}

// FR-010: modes only. A file with a marker in it and mode 0644 is reported by name, never read out;
// one with mode 000 (unreadable content) still gets a result.
#[test]
fn permissions_look_at_modes_and_never_content() {
    let Some(f) = fixture() else { return };
    let cfg = f.d.path().join("cfg/ai-gent");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::set_permissions(&cfg, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(cfg.join("unreadable.toml"), "SECRET-MARKER-1").unwrap();
    std::fs::set_permissions(
        cfg.join("unreadable.toml"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    let run = |f: &Fx| {
        let mut c = cmd(&f.exe(), &f.d.path().join("state"));
        c.env("HOME", f.h())
            .env("XDG_CONFIG_HOME", f.d.path().join("cfg"))
            .args(["doctor", "--json", "--fast"]);
        c.output().unwrap()
    };
    let o = run(&f);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(status(&v, "permissions"), "ok");
    std::fs::write(cfg.join("loose.toml"), "SECRET-MARKER-2").unwrap();
    std::fs::set_permissions(
        cfg.join("loose.toml"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    let o = run(&f);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(status(&v, "permissions"), "warn");
    assert!(detail(&v, "permissions").contains("loose.toml is 644, want 600"));
    let all = String::from_utf8_lossy(&o.stdout);
    assert!(!all.contains("SECRET-MARKER"), "no content in the output");
}

#[test]
fn update_cache_reports_cached_state_only() {
    let Some(f) = fixture() else { return };
    common::write_cache(
        &f.d.path().join("state"),
        common::now() - 2 * 86_400,
        "9.9.5",
    );
    let (_, v) = f.report(&["--fast"]);
    let d = detail(&v, "update_cache");
    assert!(
        d.contains("latest 9.9.5 cached 2 day(s) ago") && d.contains("ai-gent update"),
        "{d}"
    );
    let _ = Path::new("");
}

#[test]
fn a_development_build_without_a_root_fails_like_the_throwaway_did() {
    let d = tempfile::tempdir().unwrap();
    let st = tempfile::tempdir().unwrap();
    let o = cmd(&common::cargo_bin(), st.path())
        .env("AI_GENT_HOME", d.path())
        .args(["doctor", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(status(&v, "install_root"), "fail");
    assert_eq!(status(&v, "version_match"), "skip");
    assert_eq!(status(&v, "binary"), "ok");
}
