//! Fixtures shared by the update and doctor tests: install trees around a copy of the real binary.
#![allow(dead_code)]
use assert_cmd::Command;
use std::path::{Path, PathBuf};

/// The built binary.
pub fn cargo_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin("ai-gent")
}

/// A command with a clean `AI_GENT_*` environment and `state` as the state directory.
pub fn cmd(exe: &Path, state: &Path) -> Command {
    let mut c = Command::new(exe);
    for k in [
        "AI_GENT_HOME",
        "AI_GENT_NO_UPDATE_CHECK",
        "AI_GENT_UPDATE_DAYS",
        "AI_GENT_UPDATE_REPO",
        "AI_GENT_RELEASE_BASE",
        "AI_GENT_MODE",
        "AI_GENT_BRANCH",
        "AI_GENT_DIR",
        "AI_GENT_BIN_DIR",
        "XDG_CONFIG_HOME",
    ] {
        c.env_remove(k);
    }
    c.env("XDG_STATE_HOME", state)
        .env("LOCALAPPDATA", state)
        .env("NO_COLOR", "1");
    c
}

/// Lay out `<home>/<ver>/{bin/ai-gent,VERSION}` for each version (a copy of the real binary), point
/// `current` at the last and `previous` at the one before. Returns the path of `current`'s binary,
/// reached through the `current` link as `~/.local/bin/ai-gent` reaches it.
#[cfg(unix)]
pub fn install_tree(home: &Path, versions: &[&str]) -> PathBuf {
    for v in versions {
        let dir = home.join(v);
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join("VERSION"), format!("{v}\n")).unwrap();
        put_binary(&dir.join("bin/ai-gent"));
    }
    let n = versions.len();
    std::os::unix::fs::symlink(versions[n - 1], home.join("current")).unwrap();
    if n > 1 {
        std::os::unix::fs::symlink(versions[n - 2], home.join("previous")).unwrap();
    }
    home.join("current/bin/ai-gent")
}

/// Write the update cache the way `scripts/update-check.sh` does.
pub fn write_cache(state: &Path, epoch: u64, tag: &str) {
    let dir = state.join("ai-gent");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("update-check"), format!("{epoch} {tag}\n")).unwrap();
}

/// Seconds since the epoch.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Whether `tool` runs on this machine.
pub fn have(tool: &str) -> bool {
    std::process::Command::new(tool)
        .arg("--version")
        .output()
        .is_ok()
}

/// The repository root.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap()
}

/// Build releases `versions` under `base/<ver>/` with `scripts/package.py`, each with the
/// per-target archive holding `binary`, and write `LATEST`.
pub fn build_releases(base: &Path, versions: &[&str], target: &str, binary: &Path) {
    for v in versions {
        let out = base.join(v);
        std::fs::create_dir_all(&out).unwrap();
        for extra in [
            vec![],
            vec!["--target", target, "--binary", binary.to_str().unwrap()],
        ] {
            let o = std::process::Command::new("python3")
                .arg(repo().join("scripts/package.py"))
                .args(["--version", v, "--out"])
                .arg(&out)
                .args(&extra)
                .output()
                .unwrap();
            assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        }
    }
    std::fs::write(
        base.join("LATEST"),
        format!("{}\n", versions[versions.len() - 1]),
    )
    .unwrap();
}

/// A private copy of the binary, made once per test process and proven runnable. Copying the
/// binary per test would race with other threads' forks (ETXTBSY: "text file busy"), so tests
/// hard-link this one instead; nothing holds it open for writing after the first run succeeds.
pub fn template_binary() -> &'static Path {
    static T: std::sync::OnceLock<(tempfile::TempDir, PathBuf)> = std::sync::OnceLock::new();
    &T.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        let t = d.path().join("ai-gent");
        std::fs::copy(cargo_bin(), &t).unwrap();
        for _ in 0..200 {
            match std::process::Command::new(&t).arg("--version").output() {
                Ok(_) => break,
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(25)),
            }
        }
        (d, t)
    })
    .1
}

/// Put the binary at `dest` (replacing what is there): a hard link, else a copy.
pub fn put_binary(dest: &Path) {
    let _ = std::fs::remove_file(dest);
    if std::fs::hard_link(template_binary(), dest).is_err() {
        std::fs::copy(template_binary(), dest).unwrap();
    }
}
