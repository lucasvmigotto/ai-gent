//! `update`, `update --rollback`, `update --list`: all go through the release's own installer.
//!
//! There is no second implementation of download, unpack or link flipping here (D16, FR-006): the
//! target release's `install.sh` (`install.ps1` on Windows) is fetched, checked against the same
//! release's `SHA256SUMS`, and run. The installer unpacks into a new version directory and flips
//! `current`, so nothing in the running version changes.
use super::{source, version};
use crate::env::Env;
use crate::exit::Exit;
use crate::guard::shadow::sha256_hex;
use crate::install::InstallRoot;
use crate::output::ErrorCode;
use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Oldest release `update --to` accepts: the first with `SHA256SUMS` entries for `install.sh` and
/// the per-target archives.
pub const OLDEST_TARGET: &str = "2.26.0";

/// A failed or declined operation, with the exit code and envelope fields it maps to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// Process exit.
    pub exit: Exit,
    /// Envelope error class.
    pub code: ErrorCode,
    /// What happened.
    pub message: String,
    /// Next step.
    pub hint: Option<String>,
}

impl Failure {
    fn new(exit: Exit, code: ErrorCode, message: impl Into<String>, hint: Option<&str>) -> Self {
        Self {
            exit,
            code,
            message: message.into(),
            hint: hint.map(str::to_string),
        }
    }
    /// A usage error (exit 2).
    #[must_use]
    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(Exit::Usage, ErrorCode::Usage, message, None)
    }
    /// Nothing to do (exit 3), e.g. the person declined.
    #[must_use]
    pub fn nothing_to_do(message: impl Into<String>) -> Self {
        Self::nothing(message, None)
    }
    fn runtime(message: impl Into<String>) -> Self {
        Self::new(Exit::Runtime, ErrorCode::Runtime, message, None)
    }
    fn refused(message: impl Into<String>) -> Self {
        Self::new(Exit::Refused, ErrorCode::Refused, message, None)
    }
    /// Nothing to do (exit 3). Its envelope class is `runtime`: the contract has no other.
    fn nothing(message: impl Into<String>, hint: Option<&str>) -> Self {
        Self::new(Exit::NothingToDo, ErrorCode::Runtime, message, hint)
    }
}

/// What `update` will do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Installed version.
    pub from: String,
    /// Target version (`latest` already resolved).
    pub to: String,
    /// Install root.
    pub home: PathBuf,
    /// An explicit version pins; `latest` clears the pin.
    pub pin_after: bool,
    /// Where the installer comes from.
    pub installer_url: String,
    /// The checksum list it is verified against.
    pub sums_url: String,
    /// Installer asset name.
    pub installer_name: &'static str,
}

/// `data` of a successful `update` or `--rollback`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Applied {
    /// Version before.
    pub from: String,
    /// Version after.
    pub to: String,
    /// Install root.
    pub home: String,
    /// Whether the install is pinned now.
    pub pinned: bool,
    /// What was verified.
    pub verified: Verified,
}

/// Verification that happened.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Verified {
    /// Installer and archive checksums matched `SHA256SUMS`.
    pub checksum: bool,
    /// `gh attestation verify` ran and passed.
    pub attestation: bool,
}

fn installer_name() -> &'static str {
    if cfg!(windows) {
        "install.ps1"
    } else {
        "install.sh"
    }
}

/// Whether an executable `name` is on `PATH` (`.exe` and `.cmd` tried on Windows).
#[must_use]
pub fn on_path(env: &Env, name: &str) -> bool {
    let Some(path) = env.path("PATH") else {
        return false;
    };
    let exts: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    std::env::split_paths(&path).any(|dir| {
        exts.iter().any(|e| {
            let f = dir.join(format!("{name}{e}"));
            f.is_file() && is_executable(&f)
        })
    })
}

/// Whether the file has an execute bit (always true off Unix).
#[must_use]
pub fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        p.exists()
    }
}

/// Resolve what an `update` would do.
///
/// # Errors
/// A [`Failure`] carrying exit 3 (development build, no shell, already on the target), 4 (a version
/// below [`OLDEST_TARGET`]), 2 (not a version) or 1 (the latest release cannot be resolved).
pub fn plan(
    root: &InstallRoot,
    env: &Env,
    src: &source::ReleaseSource,
    to: Option<&str>,
) -> Result<Plan, Failure> {
    let (Some(from), Some(home)) = (root.version.clone(), root.home.clone()) else {
        return Err(Failure::nothing(
            "this is a development build, not an install; update does not apply",
            Some("install a release: sh install.sh"),
        ));
    };
    let runner = if cfg!(windows) { "powershell" } else { "sh" };
    if !on_path(env, runner) {
        return Err(Failure::nothing(
            format!("`{runner}` is not on PATH, and update runs the release's installer with it"),
            None,
        ));
    }
    let explicit = match to {
        None | Some("latest") => None,
        Some(v) if version::is_version(v) => Some(v.to_string()),
        Some(v) => {
            return Err(Failure::new(
                Exit::Usage,
                ErrorCode::Usage,
                format!("not a release version: {v:?} (want X.Y.Z or latest)"),
                None,
            ));
        }
    };
    let target = match &explicit {
        Some(v) => {
            if version::ver_gt(OLDEST_TARGET, v) {
                return Err(Failure::refused(format!(
                    "{v} is older than {OLDEST_TARGET}, the first release this updater can install"
                )));
            }
            v.clone()
        }
        None => src.latest(30).map_err(|e| {
            Failure::runtime(format!(
                "could not resolve the latest release ({e}); try --to X.Y.Z"
            ))
        })?,
    };
    let pin_after = explicit.is_some();
    if target == from && pin_after == root.pinned().is_some() {
        return Err(Failure::nothing(format!("already on {from}"), None));
    }
    let name = installer_name();
    Ok(Plan {
        from,
        to: target.clone(),
        home,
        pin_after,
        installer_url: src.asset_url(&target, name),
        sums_url: src.asset_url(&target, "SHA256SUMS"),
        installer_name: name,
    })
}

/// A scratch directory removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> std::io::Result<Self> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let dir =
            std::env::temp_dir().join(format!("ai-gent-update-{}-{nanos}", std::process::id()));
        let mut b = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            b.mode(0o700);
        }
        b.create(&dir)?;
        Ok(Self(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The checksum a `SHA256SUMS` text lists for `name` (`<hash>  <name>` or `<hash> *<name>`).
#[must_use]
pub fn sums_entry(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let (h, f) = (it.next()?, it.next()?);
        (f == name || f.strip_prefix('*') == Some(name)).then(|| h.to_string())
    })
}

struct InstallerRun {
    success: bool,
    attestation_ok: bool,
    refused: bool,
    tail: String,
}

/// Run an installer command, forwarding its output (to stdout, or to stderr when `json`).
fn run_installer(cmd: &mut Command, json: bool) -> Result<InstallerRun, Failure> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| Failure::runtime(format!("cannot run the installer: {e}")))?;
    let out = child
        .stdout
        .take()
        .ok_or_else(|| Failure::runtime("no installer stdout"))?;
    let err = child
        .stderr
        .take()
        .ok_or_else(|| Failure::runtime("no installer stderr"))?;
    let t_out = std::thread::spawn(move || {
        let mut attestation = false;
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            attestation |= line.contains("attestation ok");
            if json {
                eprintln!("{line}");
            } else {
                println!("{line}");
            }
        }
        attestation
    });
    let t_err = std::thread::spawn(move || {
        let mut refused = false;
        let mut tail = String::new();
        for line in BufReader::new(err).lines().map_while(Result::ok) {
            refused |= [
                "checksum mismatch",
                "attestation verification failed",
                "unsafe paths",
            ]
            .iter()
            .any(|m| line.contains(m));
            eprintln!("{line}");
            line.clone_into(&mut tail);
        }
        (refused, tail)
    });
    let status = child.wait().map_err(|e| Failure::runtime(e.to_string()))?;
    let attestation_ok = t_out.join().unwrap_or(false);
    let (refused, tail) = t_err.join().unwrap_or((false, String::new()));
    Ok(InstallerRun {
        success: status.success(),
        attestation_ok,
        refused,
        tail,
    })
}

fn installer_env(cmd: &mut Command, home: &Path) {
    // git mode would turn the installer into a clone updater
    cmd.env("AI_GENT_HOME", home)
        .env_remove("AI_GENT_MODE")
        .env_remove("AI_GENT_BRANCH");
}

fn finish(run: &InstallerRun, what: &str) -> Result<(), Failure> {
    if run.success {
        Ok(())
    } else if run.refused {
        Err(Failure::refused(format!(
            "{what} was refused by the installer; nothing was changed"
        )))
    } else {
        let why = if run.tail.is_empty() {
            String::new()
        } else {
            format!(": {}", run.tail)
        };
        Err(Failure::runtime(format!("the installer failed{why}")))
    }
}

/// Fetch the target's installer, verify it, run it.
///
/// # Errors
/// Exit 4 when the installer's checksum is missing or wrong (or the installer refuses an archive),
/// exit 1 for anything else.
pub fn apply(
    plan: &Plan,
    src: &source::ReleaseSource,
    verify_attestation: bool,
    json: bool,
) -> Result<Applied, Failure> {
    let scratch =
        Scratch::new().map_err(|e| Failure::runtime(format!("no scratch directory: {e}")))?;
    let installer = scratch.0.join(plan.installer_name);
    let sums = scratch.0.join("SHA256SUMS");
    source::download(&plan.sums_url, &sums, 60).map_err(Failure::runtime)?;
    source::download(&plan.installer_url, &installer, 60).map_err(Failure::runtime)?;
    let sums_text = std::fs::read_to_string(&sums).map_err(|e| Failure::runtime(e.to_string()))?;
    let want = sums_entry(&sums_text, plan.installer_name).ok_or_else(|| {
        Failure::refused(format!(
            "SHA256SUMS of {} lists no {}; nothing was run",
            plan.to, plan.installer_name
        ))
    })?;
    let bytes = std::fs::read(&installer).map_err(|e| Failure::runtime(e.to_string()))?;
    let got = sha256_hex(&bytes);
    if want != got {
        return Err(Failure::refused(format!(
            "checksum mismatch for {} (expected {want}, got {got}); nothing was run",
            plan.installer_name
        )));
    }
    let version_env = if plan.pin_after {
        plan.to.as_str()
    } else {
        "latest"
    };
    let mut cmd;
    if cfg!(windows) {
        cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&installer);
        cmd.args(["-Version", version_env, "-SkipPrompt"]);
        if verify_attestation {
            cmd.arg("-VerifyAttestation");
        }
    } else {
        cmd = Command::new("sh");
        cmd.arg(&installer).arg("--skip-prompt");
        if verify_attestation {
            cmd.arg("--verify-attestation");
        }
    }
    installer_env(&mut cmd, &plan.home);
    cmd.env("VERSION", version_env)
        .env("AI_GENT_RELEASE_BASE", src.download_base());
    let run = run_installer(&mut cmd, json)?;
    finish(&run, &format!("installing {}", plan.to))?;
    Ok(Applied {
        from: plan.from.clone(),
        to: plan.to.clone(),
        home: plan.home.display().to_string(),
        pinned: plan.pin_after,
        verified: Verified {
            checksum: true,
            attestation: verify_attestation && run.attestation_ok,
        },
    })
}

/// What `--rollback` will do: `(current, previous, home)`.
///
/// # Errors
/// Exit 3 for a development build or without a `previous` that exists.
pub fn rollback_plan(root: &InstallRoot) -> Result<(String, String, PathBuf), Failure> {
    let Some(home) = root.home.clone().filter(|_| !root.dev_build) else {
        return Err(Failure::nothing(
            "this is a development build, not an install",
            None,
        ));
    };
    let prev = root.previous().filter(|p| home.join(p).is_dir());
    let Some(prev) = prev else {
        return Err(Failure::nothing(
            "no previous version to roll back to",
            None,
        ));
    };
    let cur = root.current().unwrap_or_default();
    Ok((cur, prev, home))
}

/// Roll back with the installed release's own installer.
///
/// # Errors
/// As [`rollback_plan`], plus exit 1 when the installer fails.
pub fn rollback(root: &InstallRoot, env: &Env, json: bool) -> Result<Applied, Failure> {
    let (cur, prev, home) = rollback_plan(root)?;
    let mut cmd = installer_command(&home, "--rollback", env)?;
    installer_env(&mut cmd, &home);
    let run = run_installer(&mut cmd, json)?;
    finish(&run, "the rollback")?;
    Ok(Applied {
        from: cur,
        to: prev,
        home: home.display().to_string(),
        pinned: InstallRoot {
            home: Some(home.clone()),
            ..root.clone()
        }
        .pinned()
        .is_some(),
        verified: Verified {
            checksum: false,
            attestation: false,
        },
    })
}

/// `sh <home>/current/install.sh <flag> --skip-prompt` (or the PowerShell form).
fn installer_command(home: &Path, flag: &str, env: &Env) -> Result<Command, Failure> {
    let script = home.join("current").join(installer_name());
    if !script.is_file() {
        return Err(Failure::runtime(format!(
            "{} is missing; reinstall with: sh install.sh",
            script.display()
        )));
    }
    let runner = if cfg!(windows) { "powershell" } else { "sh" };
    if !on_path(env, runner) {
        return Err(Failure::nothing(format!("`{runner}` is not on PATH"), None));
    }
    let mut cmd = Command::new(runner);
    if cfg!(windows) {
        cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&script);
        cmd.arg(match flag {
            "--rollback" => "-Rollback",
            _ => "-List",
        });
        if flag == "--rollback" {
            cmd.arg("-SkipPrompt");
        }
    } else {
        cmd.arg(&script).arg(flag);
        if flag == "--rollback" {
            cmd.arg("--skip-prompt");
        }
    }
    Ok(cmd)
}

/// One row of `--list`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionRow {
    /// `X.Y.Z`.
    pub version: String,
    /// The `current` link names it.
    pub current: bool,
    /// The `previous` link names it.
    pub previous: bool,
}

/// `data` of `update --list --json`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListReport {
    /// Unpacked versions, oldest first.
    pub versions: Vec<VersionRow>,
    /// The pin, if any.
    pub pinned: Option<String>,
}

/// The tree as `--list` reports it (read straight from the install root).
#[must_use]
pub fn list(root: &InstallRoot) -> ListReport {
    let (cur, prev) = (root.current(), root.previous());
    ListReport {
        versions: root
            .versions()
            .into_iter()
            .map(|v| VersionRow {
                current: cur.as_deref() == Some(v.as_str()),
                previous: prev.as_deref() == Some(v.as_str()),
                version: v,
            })
            .collect(),
        pinned: root.pinned().filter(|p| !p.is_empty()),
    }
}

/// Text `--list`: the installed release's own `install.sh --list`, when it exists.
///
/// # Errors
/// Exit 1 when the installer fails.
pub fn list_with_installer(root: &InstallRoot, env: &Env) -> Result<bool, Failure> {
    let Some(home) = root.home.clone().filter(|_| !root.dev_build) else {
        return Ok(false);
    };
    let Ok(mut cmd) = installer_command(&home, "--list", env) else {
        return Ok(false);
    };
    installer_env(&mut cmd, &home);
    let run = run_installer(&mut cmd, false)?;
    finish(&run, "the listing")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_lookup() {
        let s = "aaa  ai-gent-9.9.0.tar.gz\nbbb *install.sh\n";
        assert_eq!(sums_entry(s, "install.sh").as_deref(), Some("bbb"));
        assert_eq!(
            sums_entry(s, "ai-gent-9.9.0.tar.gz").as_deref(),
            Some("aaa")
        );
        assert_eq!(sums_entry(s, "nope"), None);
    }

    fn root(home: &Path, version: &str) -> InstallRoot {
        InstallRoot {
            home: Some(home.to_path_buf()),
            version_dir: Some(home.join(version)),
            version: Some(version.into()),
            dev_build: false,
        }
    }

    #[test]
    fn dev_build_and_bad_targets() {
        let d = tempfile::tempdir().unwrap();
        let env = Env::from_pairs([("PATH", "/usr/bin:/bin")]);
        let src = source::ReleaseSource::Base("file:///nowhere".into());
        let dev = InstallRoot {
            home: Some(d.path().into()),
            version_dir: None,
            version: None,
            dev_build: true,
        };
        assert_eq!(
            plan(&dev, &env, &src, None).unwrap_err().exit,
            Exit::NothingToDo
        );
        let r = root(d.path(), "2.27.0");
        if on_path(&env, "sh") {
            assert_eq!(
                plan(&r, &env, &src, Some("2.25.0")).unwrap_err().exit,
                Exit::Refused
            );
            assert_eq!(
                plan(&r, &env, &src, Some("banana")).unwrap_err().exit,
                Exit::Usage
            );
            // same version, same pin state: nothing to do
            assert_eq!(
                plan(&r, &env, &src, Some("latest"))
                    .map(|_| ())
                    .unwrap_err()
                    .exit,
                Exit::Runtime
            );
        }
    }

    #[test]
    fn already_on_target_and_pin_changes() {
        let d = tempfile::tempdir().unwrap();
        let env = Env::from_pairs([("PATH", "/usr/bin:/bin")]);
        if !on_path(&env, "sh") {
            return;
        }
        let r = root(d.path(), "2.27.0");
        let src = source::ReleaseSource::Base("file:///nowhere".into());
        assert!(
            plan(&r, &env, &src, Some("2.27.0")).unwrap().pin_after,
            "pins the running version"
        );
        std::fs::write(d.path().join("pinned"), "2.27.0\n").unwrap();
        assert_eq!(
            plan(&r, &env, &src, Some("2.27.0")).unwrap_err().exit,
            Exit::NothingToDo
        );
        assert_eq!(plan(&r, &env, &src, Some("2.30.0")).unwrap().to, "2.30.0");
    }

    #[test]
    fn rollback_needs_a_previous() {
        let d = tempfile::tempdir().unwrap();
        let r = root(d.path(), "2.27.0");
        assert_eq!(rollback_plan(&r).unwrap_err().exit, Exit::NothingToDo);
    }

    #[cfg(unix)]
    #[test]
    fn list_marks_current_and_previous() {
        let d = tempfile::tempdir().unwrap();
        for v in ["2.26.0", "2.27.0", "2.28.0"] {
            std::fs::create_dir(d.path().join(v)).unwrap();
        }
        std::os::unix::fs::symlink("2.28.0", d.path().join("current")).unwrap();
        std::os::unix::fs::symlink("2.27.0", d.path().join("previous")).unwrap();
        let l = list(&root(d.path(), "2.28.0"));
        assert_eq!(
            serde_json::to_string(&l).unwrap(),
            r#"{"versions":[{"version":"2.26.0","current":false,"previous":false},{"version":"2.27.0","current":false,"previous":true},{"version":"2.28.0","current":true,"previous":false}],"pinned":null}"#
        );
    }
}
