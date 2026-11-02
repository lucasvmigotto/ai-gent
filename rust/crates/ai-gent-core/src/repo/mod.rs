//! `ai-gent repo`: the path to GitHub, Azure DevOps, GitLab, Bitbucket, Forgejo and Jira (a port of
//! `plugins/repo/scripts/reporun.py`, spec 037 Group C).
//!
//! Tokens come from pass (`shared/secrets.md`): the runner decrypts an entry only for the call that
//! needs it, hands it to the platform CLI through that process's environment or builds the API's auth
//! header itself, never on a command line and never printed (everything that leaves is redacted).
// A port of a Python tool; the structure mirrors it.
#![allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    clippy::missing_panics_doc,
    clippy::doc_markdown,
    clippy::assigning_clones,
    clippy::collapsible_if,
    clippy::format_push_string,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::type_complexity
)]

pub mod api;
pub mod batch;
pub mod chain;
pub mod cli;
pub mod detect;
pub mod entry;
pub mod http;
pub mod platforms;
pub mod project_status;
pub mod py;
pub mod remote;

use crate::env::Env;
use crate::passstore::{self, Config};
use std::io::Write;
use std::time::Duration;

/// Exit status: ok.
pub const EXIT_OK: i32 = 0;
/// Exit status: error.
pub const EXIT_ERROR: i32 = 1;
/// Exit status: usage.
pub const EXIT_USAGE: i32 = 2;
/// Exit status: refused (a write without `--write`, a blocked CLI verb, a host not allowed).
pub const EXIT_REFUSED: i32 = 3;

/// A failure: a message and the exit status. `HTTP` errors carry their status.
#[derive(Debug, Clone)]
pub struct Failed {
    /// What happened (redacted when printed).
    pub msg: String,
    /// Exit status.
    pub code: i32,
    /// Whether this is the refusal kind (printed as `REFUSED: ...`).
    pub refused: bool,
    /// HTTP status, for API errors.
    pub http_status: Option<u16>,
}

impl Failed {
    /// An error (exit 1).
    #[must_use]
    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            msg: msg.into(),
            code: EXIT_ERROR,
            refused: false,
            http_status: None,
        }
    }

    /// A usage error (exit 2).
    #[must_use]
    pub fn usage(msg: impl Into<String>) -> Self {
        Self {
            msg: msg.into(),
            code: EXIT_USAGE,
            refused: false,
            http_status: None,
        }
    }

    /// A refusal (exit 3).
    #[must_use]
    pub fn refused(msg: impl Into<String>) -> Self {
        Self {
            msg: msg.into(),
            code: EXIT_REFUSED,
            refused: true,
            http_status: None,
        }
    }
}

/// Result of a runner step.
pub type R<T> = Result<T, Failed>;

/// What every operation needs: the environment, the pass settings and the timeouts.
#[derive(Debug, Clone)]
pub struct Ctx {
    /// Environment snapshot (also what children inherit).
    pub env: Env,
    /// Pass settings.
    pub pass: Config,
    /// Seconds for one API request.
    pub timeout: f64,
}

impl Ctx {
    /// From the process environment.
    #[must_use]
    pub fn from_process() -> Self {
        Self::new(Env::real())
    }

    /// From an environment snapshot.
    #[must_use]
    pub fn new(env: Env) -> Self {
        Self {
            pass: Config::new(env.clone()),
            env,
            timeout: 30.0,
        }
    }

    /// `AI_GENT_PASS_PREFIX`, default `ai-gent`.
    #[must_use]
    pub fn prefix(&self) -> String {
        self.pass.prefix()
    }

    /// The command timeout as a duration.
    #[must_use]
    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs_f64(self.timeout.max(0.1))
    }
}

/// Print a line to stdout, redacted.
pub fn out(text: &str) {
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{}", passstore::redact(text));
}

/// Print a line to stderr, redacted.
pub fn err(text: &str) {
    let mut o = std::io::stderr().lock();
    let _ = writeln!(o, "{}", passstore::redact(text));
}

/// The status line the runner prints on stderr.
pub fn note(text: &str) {
    err(&format!("ai-gent repo: {text}"));
}
