//! The autonomy grant: `.ai-gent/autonomy` at the repository root.
//!
//! `key: value` lines the user writes (`expires: 2026-11-06T18:00:00Z`, `branches: develop dev`,
//! `actions: push pr merge issues`). It counts only while `expires` lies ahead; a missing,
//! unreadable or malformed file is no grant. The git and repo guards stop asking for the forms it
//! covers, `ai-gent repo` makes the one check only it can (a PR's base), and the guards deny the
//! agent writing the file. Mirrors the grant functions of `plugins/{git,repo}/hooks/guard.sh`.
use super::rules::re;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// The path of the grant under the repository root.
pub const FILE: &str = ".ai-gent/autonomy";

/// An active grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    actions: Vec<String>,
    branches: Vec<String>,
}

impl Grant {
    /// Whether the grant names `action` (`push`, `pr`, `merge`, `issues`).
    #[must_use]
    pub fn has(&self, action: &str) -> bool {
        self.actions.iter().any(|a| a == action)
    }

    /// Whether `branch` is one the grant covers.
    #[must_use]
    pub fn covers(&self, branch: &str) -> bool {
        self.branches.iter().any(|b| b == branch)
    }

    /// The branches the grant covers.
    #[must_use]
    pub fn branches(&self) -> &[String] {
        &self.branches
    }
}

/// The active grant of the repository that contains `dir`, as of now.
#[must_use]
pub fn load(dir: &str) -> Option<Grant> {
    let top = Command::new("git")
        .args(["-C", if dir.is_empty() { "." } else { dir }])
        .args(["rev-parse", "--show-toplevel"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !top.status.success() {
        return None;
    }
    let top = String::from_utf8(top.stdout).ok()?;
    let text = std::fs::read_to_string(Path::new(top.trim_end_matches('\n')).join(FILE)).ok()?;
    parse(&text, &now_utc())
}

/// Parse the file's text against `now` (`YYYY-MM-DDTHH:MM:SSZ`).
#[must_use]
pub fn parse(text: &str, now: &str) -> Option<Grant> {
    let value = |key: &str| -> String {
        let prefix = format!("{key}:");
        text.lines()
            .find_map(|l| l.strip_prefix(&prefix))
            .map(|v| v.trim_start().trim_end_matches('\r').to_string())
            .unwrap_or_default()
    };
    let expires = value("expires");
    if !re(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$").is_match(&expires) {
        return None;
    }
    if now >= expires.as_str() {
        return None;
    }
    let words = |s: String| s.split_whitespace().map(str::to_string).collect::<Vec<_>>();
    let branches = words(value("branches"));
    Some(Grant {
        actions: words(value("actions")),
        branches: if branches.is_empty() {
            vec!["develop".into(), "dev".into()]
        } else {
            branches
        },
    })
}

/// The current time in UTC as `YYYY-MM-DDTHH:MM:SSZ`, which sorts like the time itself.
#[must_use]
pub fn now_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    format_utc(secs)
}

/// Seconds since the Unix epoch as `YYYY-MM-DDTHH:MM:SSZ`.
#[must_use]
pub fn format_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil date from a day count (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-11-06T12:00:00Z";

    #[test]
    fn formats_known_instants() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_utc(1_793_966_400), "2026-11-06T12:00:00Z");
    }

    #[test]
    fn an_active_grant_names_its_actions_and_branches() {
        let g = parse(
            "expires: 2099-01-01T00:00:00Z\nbranches: develop main-x\nactions: push pr\n",
            NOW,
        )
        .expect("active");
        assert!(g.has("push") && g.has("pr") && !g.has("merge"));
        assert!(g.covers("develop") && g.covers("main-x") && !g.covers("dev"));
    }

    #[test]
    fn branches_default_to_develop_and_dev() {
        let g = parse("expires: 2099-01-01T00:00:00Z\nactions: merge\n", NOW).expect("active");
        assert_eq!(g.branches(), ["develop", "dev"]);
    }

    #[test]
    fn expired_missing_or_malformed_is_no_grant() {
        for text in [
            "expires: 2000-01-01T00:00:00Z\nactions: push\n",
            "expires: 2026-11-06T12:00:00Z\nactions: push\n",
            "actions: push\n",
            "expires: tomorrow\nactions: push\n",
            "expires: 2099-01-01T00:00:00Z \nactions: push\n",
            "",
        ] {
            assert_eq!(parse(text, NOW), None, "{text:?}");
        }
    }

    #[test]
    fn carriage_returns_are_ignored() {
        let g = parse("expires: 2099-01-01T00:00:00Z\r\nactions: push\r\n", NOW).expect("active");
        assert!(g.has("push"));
    }
}
