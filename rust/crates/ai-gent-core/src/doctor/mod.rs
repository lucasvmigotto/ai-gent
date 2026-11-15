//! `ai-gent doctor`: an offline, read-only health report over the install tree (spec 034).
//!
//! Each check is an independent function returning one [`Check`]; the runner turns a panic in one
//! into a `fail`, so a broken fixture never hides the other twelve results.
use crate::env::Env;
use crate::install::InstallRoot;
use serde::Serialize;
use std::path::PathBuf;

pub mod checks;
pub mod selftest;

/// Status of one check.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Passed.
    Ok,
    /// Worth a look; not broken.
    Warn,
    /// Broken.
    Fail,
    /// Not applicable here.
    Skip,
}

/// One check result.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Check {
    /// Stable id.
    pub id: &'static str,
    /// Result.
    pub status: Status,
    /// What was found.
    pub detail: String,
    /// The exact command or step that fixes it.
    pub hint: Option<String>,
}

impl Check {
    pub(crate) fn new(id: &'static str, status: Status, detail: impl Into<String>) -> Self {
        Self {
            id,
            status,
            detail: detail.into(),
            hint: None,
        }
    }
    pub(crate) fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

/// Count per status.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct Summary {
    /// Passed.
    pub ok: u32,
    /// Warnings.
    pub warn: u32,
    /// Failures.
    pub fail: u32,
    /// Skipped.
    pub skip: u32,
}

/// `data` of `doctor --json`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Report {
    /// Results, in the order of `contracts/cli.md`.
    pub checks: Vec<Check>,
    /// Counts.
    pub summary: Summary,
}

impl Report {
    /// Exit status: 1 when a check failed (or warned, with `strict`).
    #[must_use]
    pub fn failed(&self, strict: bool) -> bool {
        self.summary.fail > 0 || (strict && self.summary.warn > 0)
    }
}

/// What the checks look at.
pub struct Ctx {
    /// Path of the running binary.
    pub exe: PathBuf,
    /// Release the binary was built as (`dev` outside the release workflow).
    pub release: String,
    /// Where the binary lives.
    pub root: InstallRoot,
    /// The environment.
    pub env: Env,
    /// Skip the MANIFEST hashing.
    pub fast: bool,
}

type CheckFn = fn(&Ctx) -> Check;

/// Every check, with its id (for the crash report).
const ALL: [(&str, CheckFn); 13] = [
    ("binary", checks::binary),
    ("install_root", checks::install_root),
    ("version_match", checks::version_match),
    ("install_integrity", checks::install_integrity),
    ("path_link", checks::path_link),
    ("agent_links", checks::agent_links),
    ("hooks", checks::hooks),
    ("guard_selftest", checks::guard_selftest),
    ("tools", checks::tools),
    ("state", checks::state),
    ("update_cache", checks::update_cache),
    ("environment", checks::environment),
    ("permissions", checks::permissions),
];

/// Run every check.
#[must_use]
pub fn run(ctx: &Ctx) -> Report {
    let mut checks = Vec::with_capacity(ALL.len());
    for (id, f) in ALL {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(ctx)));
        checks.push(r.unwrap_or_else(|_| Check::new(id, Status::Fail, "check crashed")));
    }
    let mut summary = Summary::default();
    for c in &checks {
        match c.status {
            Status::Ok => summary.ok += 1,
            Status::Warn => summary.warn += 1,
            Status::Fail => summary.fail += 1,
            Status::Skip => summary.skip += 1,
        }
    }
    Report { checks, summary }
}

impl Status {
    /// The word printed in text output.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "FAIL",
            Self::Skip => "skip",
        }
    }
}

impl Report {
    /// One line per check, hints indented below.
    #[must_use]
    pub fn text(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        for c in &self.checks {
            let _ = writeln!(out, "{:<4}  {:<18} {}", c.status.word(), c.id, c.detail);
            if let Some(h) = &c.hint {
                let _ = writeln!(out, "      -> {h}");
            }
        }
        let s = self.summary;
        let _ = writeln!(
            out,
            "{} ok, {} warn, {} fail, {} skip",
            s.ok, s.warn, s.fail, s.skip
        );
        out
    }
}
