//! Where releases come from, and the system `curl` that fetches them (no HTTP crate, 032 FR-003).
use crate::env::Env;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Default repository slug.
pub const DEFAULT_SLUG: &str = "lucasvmigotto/ai-gent";

/// A release source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseSource {
    /// `AI_GENT_RELEASE_BASE`: `<base>/LATEST`, `<base>/<ver>/...` (https or file).
    Base(String),
    /// GitHub releases of `owner/repo`.
    Github(String),
}

impl ReleaseSource {
    /// From `AI_GENT_RELEASE_BASE`, else `AI_GENT_UPDATE_REPO`, else the default slug.
    #[must_use]
    pub fn from_env(env: &Env) -> Self {
        if let Some(b) = env.text("AI_GENT_RELEASE_BASE") {
            return Self::Base(b.trim_end_matches('/').to_string());
        }
        Self::Github(
            env.text("AI_GENT_UPDATE_REPO")
                .unwrap_or_else(|| DEFAULT_SLUG.to_string()),
        )
    }

    /// The base `install.sh` is given as `AI_GENT_RELEASE_BASE`, so it reads the same source.
    #[must_use]
    pub fn download_base(&self) -> String {
        match self {
            Self::Base(b) => b.clone(),
            Self::Github(slug) => format!("https://github.com/{slug}/releases/download"),
        }
    }

    /// URL of one asset of release `ver`.
    #[must_use]
    pub fn asset_url(&self, ver: &str, name: &str) -> String {
        format!("{}/{ver}/{name}", self.download_base())
    }

    /// The newest release, `X.Y.Z`. Blocks for at most `max_secs`; call only from a worker.
    ///
    /// # Errors
    /// When `curl` is missing, the request fails or times out, or the answer is not a version.
    pub fn latest(&self, max_secs: u64) -> Result<String, String> {
        let mut cmd = Command::new("curl");
        match self {
            Self::Base(b) => {
                cmd.args(["-fsS", "--proto", "=https,file"]);
                cmd.args(["--max-time", &max_secs.to_string()]);
                cmd.arg(format!("{b}/LATEST"));
            }
            Self::Github(slug) => {
                // The redirect target of releases/latest ends in the tag: no JSON, no API rate limit.
                cmd.args([
                    "-sSL",
                    "--proto",
                    "=https",
                    "--max-time",
                    &max_secs.to_string(),
                ]);
                cmd.args(["-o", null_device(), "-w", "%{url_effective}"]);
                cmd.arg(format!("https://github.com/{slug}/releases/latest"));
            }
        }
        let out = run_bounded(&mut cmd, Duration::from_secs(max_secs + 1))?;
        let text = String::from_utf8_lossy(&out).trim().to_string();
        let tag = text.rsplit('/').next().unwrap_or("");
        let tag = tag.strip_prefix('v').unwrap_or(tag);
        if super::version::is_version(tag) {
            Ok(tag.to_string())
        } else {
            Err(format!("not a release version: {tag:?}"))
        }
    }
}

fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

/// Run `cmd` with stdin and stderr closed, return its stdout, kill it after `limit`.
fn run_bounded(cmd: &mut Command, limit: Duration) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut child = cmd
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    let mut stdout = child.stdout.take().ok_or("no stdout")?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() >= limit => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("timed out".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let buf = reader.join().map_err(|_| "reader panicked".to_string())?;
    if status.success() {
        Ok(buf)
    } else {
        Err(format!("curl failed ({status})"))
    }
}

/// Download `url` to `dest` with `curl`, protocols https and file only.
///
/// # Errors
/// When `curl` is missing or fails.
pub fn download(url: &str, dest: &Path, max_secs: u64) -> Result<(), String> {
    let status = Command::new("curl")
        .args(["-fsSL", "--proto", "=https,file", "--max-time"])
        .arg(max_secs.to_string())
        .arg("-o")
        .arg(dest)
        .arg(url)
        .stdin(Stdio::null())
        .status()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("could not download {url}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_from_env() {
        let e = Env::from_pairs([("AI_GENT_RELEASE_BASE", "file:///r/")]);
        assert_eq!(
            ReleaseSource::from_env(&e),
            ReleaseSource::Base("file:///r".into())
        );
        let e = Env::from_pairs([("AI_GENT_UPDATE_REPO", "a/b")]);
        let s = ReleaseSource::from_env(&e);
        assert_eq!(
            s.asset_url("1.2.3", "install.sh"),
            "https://github.com/a/b/releases/download/1.2.3/install.sh"
        );
        assert_eq!(
            ReleaseSource::from_env(&Env::default()),
            ReleaseSource::Github(DEFAULT_SLUG.into())
        );
    }

    #[cfg(unix)]
    #[test]
    fn latest_from_a_file_base() {
        if Command::new("curl").arg("--version").output().is_err() {
            return; // no curl on this machine: the integration tests skip the same way
        }
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("LATEST"), "2.28.0\n").unwrap();
        let s = ReleaseSource::Base(format!("file://{}", d.path().display()));
        assert_eq!(s.latest(5).unwrap(), "2.28.0");
        std::fs::write(d.path().join("LATEST"), "<html>\n").unwrap();
        assert!(s.latest(5).is_err());
        std::fs::remove_file(d.path().join("LATEST")).unwrap();
        assert!(s.latest(5).is_err());
    }
}
