//! The command log (spec 043 T009, FR-013): every command writes an entry BEFORE it touches the database
//! and a result line after, in the format of `log_start` and `log_result` of `plugins/db/scripts/dbrun.py`
//! (`<state>/log/<date>_<repo>.md`, mode 600, append only).
//!
//! Fail closed: a log that cannot be written is an error and the caller must not execute (Python let the
//! exception propagate; here it is a typed [`LogError`]). Beyond Python:
//!
//! - every entry passes through the profile's [`Redactor`] (password, host, user), reason and statements
//!   included;
//! - text that comes from the user (reason, statements, extra) cannot forge a log structure: a line that
//!   begins a heading (`## `) or a code fence is prefixed with a tab, and control characters other than
//!   tab and line feed become U+FFFD, so one entry cannot imitate another or hide text in a terminal;
//! - the log path must not be a symbolic link or anything but a regular file, and one entry is at most
//!   [`MAX_ENTRY_BYTES`].
use super::redact::Redactor;
use std::fmt::{self, Write as _};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Largest entry written (a bound: statements are capped by the plan layer well below it).
pub const MAX_ENTRY_BYTES: usize = 32 * 1024 * 1024;

/// A log write that did not happen. The message names no log content.
#[derive(Debug, PartialEq, Eq)]
pub struct LogError(pub String);

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LogError {}

/// The time of a command: `date` names the log file, `iso` heads the entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// `YYYY-MM-DDTHH:MM:SS+00:00`.
    pub iso: String,
}

/// Time and identifiers, replaced in tests.
pub trait Clock {
    /// The current time.
    fn now(&self) -> Stamp;
    /// `hex_len` lowercase hex characters, unpredictable enough to tell two entries apart.
    fn token(&self, hex_len: usize) -> String;
}

/// UTC time and ids from the standard library's randomly keyed hasher.
#[derive(Debug, Default)]
pub struct SystemClock;

/// Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Formats Unix seconds as the `Stamp` of that UTC instant.
#[must_use]
pub fn stamp_at(secs: i64) -> Stamp {
    let (y, m, d) = civil(secs.div_euclid(86_400));
    let t = secs.rem_euclid(86_400);
    Stamp {
        date: format!("{y:04}-{m:02}-{d:02}"),
        iso: format!(
            "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}+00:00",
            t / 3600,
            t % 3600 / 60,
            t % 60
        ),
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Stamp {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
        stamp_at(secs)
    }

    fn token(&self, hex_len: usize) -> String {
        use std::hash::{BuildHasher, Hasher};
        let mut out = String::new();
        let mut round = 0u64;
        while out.len() < hex_len {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(round);
            h.write_u128(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos()),
            );
            let _ = write!(out, "{:016x}", h.finish());
            round += 1;
        }
        out.truncate(hex_len);
        out
    }
}

/// What an entry says about the database and how it was reached (`p` and `cls` of the original).
#[derive(Debug, Clone, Copy)]
pub struct Subject<'a> {
    /// Profile name.
    pub name: &'a str,
    /// Engine word.
    pub engine: &'a str,
    /// Environment class word.
    pub class: &'a str,
    /// Whether writes are allowed.
    pub writable: bool,
    /// `file`, `host` or `container`.
    pub via: &'a str,
    /// The classification evidence.
    pub evidence: &'a [String],
}

/// The command log of one repository.
pub struct CommandLog {
    dir: PathBuf,
    repo: String,
    redactor: Redactor,
}

/// Control characters (but tab) and the invisible format and bidi characters that can hide or reorder text.
fn hidden(c: char) -> bool {
    (c.is_control() && c != '\t')
        || matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}')
}

/// Makes user text safe to put in an entry: no forged headings or fences, no control characters.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let t = line.trim_start();
        if t.starts_with("## ") || t.starts_with("```") || (i > 0 && t.starts_with("- ")) {
            out.push('\t');
        }
        out.extend(line.chars().map(|c| if hidden(c) { '\u{fffd}' } else { c }));
    }
    out
}

impl CommandLog {
    /// A log under `dir` (created with mode 700 when missing) for repository `repo`.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>, repo: &str, redactor: Redactor) -> Self {
        Self {
            dir: dir.into(),
            repo: repo.to_string(),
            redactor,
        }
    }

    /// The file of `date`.
    #[must_use]
    pub fn path(&self, date: &str) -> PathBuf {
        let repo: String = self
            .repo
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.dir.join(format!("{date}_{repo}.md"))
    }

    fn append(&self, date: &str, entry: &str) -> Result<(), LogError> {
        let text = format!("{}\n\n", self.redactor.redact(entry).trim_end_matches('\n'));
        if text.len() > MAX_ENTRY_BYTES {
            return Err(LogError("the log entry is too large".into()));
        }
        let io = |what: &str, e: &std::io::Error| {
            LogError(format!("cannot {what} the command log: {}", e.kind()))
        };
        if !self.dir.exists() {
            let mut b = std::fs::DirBuilder::new();
            b.recursive(true);
            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
            b.create(&self.dir).map_err(|e| io("create", &e))?;
        }
        let path = self.path(date);
        if let Ok(m) = std::fs::symlink_metadata(&path)
            && !m.is_file()
        {
            return Err(LogError(
                "the command log path is not a regular file".into(),
            ));
        }
        let mut o = OpenOptions::new();
        o.append(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let mut f = o.open(&path).map_err(|e| io("open", &e))?;
        f.write_all(text.as_bytes()).map_err(|e| io("write", &e))?;
        f.sync_data().map_err(|e| io("flush", &e))
    }

    /// Writes the start of an entry and returns its id. Call this BEFORE anything runs.
    ///
    /// # Errors
    /// [`LogError`] when the entry cannot be written; the caller must stop.
    pub fn start(
        &self,
        clock: &dyn Clock,
        action: &str,
        s: &Subject<'_>,
        reason: &str,
        statements: &[String],
        extra: &str,
    ) -> Result<LogEntry, LogError> {
        let stamp = clock.now();
        let id = clock.token(8);
        let body = statements
            .iter()
            .map(|x| plain(x))
            .collect::<Vec<_>>()
            .join("\n\n");
        let entry = format!(
            "## {} · {action} · {} · {id}\n\n- engine: {} · class: {} · writable: {} · via: {}\n- evidence: {}\n- reason: {}\n{}{}\n- status: started",
            stamp.iso,
            plain(s.name),
            plain(s.engine),
            plain(s.class),
            s.writable,
            plain(s.via),
            plain(&s.evidence.join("; ")),
            plain(reason),
            if extra.is_empty() {
                String::new()
            } else {
                format!("- {}\n", plain(extra))
            },
            if statements.is_empty() {
                String::new()
            } else {
                format!("\n```sql\n{body}\n```\n")
            },
        );
        self.append(&stamp.date, &entry)?;
        Ok(LogEntry {
            id,
            date: stamp.date,
        })
    }

    /// Writes the result line of an entry.
    ///
    /// # Errors
    /// [`LogError`] when it cannot be written.
    pub fn result(&self, entry: &LogEntry, text: &str) -> Result<(), LogError> {
        self.append(
            &entry.date,
            &format!("- {} result: {}", entry.id, plain(text)),
        )
    }

    /// The last `n` entries, as `dbrun log --tail` prints them (bounded read).
    ///
    /// # Errors
    /// [`LogError`] when the file cannot be read within 64 MiB.
    pub fn tail(&self, date: &str, n: usize) -> Result<Vec<String>, LogError> {
        let path = self.path(date);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = crate::fsutil::read_text_bounded(&path, 64 * 1024 * 1024).map_err(LogError)?;
        let entries: Vec<&str> = text.split("\n## ").collect();
        let from = entries.len().saturating_sub(n);
        Ok(entries[from..]
            .iter()
            .map(|e| {
                format!(
                    "{}{}",
                    if e.starts_with("## ") { "" } else { "## " },
                    e.trim()
                )
            })
            .collect())
    }
}

/// An entry that was started: its id and the day it went to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// The 8-hex id.
    pub id: String,
    /// The log date.
    pub date: String,
}

impl AsRef<Path> for CommandLog {
    fn as_ref(&self) -> &Path {
        &self.dir
    }
}
