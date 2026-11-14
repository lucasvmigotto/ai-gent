//! Connector isolation (spec 043 T012, FR-014): each engine call runs in a short-lived child process, so
//! a panic, an abort or a crash inside a driver becomes a failure JSON instead of ending the runner.
//!
//! Two halves of one contract, the `_exec` protocol of `dbrun.py`:
//!
//! - [`IsolatedConnector`] (the parent) starts `<program> <args>` (in the shipped binary,
//!   `ai-gent db _exec`), writes one JSON request on the child's stdin, reads ONE result line with a size
//!   cap, enforces a deadline, and kills the child by handle when it is late. It is the process connector
//!   ([`super::process::ProcessConnector`]: bounded stdout and stderr, redacted excerpts, fixed messages)
//!   with two additions: the child's environment is rebuilt from a short allowlist, never inherited, and a
//!   child that dies during a write request is a failure at stage `execute`, not `connect`.
//! - [`serve_exec`] (the child) reads the request from stdin with its own bound, runs the real connector,
//!   prints one line. A panic that unwinds is caught and turned into a failure line; one that aborts
//!   (`panic = "abort"`) ends the child and the parent reports it.
//!
//! No secret travels in argv or the environment: the request, password included, is on stdin only; the
//! environment holds `PATH`, the platform's system and temp variables, locale and certificate-store
//! variables, and anything the caller adds with [`IsolatedConnector::with_env`] (a caller that adds a
//! secret defeats this on its own).
use super::process::ProcessConnector;
use super::request::MAX_REQUEST_LINE;
use super::{Connector, ExecResult, Mode, Request, Stage};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::time::Duration;

/// The variables a child may inherit from its parent. Everything else (`PGPASSWORD`, `MYSQL_PWD`,
/// `ORACLE_*`, tokens, `AWS_*`, ...) is dropped. Compared in upper case, as Windows names are not
/// case-sensitive.
pub const INHERITED_ENV: &[&str] = &[
    "PATH",
    "SYSTEMROOT",
    "WINDIR",
    "COMSPEC",
    "TEMP",
    "TMP",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TZ",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

/// The allowed pairs of the current process environment.
#[must_use]
pub fn clean_env() -> Vec<(OsString, OsString)> {
    std::env::vars_os()
        .filter(|(k, _)| {
            k.to_str()
                .is_some_and(|k| INHERITED_ENV.contains(&k.to_ascii_uppercase().as_str()))
        })
        .collect()
}

/// A [`Connector`] that runs every request in a child process.
#[derive(Debug, Clone)]
pub struct IsolatedConnector {
    program: PathBuf,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
    timeout: Option<Duration>,
    max_output: Option<usize>,
}

impl IsolatedConnector {
    /// Runs `program args...` for each request, with the allowlisted environment.
    #[must_use]
    pub fn new<S: Into<OsString>>(
        program: impl Into<PathBuf>,
        args: impl IntoIterator<Item = S>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            env: clean_env(),
            timeout: None,
            max_output: None,
        }
    }

    /// The shipped shape: this executable with `db _exec`.
    ///
    /// # Errors
    /// When the path of the running executable is unknown.
    pub fn current_exe() -> std::io::Result<Self> {
        Ok(Self::new(std::env::current_exe()?, ["db", "_exec"]))
    }

    /// Adds one environment variable for the child (never a credential).
    #[must_use]
    pub fn with_env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// A fixed wall-clock limit for one call (default: the request's `timeout` per statement plus 30 s).
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Largest result line accepted from the child.
    #[must_use]
    pub fn with_max_output(mut self, bytes: usize) -> Self {
        self.max_output = Some(bytes);
        self
    }

    fn process(&self) -> ProcessConnector {
        let mut p = ProcessConnector::new(&self.program, self.args.clone())
            .with_clean_env(self.env.clone());
        if let Some(t) = self.timeout {
            p = p.with_timeout(t);
        }
        if let Some(b) = self.max_output {
            p = p.with_max_output(b);
        }
        p
    }
}

impl Connector for IsolatedConnector {
    fn engines(&self) -> &'static [&'static str] {
        &[
            "postgresql",
            "mysql",
            "mariadb",
            "sqlserver",
            "oracle",
            "sqlite",
        ]
    }

    fn execute(&self, req: &Request) -> ExecResult {
        let mut result = self.process().execute(req);
        // A child that failed to deliver a result while a write was running leaves the transaction in the
        // server's hands (it rolls back when the connection closes, but the runner never saw that): that
        // is an `execute` failure, and it says so.
        if !result.ok && req.mode == Mode::Write && result.stage == Some(Stage::Connect) {
            result.stage = Some(Stage::Execute);
            let why = result.error.take().unwrap_or_default();
            result.error = Some(format!(
                "{why}; the transaction was not confirmed committed, check the database state"
            ));
        }
        result
    }
}

/// The child side of `_exec`: reads ONE request line from `input`, runs it on `connector`, writes ONE result
/// line to `output`. The read stops at [`MAX_REQUEST_LINE`] bytes, so a flood on stdin cannot grow memory
/// (the check in [`Connector::run_line`] only runs after the line is in memory). Returns the exit code: 0
/// when a line was written, 1 when the output could not be written.
pub fn serve_exec(connector: &dyn Connector, input: impl Read, mut output: impl Write) -> i32 {
    let mut raw = Vec::new();
    let limit = MAX_REQUEST_LINE as u64 + 2;
    let read = BufReader::new(input.take(limit)).read_until(b'\n', &mut raw);
    let line = if read.is_err() {
        ExecResult::failure(Stage::Request, "invalid request: unreadable").to_line()
    } else {
        while matches!(raw.last(), Some(b'\n' | b'\r')) {
            raw.pop();
        }
        if raw.len() > MAX_REQUEST_LINE {
            ExecResult::failure(Stage::Request, "invalid request: too large").to_line()
        } else if let Ok(text) = std::str::from_utf8(&raw) {
            catch_unwind(AssertUnwindSafe(|| connector.run_line(text))).unwrap_or_else(|_| {
                ExecResult::failure(
                    Stage::Execute,
                    "the engine driver failed unexpectedly; the detail is withheld because it may hold a credential",
                )
                .to_line()
            })
        } else {
            ExecResult::failure(Stage::Request, "invalid request: not valid UTF-8").to_line()
        }
    };
    match writeln!(output, "{line}").and_then(|()| output.flush()) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}
