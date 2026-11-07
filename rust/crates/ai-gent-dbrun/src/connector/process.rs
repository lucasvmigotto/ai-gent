//! The process connector: runs a command that speaks the `_exec` protocol (spec 038, option H-exec).
//!
//! The command reads one JSON [`Request`] on stdin and prints one JSON result line on stdout, as
//! `dbrun.py _exec` does. By default that is the host's `python3 <dbrun.py> _exec`; the program and its
//! arguments are configurable, so a digest-pinned client container command (or `uv run --with <pin>`)
//! plugs in without code changes.
//!
//! - Credentials travel on the child's stdin only: never in argv, never in the environment this
//!   connector sets, never in an error. The request is written then stdin is closed.
//! - stdout and stderr are drained concurrently, so a full pipe cannot deadlock the child.
//! - stderr is captured (bounded), redacted (password, host, user) and shown only as a short excerpt
//!   in the error of a command that exited without a result.
//! - Every failure (cannot start, exits without a result line, invalid JSON, timeout, oversized
//!   result) is a `connect`-stage [`ExecResult`] with a fixed message. On a timeout the child is
//!   killed and reaped.
//!
//! Limitation: the kill reaches the direct child. A wrapper that forks (`uv run`, a container
//! client) must forward the signal or exit when its pipes close.
use super::{ConnFacts, Connector, ExecResult, Request, Stage};
use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

/// Largest result line accepted by default.
pub const DEFAULT_MAX_OUTPUT: usize = 64 * 1024 * 1024;
/// Extra wall time granted on top of the request's own `timeout` by default.
pub const DEFAULT_GRACE: Duration = Duration::from_secs(30);
/// Bytes of stderr kept.
const STDERR_KEEP: usize = 8 * 1024;
/// Characters of redacted stderr shown in an error.
const STDERR_SHOWN: usize = 400;

const ALL_ENGINES: &[&str] = &[
    "postgresql",
    "mysql",
    "mariadb",
    "sqlserver",
    "oracle",
    "sqlite",
];

/// A [`Connector`] that delegates to an external `_exec` command.
#[derive(Debug, Clone)]
pub struct ProcessConnector {
    program: OsString,
    args: Vec<OsString>,
    timeout: Option<Duration>,
    max_output: usize,
}

impl ProcessConnector {
    /// Runs `program args...`; the request goes to its stdin.
    #[must_use]
    pub fn new<S: Into<OsString>>(
        program: impl Into<OsString>,
        args: impl IntoIterator<Item = S>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            timeout: None,
            max_output: DEFAULT_MAX_OUTPUT,
        }
    }

    /// The host default: `python3 <dbrun_py> _exec`.
    #[must_use]
    pub fn host_python(dbrun_py: impl Into<PathBuf>) -> Self {
        let script: PathBuf = dbrun_py.into();
        Self::new("python3", [script.into_os_string(), "_exec".into()])
    }

    /// Fixed wall-clock limit for the whole command. Without it the limit is the request's `timeout`
    /// plus [`DEFAULT_GRACE`].
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Largest result line accepted, in bytes.
    #[must_use]
    pub fn with_max_output(mut self, bytes: usize) -> Self {
        self.max_output = bytes;
        self
    }

    fn wall_limit(&self, req: &Request) -> Duration {
        self.timeout.unwrap_or_else(|| {
            let secs = if req.timeout.is_finite() && req.timeout > 0.0 {
                req.timeout.min(86_400.0)
            } else {
                0.0
            };
            let statements = f64::from(
                u32::try_from(req.statements.len())
                    .unwrap_or(u32::MAX)
                    .max(1),
            );
            Duration::from_secs_f64((secs * statements).min(86_400.0)) + DEFAULT_GRACE
        })
    }
}

enum Out {
    Line(Vec<u8>),
    TooLong,
    Eof,
}

/// Reads one line of at most `limit` bytes, then drains the rest so the child never blocks.
fn read_result(reader: impl Read, limit: usize, tx: &mpsc::Sender<Out>) {
    let mut r = BufReader::new(reader);
    let mut line = Vec::new();
    let out = loop {
        let buf = match r.fill_buf() {
            Ok([]) | Err(_) => {
                break if line.is_empty() {
                    Out::Eof
                } else {
                    Out::Line(line)
                };
            }
            Ok(b) => b,
        };
        let (take, done) = buf
            .iter()
            .position(|&b| b == b'\n')
            .map_or((buf.len(), false), |p| (p, true));
        if line.len() + take > limit {
            break Out::TooLong;
        }
        line.extend_from_slice(&buf[..take]);
        let consumed = take + usize::from(done);
        r.consume(consumed);
        if done {
            break Out::Line(line);
        }
    };
    let _ = tx.send(out);
    let _ = io::copy(&mut r, &mut io::sink());
}

fn read_stderr(reader: impl Read, tx: &mpsc::Sender<(Vec<u8>, bool)>) {
    let mut r = reader;
    let mut kept = Vec::new();
    let mut cut = false;
    let mut chunk = [0u8; 4096];
    while let Ok(n) = r.read(&mut chunk) {
        if n == 0 {
            break;
        }
        let room = STDERR_KEEP.saturating_sub(kept.len());
        cut |= n > room;
        kept.extend_from_slice(&chunk[..n.min(room)]);
    }
    let _ = tx.send((kept, cut));
}

/// Waits for the child until `deadline`; kills and reaps it past that. Returns the exit code.
fn finish(child: &mut Child, deadline: Instant) -> Option<i32> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.code(),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Plain replacement of the secrets, then the contract's word-aware redaction.
fn scrub(text: &str, conn: &ConnFacts) -> String {
    let value = |v: &Option<String>| v.clone().unwrap_or_default();
    let (password, host, user) = (value(&conn.password), value(&conn.host), value(&conn.user));
    let mut out = text.to_string();
    if !password.is_empty() {
        out = out.replace(&password, "<password>");
    }
    crate::core::redact(&out, &[(&host, "<host>"), (&user, "<user>")])
}

/// `cut` says the capture stopped early: a secret may straddle the end, so the tail that long
/// is dropped before redaction (a prefix of a secret would not match its replacement).
fn excerpt(raw: &[u8], cut: bool, conn: &ConnFacts) -> String {
    let longest = [&conn.password, &conn.host, &conn.user]
        .iter()
        .filter_map(|v| v.as_ref().map(String::len))
        .max()
        .unwrap_or(0);
    let raw = if cut {
        &raw[..raw.len().saturating_sub(longest)]
    } else {
        raw
    };
    let text = String::from_utf8_lossy(raw);
    let clean: String = scrub(&text, conn)
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    clean.chars().take(STDERR_SHOWN).collect()
}

fn fail(msg: &str) -> ExecResult {
    ExecResult::failure(Stage::Connect, format!("process connector: {msg}"))
}

impl Connector for ProcessConnector {
    fn engines(&self) -> &'static [&'static str] {
        ALL_ENGINES
    }

    fn execute(&self, req: &Request) -> ExecResult {
        let Ok(body) = serde_json::to_vec(req) else {
            return fail("the request could not be encoded");
        };
        let deadline = Instant::now() + self.wall_limit(req);
        let spawned = Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let Ok(mut child) = spawned else {
            return fail("the command could not be started");
        };
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            kill(&mut child);
            return fail("the command pipes could not be opened");
        };

        // Written then closed on its own thread: a child that never reads cannot block us.
        thread::spawn(move || {
            let mut stdin = stdin;
            let _ = stdin.write_all(&body);
            let _ = stdin.write_all(b"\n");
        });
        let (out_tx, out_rx) = mpsc::channel();
        let limit = self.max_output;
        thread::spawn(move || read_result(stdout, limit, &out_tx));
        let (err_tx, err_rx) = mpsc::channel();
        thread::spawn(move || read_stderr(stderr, &err_tx));

        let left = deadline.saturating_duration_since(Instant::now());
        match out_rx.recv_timeout(left) {
            Ok(Out::Line(line)) => {
                if finish(&mut child, deadline).is_none() {
                    return fail("the command did not exit in time and was killed");
                }
                match serde_json::from_slice::<ExecResult>(&line) {
                    Ok(result) => result,
                    Err(_) => fail("the command printed an invalid result"),
                }
            }
            Ok(Out::TooLong) => {
                kill(&mut child);
                fail("the result is larger than the allowed size")
            }
            Err(RecvTimeoutError::Timeout) => {
                kill(&mut child);
                fail("the command timed out and was killed")
            }
            Ok(Out::Eof) | Err(RecvTimeoutError::Disconnected) => {
                let code = finish(&mut child, deadline);
                let (err, cut) = err_rx
                    .recv_timeout(Duration::from_millis(500))
                    .unwrap_or_default();
                let shown = excerpt(&err, cut, &req.conn);
                let status = code.map_or_else(
                    || "was killed".to_string(),
                    |c| format!("exited with status {c}"),
                );
                let tail = if shown.is_empty() {
                    String::new()
                } else {
                    format!(": {shown}")
                };
                fail(&format!("the command {status} without a result{tail}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connector::{ConnFacts, Mode};

    #[test]
    fn the_wall_limit_scales_with_the_statement_count() {
        let c = ProcessConnector::new("x", Vec::<&str>::new());
        let mk = |n: usize| {
            let mut r = Request::new(
                "postgresql",
                ConnFacts::default(),
                Mode::Write,
                vec!["SELECT 1".to_string(); n],
            );
            r.timeout = 10.0;
            r
        };
        assert_eq!(
            c.wall_limit(&mk(1)),
            Duration::from_secs(10) + DEFAULT_GRACE
        );
        assert_eq!(
            c.wall_limit(&mk(4)),
            Duration::from_secs(40) + DEFAULT_GRACE
        );
        assert_eq!(
            c.wall_limit(&mk(0)),
            Duration::from_secs(10) + DEFAULT_GRACE
        );
    }
}
