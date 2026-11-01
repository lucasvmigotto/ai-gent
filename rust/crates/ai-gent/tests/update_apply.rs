//! `ai-gent update`, `--rollback`, `--list` (spec 034 US2, US3) against a `file://` release base
//! built by `scripts/package.py`, with the real `install.sh` as the installer.
//!
//! The archives hold a stub `ai-gent`; the binary that runs is the real one, copied into the first
//! version's directory (`scripts/test-update.sh` runs the whole chain with the real binary inside
//! the packages too).
#![cfg(unix)]
mod common;
use common::{cmd, have};
use std::path::{Path, PathBuf};

const TARGET: &str = "x86_64-unknown-linux-musl";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap()
}

struct Fx {
    d: tempfile::TempDir,
    home: PathBuf,
    base: PathBuf,
    exe: PathBuf,
    bash: PathBuf,
}

impl Fx {
    fn state(&self) -> PathBuf {
        self.d.path().join("state")
    }
    /// `exe` as a command inside the sandbox.
    fn command(&self, exe: &Path) -> assert_cmd::Command {
        let h = self.d.path().join("h");
        let mut c = cmd(exe, &self.state());
        c.env("HOME", &h)
            .env("CLAUDE_SKILLS_DIR", h.join("claude"))
            .env("OPENCODE_SKILLS_DIR", h.join("opencode"))
            .env("AGENTS_SKILLS_DIR", h.join("agents"))
            .env("AI_GENT_BIN_DIR", h.join("bin"))
            .env("AI_GENT_TARGET", TARGET)
            .env("AI_GENT_BASH", &self.bash)
            .env(
                "AI_GENT_RELEASE_BASE",
                format!("file://{}", self.base.display()),
            );
        c
    }
    /// The real binary in 9.9.0.
    fn ai(&self) -> assert_cmd::Command {
        self.command(&self.exe)
    }
    /// The real binary copied into `<ver>/bin`.
    fn ai_at(&self, ver: &str) -> assert_cmd::Command {
        let exe = self.home.join(ver).join("bin/ai-gent");
        common::put_binary(&exe);
        self.command(&exe)
    }
    fn link(&self, name: &str) -> Option<String> {
        std::fs::read_link(self.home.join(name))
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    }
    fn set_latest(&self, v: &str) {
        std::fs::write(self.base.join("LATEST"), format!("{v}\n")).unwrap();
    }
}

/// Releases 9.9.0 and 9.9.1 under a base, 9.9.0 installed with the real installer, then the real
/// binary dropped into 9.9.0's `bin/`.
fn fixture() -> Option<Fx> {
    if !have("curl") || !have("python3") || !have("tar") {
        return None;
    }
    let Some(modern) = common::modern_bash() else {
        eprintln!("skipped: no bash 4 or newer (setup.sh needs it; on macOS: brew install bash)");
        return None;
    };
    let d = tempfile::tempdir().unwrap();
    let p = d.path().canonicalize().unwrap();
    std::fs::create_dir_all(p.join("h")).unwrap();
    let stub = p.join("stub");
    std::fs::write(&stub, "#!/bin/sh\necho stub\n").unwrap();
    let base = p.join("base");
    for v in ["9.9.0", "9.9.1"] {
        let out = base.join(v);
        std::fs::create_dir_all(&out).unwrap();
        for extra in [
            vec![],
            vec!["--target", TARGET, "--binary", stub.to_str().unwrap()],
        ] {
            let st = std::process::Command::new("python3")
                .arg(repo().join("scripts/package.py"))
                .args(["--version", v, "--out"])
                .arg(&out)
                .args(&extra)
                .output()
                .unwrap();
            assert!(
                st.status.success(),
                "{}",
                String::from_utf8_lossy(&st.stderr)
            );
        }
    }
    std::fs::write(base.join("LATEST"), "9.9.0\n").unwrap();
    let home = p.join("home");
    let fx = Fx {
        d,
        home: home.clone(),
        base,
        exe: home.join("9.9.0/bin/ai-gent"),
        bash: modern.clone(),
    };
    let out = std::process::Command::new("sh")
        .arg(repo().join("install.sh"))
        .arg("--skip-prompt")
        .args(["--target", "claude"])
        .env_clear()
        .envs(std::env::vars().filter(|(k, _)| k == "PATH"))
        .env("HOME", p.join("h"))
        .env("CLAUDE_SKILLS_DIR", p.join("h/claude"))
        .env("XDG_STATE_HOME", p.join("state"))
        .env("AI_GENT_BIN_DIR", p.join("h/bin"))
        .env("AI_GENT_HOME", &home)
        .env("AI_GENT_BASH", &modern)
        .env("AI_GENT_TARGET", TARGET)
        .env(
            "AI_GENT_RELEASE_BASE",
            format!("file://{}", fx.base.display()),
        )
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(fx.link("current").as_deref(), Some("9.9.0"));
    // the running binary: the real one
    common::put_binary(&home.join("9.9.0/bin/ai-gent"));
    fx.set_latest("9.9.1");
    Some(fx)
}

fn json(out: &[u8]) -> serde_json::Value {
    serde_json::from_slice(out).unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(out)))
}

#[test]
fn update_installs_the_latest_and_rollback_restores() {
    let Some(f) = fixture() else { return };
    let out = f
        .ai()
        .args(["update", "--yes", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v = json(&out);
    assert_eq!(v["data"]["from"], "9.9.0");
    assert_eq!(v["data"]["to"], "9.9.1");
    assert_eq!(v["data"]["pinned"], false);
    assert_eq!(v["data"]["verified"]["checksum"], true);
    assert_eq!(f.link("current").as_deref(), Some("9.9.1"));
    assert_eq!(f.link("previous").as_deref(), Some("9.9.0"));
    assert!(
        f.home.join("9.9.0/VERSION").is_file(),
        "the running version was not touched"
    );
    assert!(!f.home.join("pinned").exists());

    // a second run has nothing to do (the binary now runs through `current` -> 9.9.1)
    f.ai_at("9.9.1").args(["update", "--yes"]).assert().code(3);

    // rollback
    let out = f
        .ai()
        .args(["update", "--rollback", "--yes", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v = json(&out);
    assert_eq!(
        (v["data"]["from"].as_str(), v["data"]["to"].as_str()),
        (Some("9.9.1"), Some("9.9.0"))
    );
    assert_eq!(f.link("current").as_deref(), Some("9.9.0"));
    assert_eq!(f.link("previous").as_deref(), Some("9.9.1"));
}

#[test]
fn explicit_version_pins_and_latest_unpins() {
    let Some(f) = fixture() else { return };
    f.ai()
        .args(["update", "--to", "9.9.1", "--yes"])
        .assert()
        .success();
    assert_eq!(
        std::fs::read_to_string(f.home.join("pinned")).unwrap(),
        "9.9.1\n"
    );
    // pinned: --check is silent even with a newer release cached
    common::write_cache(&f.state(), common::now(), "9.9.9");
    let out = f
        .ai()
        .args(["update", "--check"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(out.is_empty());
    // --to latest clears the pin (already on 9.9.1, but the pin state changes, so it runs)
    f.ai_at("9.9.1")
        .args(["update", "--to", "latest", "--yes"])
        .assert()
        .success();
    assert!(!f.home.join("pinned").exists());
}

#[test]
fn a_tampered_installer_is_refused_with_exit_4_and_nothing_changes() {
    let Some(f) = fixture() else { return };
    std::fs::write(
        f.base.join("9.9.1/install.sh"),
        format!("#!/bin/sh\ntouch {}/ran\n", f.base.display()),
    )
    .unwrap();
    let out = f
        .ai()
        .args(["update", "--yes", "--json"])
        .assert()
        .code(4)
        .get_output()
        .stdout
        .clone();
    let v = json(&out);
    assert_eq!(v["error"]["code"], "refused");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("checksum mismatch")
    );
    assert!(
        !f.base.join("ran").exists(),
        "the tampered installer never ran"
    );
    assert_eq!(f.link("current").as_deref(), Some("9.9.0"));
    assert!(!f.home.join("9.9.1").exists());
}

#[test]
fn a_tampered_archive_is_refused_by_the_installer_with_exit_4() {
    let Some(f) = fixture() else { return };
    let archive = f.base.join(format!("9.9.1/ai-gent-9.9.1-{TARGET}.tar.gz"));
    let mut bytes = std::fs::read(&archive).unwrap();
    let n = bytes.len() / 2;
    bytes[n] ^= 0xff;
    std::fs::write(&archive, bytes).unwrap();
    f.ai().args(["update", "--yes"]).assert().code(4);
    assert_eq!(f.link("current").as_deref(), Some("9.9.0"));
    assert!(!f.home.join("9.9.1").exists());
}

#[test]
fn nothing_to_do_and_refusals() {
    let Some(f) = fixture() else { return };
    // no --yes and no terminal: confirmation needed
    f.ai().arg("update").assert().code(2);
    f.ai().args(["update", "--json"]).assert().code(2);
    // older than the first installable release
    f.ai()
        .args(["update", "--to", "2.25.0", "--yes"])
        .assert()
        .code(4);
    f.ai()
        .args(["update", "--to", "banana", "--yes"])
        .assert()
        .code(2);
    // already on the target
    f.set_latest("9.9.0");
    f.ai().args(["update", "--yes"]).assert().code(3);
    // no previous version
    f.ai()
        .args(["update", "--rollback", "--yes"])
        .assert()
        .code(3);
    assert_eq!(f.link("current").as_deref(), Some("9.9.0"));
}

#[test]
fn a_development_build_does_not_update() {
    let d = tempfile::tempdir().unwrap();
    let st = tempfile::tempdir().unwrap();
    cmd(&common::cargo_bin(), st.path())
        .env("AI_GENT_HOME", d.path())
        .args(["update", "--yes"])
        .assert()
        .code(3);
}

#[test]
fn list_json_and_text() {
    let Some(f) = fixture() else { return };
    f.ai().args(["update", "--yes"]).assert().success();
    let out = f
        .ai()
        .args(["update", "--list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v = json(&out);
    let rows = v["data"]["versions"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["version"], "9.9.0");
    assert_eq!(rows[0]["previous"], true);
    assert_eq!(rows[1]["current"], true);
    assert!(v["data"]["pinned"].is_null());
    let text = String::from_utf8(
        f.ai()
            .args(["update", "--list"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    assert!(
        text.contains("9.9.1 (current)") && text.contains("9.9.0 (previous)"),
        "{text}"
    );
}
