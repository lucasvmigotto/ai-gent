//! `update --check`: decide the notice from the cache, start a detached refresh when due.
use super::{cache, version};
use crate::env::Env;
use crate::install::InstallRoot;
use serde::Serialize;
use std::path::Path;

/// Default days between refreshes.
pub const DEFAULT_DAYS: u64 = 7;

/// The one-line notice (FR-008).
#[must_use]
pub fn notice_line(installed: &str, latest: &str) -> String {
    format!(
        "[ai-gent] update available: {installed} -> {latest} - run: ai-gent update (propose first, never self-apply in-session)"
    )
}

/// `data` of `update --check --json`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CheckReport {
    /// Installed version, `null` for a development build.
    pub installed: Option<String>,
    /// Newest cached release.
    pub latest: Option<String>,
    /// ISO time of the last refresh.
    pub cached_at: Option<String>,
    /// The cache is older than the interval (false with no cache: there is nothing to be stale).
    pub stale: bool,
    /// A pin silences the check.
    pub pinned: bool,
    /// This call started a refresh.
    pub checking: bool,
    /// The line to print, or `null`.
    pub notice: Option<String>,
}

/// What a check decided: the report and whether a refresh is due.
#[derive(Debug)]
pub struct Decision {
    /// The report (with `checking` false; the caller sets it once the refresh was started).
    pub report: CheckReport,
    /// Start a detached refresh.
    pub refresh: bool,
}

/// Pure decision: no clock, no network, no process. `now` is epoch seconds.
#[must_use]
pub fn decide(root: &InstallRoot, env: &Env, cache_file: &Path, now: u64) -> Decision {
    let mut report = CheckReport {
        installed: root.version.clone(),
        latest: None,
        cached_at: None,
        stale: false,
        pinned: root.pinned().is_some(),
        checking: false,
        notice: None,
    };
    // The three silences: opt-out, pin, and a binary that is not under an install root.
    if env.is_set("AI_GENT_NO_UPDATE_CHECK") || report.pinned || root.dev_build {
        return Decision {
            report,
            refresh: false,
        };
    }
    let max_age = env
        .text("AI_GENT_UPDATE_DAYS")
        .and_then(|d| d.parse::<u64>().ok())
        .unwrap_or(DEFAULT_DAYS)
        .saturating_mul(86_400);
    let Some(c) = cache::read(cache_file) else {
        return Decision {
            report,
            refresh: true,
        };
    };
    report.stale = cache::is_stale(&c, now, max_age);
    report.cached_at = c.checked.map(crate::guard::shadow::iso_utc);
    if let Some(installed) = root.version.as_deref()
        && version::ver_gt(&c.latest, installed)
    {
        report.notice = Some(notice_line(installed, &c.latest));
    }
    report.latest = Some(c.latest);
    Decision {
        refresh: report.stale,
        report,
    }
}

/// Seconds since the epoch.
#[must_use]
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Start `<exe> update --refresh` detached: no stdio, its own process group (Unix) or detached
/// process (Windows), so it outlives a hook that exits. Never waits.
#[must_use]
pub fn spawn_refresh(exe: &Path) -> bool {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(exe);
    cmd.args(["update", "--refresh"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
        cmd.creation_flags(0x0000_0008 | 0x0000_0200);
    }
    cmd.spawn().is_ok()
}

/// The refresh worker: ask the source, write the cache. Silent; a failure leaves the cache alone.
#[must_use]
pub fn refresh(env: &Env, cache_file: &Path) -> bool {
    let source = super::source::ReleaseSource::from_env(env);
    match source.latest(8) {
        Ok(tag) => cache::write(cache_file, now_secs(), &tag).is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn root(version: Option<&str>, home: Option<&Path>) -> InstallRoot {
        InstallRoot {
            home: home.map(Path::to_path_buf),
            version_dir: version.map(|_| PathBuf::from("/x")),
            version: version.map(str::to_string),
            dev_build: version.is_none(),
        }
    }

    const NOW: u64 = 1_800_000_000;

    fn cache_with(d: &Path, age_days: u64, tag: &str) -> PathBuf {
        let p = d.join("update-check");
        cache::write(&p, NOW - age_days * 86_400, tag).unwrap();
        p
    }

    // US1 scenario 1
    #[test]
    fn newer_cached_release_gives_one_notice() {
        let d = tempfile::tempdir().unwrap();
        let p = cache_with(d.path(), 1, "2.28.0");
        let r = decide(
            &root(Some("2.27.0"), Some(d.path())),
            &Env::default(),
            &p,
            NOW,
        );
        assert_eq!(
            r.report.notice.as_deref(),
            Some(
                "[ai-gent] update available: 2.27.0 -> 2.28.0 - run: ai-gent update (propose first, never self-apply in-session)"
            )
        );
        assert!(!r.refresh && !r.report.stale);
    }

    // US1 scenario 2: no cache, no notice, refresh
    #[test]
    fn no_cache_refreshes_silently() {
        let d = tempfile::tempdir().unwrap();
        let r = decide(
            &root(Some("2.27.0"), Some(d.path())),
            &Env::default(),
            &d.path().join("update-check"),
            NOW,
        );
        assert!(r.refresh && r.report.notice.is_none() && r.report.latest.is_none());
    }

    // US1 scenario 3: a stale cache still notifies, and refreshes
    #[test]
    fn stale_cache_notifies_and_refreshes() {
        let d = tempfile::tempdir().unwrap();
        let p = cache_with(d.path(), 30, "2.28.0");
        let r = decide(
            &root(Some("2.27.0"), Some(d.path())),
            &Env::default(),
            &p,
            NOW,
        );
        assert!(r.refresh && r.report.stale && r.report.notice.is_some());
        // AI_GENT_UPDATE_DAYS widens the interval
        let env = Env::from_pairs([("AI_GENT_UPDATE_DAYS", "60")]);
        let r = decide(&root(Some("2.27.0"), Some(d.path())), &env, &p, NOW);
        assert!(!r.refresh && r.report.notice.is_some());
    }

    // US1 scenario 4
    #[test]
    fn the_three_silences() {
        let d = tempfile::tempdir().unwrap();
        let p = cache_with(d.path(), 30, "2.28.0");
        let off = Env::from_pairs([("AI_GENT_NO_UPDATE_CHECK", "1")]);
        let r = decide(&root(Some("2.27.0"), Some(d.path())), &off, &p, NOW);
        assert!(!r.refresh && r.report.notice.is_none());
        std::fs::write(d.path().join("pinned"), "2.27.0\n").unwrap();
        let r = decide(
            &root(Some("2.27.0"), Some(d.path())),
            &Env::default(),
            &p,
            NOW,
        );
        assert!(!r.refresh && r.report.notice.is_none() && r.report.pinned);
        let r = decide(&root(None, Some(d.path())), &Env::default(), &p, NOW);
        assert!(!r.refresh && r.report.notice.is_none());
    }

    #[test]
    fn edge_cases() {
        let d = tempfile::tempdir().unwrap();
        // newer than the latest release (a pre-release build): no notice
        let p = cache_with(d.path(), 1, "2.27.0");
        let r = decide(
            &root(Some("2.28.0"), Some(d.path())),
            &Env::default(),
            &p,
            NOW,
        );
        assert!(r.report.notice.is_none());
        // a timestamp in the future is stale
        let p = d.path().join("c2");
        cache::write(&p, NOW + 99_999, "2.28.0").unwrap();
        let r = decide(
            &root(Some("2.27.0"), Some(d.path())),
            &Env::default(),
            &p,
            NOW,
        );
        assert!(r.refresh && r.report.stale);
    }
}
