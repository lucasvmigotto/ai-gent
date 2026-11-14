//! The SQLite connector (feature `sqlite`): rusqlite with bundled SQLite, no system library.
//!
//! A read opens the file with `mode=ro` and sets `query_only`: the server itself refuses a write the
//! classifier missed (`mode=ro` blocks the main file, `query_only` blocks TEMP). A write runs one
//! transaction and commits it only when the row counts match `expect`.
use super::{Connector, ExecResult, Mode, Request, Stage, StatementResult, request};
use crate::core::explain_sql;
use rusqlite::backup::StepResult;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::time::{Duration, Instant};

/// Lock wait, as `LOCK_TIMEOUT_MS` in `dbrun.py`.
const LOCK_TIMEOUT: Duration = Duration::from_secs(5);
/// Pages one step of [`copy_database`] copies.
const COPY_PAGES_PER_STEP: i32 = 256;
/// Longest a single step of [`copy_database`] waits for a lock before it reports busy.
const COPY_LOCK_WAIT: Duration = Duration::from_millis(100);
/// Pause between two steps that found the database locked.
const COPY_PAUSE: Duration = Duration::from_millis(20);

/// The longest string or blob SQLite may hold for this connector (32 MiB); [`request::MAX_VALUE_BYTES`] of
/// it reach the result.
const MAX_SQLITE_VALUE: i32 = 32 * 1024 * 1024;

/// The open failure SQLite itself reports for a missing file (the text fixture 17 pins).
const UNABLE_TO_OPEN: &str = "sqlite: unable to open database file";

/// Executes requests against a SQLite file named by `conn.path`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SqliteConnector;

impl Connector for SqliteConnector {
    fn engines(&self) -> &'static [&'static str] {
        &["sqlite"]
    }

    fn execute(&self, req: &Request) -> ExecResult {
        let started = Instant::now();
        let conn = match connect(req) {
            Ok(c) => c,
            Err(e) => return ExecResult::failure(Stage::Connect, e),
        };
        let mut result = ExecResult::success();
        if let Err(e) = run(&conn, req, &mut result) {
            if !conn.is_autocommit() {
                let _ = conn.execute_batch("ROLLBACK");
            }
            result.ok = false;
            result.stage = Some(Stage::Execute);
            result.error = Some(format!("sqlite: {}", describe(&e)));
        }
        result.seconds = Some(started.elapsed().as_secs_f64());
        result
    }
}

/// Why an open was refused: a link where none may be followed, and a file that changed between the two opens.
const NOT_A_LINK: &str =
    "sqlite: unable to open database file: a symbolic link is not followed here";
const CHANGED: &str = "sqlite: unable to open database file: the file changed while it was opened";

/// A database file the runner opened itself, before SQLite opens it by name: SQLite takes a name, not a
/// handle, so the runner proves what the name points at and checks again once SQLite has it.
#[derive(Debug)]
pub struct CheckedFile {
    file: std::fs::File,
    #[cfg(unix)]
    id: (u64, u64),
}

/// Opens `path` read-only for [`CheckedFile`], without blocking and without creating anything: `O_NONBLOCK`
/// (a FIFO swapped in cannot block the open) and, when `follow` is false, `O_NOFOLLOW` after an `lstat`
/// that refuses a link in the last component. The opened handle must be a regular file. The messages name
/// no path.
///
/// # Errors
/// Fixed text, prefixed with SQLite's own open-failure text (`unable to open database file`).
pub fn open_checked(path: &Path, follow: bool) -> Result<CheckedFile, String> {
    if !follow {
        match std::fs::symlink_metadata(path) {
            Err(_) => return Err(UNABLE_TO_OPEN.to_string()),
            Ok(m) if m.file_type().is_symlink() => return Err(NOT_A_LINK.to_string()),
            Ok(_) => {}
        }
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | if follow { 0 } else { libc::O_NOFOLLOW });
    }
    let file = options.open(path).map_err(|e| {
        #[cfg(unix)]
        if e.raw_os_error() == Some(libc::ELOOP) {
            return NOT_A_LINK.to_string();
        }
        let _ = e;
        UNABLE_TO_OPEN.to_string()
    })?;
    let meta = file.metadata().map_err(|_| UNABLE_TO_OPEN.to_string())?;
    if !meta.is_file() {
        return Err(format!("{UNABLE_TO_OPEN}: not a regular file"));
    }
    #[cfg(unix)]
    let id = {
        use std::os::unix::fs::MetadataExt;
        (meta.dev(), meta.ino())
    };
    Ok(CheckedFile {
        file,
        #[cfg(unix)]
        id,
    })
}

impl CheckedFile {
    /// Whether `path` still names the file this handle has (same device and inode on Unix; on other
    /// systems only the link check is repeated). Call it after SQLite opened the name.
    ///
    /// # Errors
    /// [`NOT_A_LINK`] or [`CHANGED`] text.
    pub fn still_at(&self, path: &Path, follow: bool) -> Result<(), String> {
        let meta = if follow {
            std::fs::metadata(path)
        } else {
            std::fs::symlink_metadata(path)
        }
        .map_err(|_| CHANGED.to_string())?;
        if meta.file_type().is_symlink() {
            return Err(NOT_A_LINK.to_string());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if (meta.dev(), meta.ino()) != self.id {
                return Err(CHANGED.to_string());
            }
        }
        Ok(())
    }

    /// Proof for a file the runner has just created itself (exclusively): its identity, to check after SQLite
    /// opens the name.
    ///
    /// # Errors
    /// When the handle's metadata cannot be read or it is not a regular file.
    pub fn created(file: std::fs::File) -> Result<Self, String> {
        let meta = file.metadata().map_err(|_| UNABLE_TO_OPEN.to_string())?;
        if !meta.is_file() {
            return Err(format!("{UNABLE_TO_OPEN}: not a regular file"));
        }
        #[cfg(unix)]
        let id = {
            use std::os::unix::fs::MetadataExt;
            (meta.dev(), meta.ino())
        };
        Ok(Self {
            file,
            #[cfg(unix)]
            id,
        })
    }

    /// The opened handle (to read the file it proved).
    pub fn file(&mut self) -> &mut std::fs::File {
        &mut self.file
    }
}

fn connect(req: &Request) -> Result<Connection, String> {
    let path = req
        .conn
        .path
        .as_deref()
        .filter(|p| !p.is_empty())
        .ok_or("missing conn.path")?;
    // `run` validated it already; a direct call to `execute` gets the same refusal, never a default.
    let timeout = request::timeout(req.timeout).map_err(String::from)?;
    let read = req.mode != Mode::Write;
    let proof = open_checked(Path::new(path), read)?;
    // Reserved URI characters are percent-encoded so a literal file name cannot add parameters.
    let encoded: String = path
        .bytes()
        .map(|b| match b {
            b'%' | b'?' | b'#' => format!("%{b:02X}"),
            _ if !b.is_ascii() => format!("%{b:02X}"),
            _ => char::from(b).to_string(),
        })
        .collect();
    // No CREATE flag: a missing file is a connect failure in every mode (Python created one in write
    // mode; its CLI never got there, the local proof needs an existing file).
    let flags = if read {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW
    };
    let uri = format!("file:{encoded}{}", if read { "?mode=ro" } else { "" });
    let mut conn = Connection::open_with_flags(uri, flags | OpenFlags::SQLITE_OPEN_URI)
        .map_err(|e| format!("sqlite: {}", describe(&e)))?;
    let setup = |conn: &mut Connection| -> rusqlite::Result<()> {
        conn.busy_timeout(LOCK_TIMEOUT)?;
        // SQLite's own allocation for one value: a longer string or blob is an `execute` failure ("string
        // or blob too big"), even when it is only read from the file.
        conn.set_limit(
            rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
            MAX_SQLITE_VALUE,
        )?;
        let deadline = Instant::now().checked_add(timeout);
        // No representable deadline (impossible for at most an hour) interrupts at once: fail closed.
        conn.progress_handler(
            10_000,
            Some(move || deadline.is_none_or(|d| Instant::now() > d)),
        )?;
        if read {
            conn.execute_batch("PRAGMA query_only = ON")?;
            // Installed after the pragma above: a read can neither turn query_only off nor
            // attach another database (an in-memory one would be writable).
            conn.authorizer(Some(|ctx: AuthContext<'_>| match ctx.action {
                AuthAction::Attach { .. } | AuthAction::Detach { .. } => Authorization::Deny,
                AuthAction::Pragma {
                    pragma_name,
                    pragma_value,
                } if pragma_name.eq_ignore_ascii_case("query_only")
                    && pragma_value.is_some_and(|v| !is_on(v)) =>
                {
                    Authorization::Deny
                }
                _ => Authorization::Allow,
            }))?;
        }
        Ok(())
    };
    setup(&mut conn).map_err(|e| format!("sqlite: {}", describe(&e)))?;
    // SQLite opened the name: it must still be the regular file proven above. (A read-only open by name can
    // still block if a FIFO is swapped in between the two opens; the child process of FR-014 bounds that
    // with its deadline and kill.)
    proof.still_at(Path::new(path), read)?;
    Ok(conn)
}
fn is_on(value: &str) -> bool {
    let v = value
        .trim_matches(|c| c == '\'' || c == '"')
        .to_ascii_lowercase();
    matches!(v.as_str(), "on" | "1" | "true" | "yes")
}

/// The error kind and SQLite's own message, never the statement text. SQLite quotes a token of the
/// statement in some messages (`near "x": syntax error`, `unrecognized token: "x"`): those are cut.
fn describe(e: &rusqlite::Error) -> String {
    let message = match e {
        rusqlite::Error::SqlInputError { msg, .. } => msg.clone(),
        rusqlite::Error::SqliteFailure(err, msg) => msg.clone().unwrap_or_else(|| err.to_string()),
        other => other.to_string(),
    };
    strip_quoted_sql(&message)
}

fn strip_quoted_sql(message: &str) -> String {
    for prefix in ["near \"", "unrecognized token: \""] {
        if let Some(start) = message.find(prefix) {
            let head = &message[..start];
            let tail = message[start + prefix.len()..]
                .find('"')
                .map_or("", |i| &message[start + prefix.len() + i + 1..]);
            let label = prefix.trim_end_matches(" \"").trim_end_matches('"');
            return format!("{head}{}{tail}", label.trim_end_matches(':'));
        }
    }
    message.to_string()
}

fn run(conn: &Connection, req: &Request, result: &mut ExecResult) -> rusqlite::Result<()> {
    let version: String = conn.query_row("SELECT sqlite_version()", [], |r| r.get(0))?;
    result.server = Some(vec![
        Value::from(format!("SQLite {version}")),
        Value::from("file"),
        Value::from("main"),
    ]);
    result.links = Some(conn.query_row(
        "SELECT COUNT(*) - 1 FROM pragma_database_list",
        [],
        |r| r.get::<_, i64>(0),
    )?);
    if req.mode == Mode::Test {
        return Ok(());
    }
    let read = req.mode == Mode::Read;
    if !read {
        conn.execute_batch("BEGIN")?;
    }
    // Bytes of values the result may still carry, over every statement (FR-011's 16 MiB).
    let mut budget = request::MAX_RESULT_BYTES;
    // Results go straight into `result`, so a failure at statement N keeps the ones before it.
    for sql in &req.statements {
        let sql = if req.explain {
            explain_sql("sqlite", sql)
        } else {
            sql.clone()
        };
        if read {
            // Re-asserted per statement; the authorizer refuses any other value.
            conn.execute_batch("PRAGMA query_only = ON")?;
        }
        let one = run_statement(conn, &sql, read, req.max_rows, &mut budget)?;
        result.results.get_or_insert_with(Vec::new).push(one);
    }
    if read {
        return Ok(());
    }
    let counts: Vec<i64> = result
        .results
        .iter()
        .flatten()
        .map(|r| r.rowcount)
        .collect();
    if let Some(error) = request::verify_counts(&req.expect, &counts) {
        conn.execute_batch("ROLLBACK")?;
        result.ok = false;
        result.stage = Some(Stage::Verify);
        result.error = Some(error);
    } else {
        conn.execute_batch("COMMIT")?;
        result.committed = Some(true);
    }
    Ok(())
}

/// Runs one statement. The row count comes from the connection's own change counter and from what SQLite
/// says about the prepared statement (read-only or not, columns or not), never from the statement text:
///
/// - a read-only statement that returns rows (a SELECT) reports `-1`;
/// - a statement that writes AND returns rows (`... RETURNING`, a writing PRAGMA) reports `0`, the value
///   Python's `sqlite3` gives (it reads the count before the rows are fetched): each engine keeps its
///   original row-count value (owner decision (chat 2026-11-14));
/// - any other statement that changed rows reports `changes()` (its own rows, triggers excluded), so a
///   `WITH ... DELETE` counts its rows (D-007);
/// - DDL and DML that matched nothing report `0` (D-001).
///
/// A fresh `changes()` above the total-changes difference is impossible, so it is refused rather than
/// reported.
fn run_statement(
    conn: &Connection,
    sql: &str,
    read: bool,
    cap: usize,
    budget: &mut usize,
) -> rusqlite::Result<StatementResult> {
    let mut statement = conn.prepare(sql)?;
    let before = conn.total_changes();
    let readonly = statement.readonly();
    let columns = column_names(&statement)?;
    let returns_rows = !columns.is_empty();
    let (rows, truncated) = if returns_rows {
        let mut query = statement.query([])?;
        let mut failure = None;
        let ncols = columns.len();
        let source = std::iter::from_fn(|| match query.next() {
            Ok(Some(row)) => match read_row(row, ncols, budget) {
                Ok(values) => Some(values),
                Err(e) => {
                    failure = Some(e);
                    None
                }
            },
            Ok(None) => None,
            Err(e) => {
                failure = Some(e);
                None
            }
        });
        // A read stops at `cap + 1`; a write drains the statement so its changes are all made.
        let kept = request::cap_rows(source, read, cap);
        if let Some(e) = failure {
            return Err(e);
        }
        kept
    } else {
        statement.execute([])?;
        (Vec::new(), false)
    };
    let changed = conn.total_changes() != before;
    let server_count = changed.then(|| i64::try_from(conn.changes()).unwrap_or(i64::MAX));
    if let Some(n) = server_count {
        let total = i64::try_from(conn.total_changes() - before).unwrap_or(i64::MAX);
        if n > total {
            return Err(sqlite_error(
                "the change counter is inconsistent; refusing to report a row count",
            ));
        }
    }
    let kind = if returns_rows && !readonly {
        request::CountKind::Ddl
    } else if changed {
        request::CountKind::Dml
    } else if returns_rows {
        request::CountKind::Rows
    } else {
        request::CountKind::Ddl
    };
    let rowcount = request::rowcount(kind, server_count).map_err(sqlite_error)?;
    Ok(StatementResult {
        rowcount,
        columns: returns_rows.then_some(columns),
        rows: returns_rows.then_some(rows),
        truncated: returns_rows.then_some(truncated),
    })
}

fn sqlite_error(message: &str) -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
        Some(message.to_string()),
    )
}

/// The statement's column names. rusqlite panics on a name that is not UTF-8 (a crafted schema can hold
/// one); that panic is caught here and becomes an `execute` failure, so it never takes the runner down.
/// Under `panic = "abort"` the child process of FR-014 dies instead and the parent reports a failure.
fn column_names(statement: &rusqlite::Statement<'_>) -> rusqlite::Result<Vec<String>> {
    let mut names = Vec::with_capacity(statement.column_count());
    for i in 0..statement.column_count() {
        match catch_unwind(AssertUnwindSafe(|| {
            statement.column_name(i).map(String::from)
        })) {
            Ok(name) => names.push(name?),
            Err(_) => return Err(sqlite_error("a column name is not valid UTF-8")),
        }
    }
    Ok(names)
}

/// One row as JSON values. A text value keeps at most [`request::MAX_VALUE_BYTES`] bytes (cut and marked), a
/// blob is only its length; every value is charged to `budget`, and a row that would pass it fails the
/// statement, so one result never grows past [`request::MAX_RESULT_BYTES`] of values.
fn read_row(
    row: &rusqlite::Row<'_>,
    ncols: usize,
    budget: &mut usize,
) -> rusqlite::Result<Vec<Value>> {
    let mut values = Vec::with_capacity(ncols);
    for i in 0..ncols {
        let value = match row.get_ref(i)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(v) => Value::from(v),
            ValueRef::Real(v) => Value::from(v),
            ValueRef::Text(v) => {
                Value::from(request::text_value_capped(v, request::MAX_VALUE_BYTES))
            }
            ValueRef::Blob(v) => Value::from(format!("<{} bytes>", v.len())),
        };
        let cost = match &value {
            Value::String(text) => text.len(),
            _ => 24,
        };
        *budget = budget.checked_sub(cost).ok_or_else(|| {
            sqlite_error("the result is larger than 16 MiB; select fewer rows or columns")
        })?;
        values.push(value);
    }
    Ok(values)
}

/// Copies the database open on `from` over the one open on `to` with SQLite's online backup API, in this
/// process (no container to exec into): the backup before `apply` and the `restore` after it (FR-018).
///
/// Bounded and all-or-nothing:
///
/// - `from` holds a read transaction for the whole copy, so the source cannot change under it and its size
///   (`page_count x page_size`) is known exactly before the first page moves: a source over `max_bytes` is
///   refused before anything is written;
/// - the deadline is checked before every step, the last one included, and a step waits at most 100 ms for a
///   lock and copies at most 256 pages, so the commit (which the last step makes) never starts past the
///   deadline and ends at most one step after it; the size probes wait only for the time that is left;
/// - the copy is one write transaction on `to`: a copy that stops (deadline, lock, error) leaves `to` as it
///   was.
///
/// The messages are fixed text.
///
/// # Errors
/// Why the copy stopped.
pub fn copy_database(
    from: &Connection,
    to: &mut Connection,
    timeout: Duration,
    max_bytes: u64,
) -> Result<(), String> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or("the copy has no usable deadline")?;
    let left = || deadline.saturating_duration_since(Instant::now());
    let late = || "the copy did not finish in time (is the database in use?)".to_string();
    let in_use = |_| "cannot read the database's size (is the database in use?)".to_string();
    from.busy_timeout(left())
        .map_err(|_| "cannot set the lock wait".to_string())?;
    // A read transaction pins the source: what is measured here is what is copied.
    from.execute_batch("BEGIN")
        .and_then(|()| {
            from.query_row("SELECT COUNT(*) FROM sqlite_schema", [], |r| {
                r.get::<_, i64>(0)
            })
        })
        .map_err(in_use)?;
    let result = (|| {
        let page_size: i64 = from
            .query_row("PRAGMA page_size", [], |r| r.get(0))
            .map_err(in_use)?;
        let pages: i64 = from
            .query_row("PRAGMA page_count", [], |r| r.get(0))
            .map_err(in_use)?;
        let bytes = u64::try_from(pages)
            .ok()
            .zip(u64::try_from(page_size).ok())
            .map(|(p, s)| p.saturating_mul(s));
        if bytes.is_none_or(|b| b > max_bytes) {
            return Err("the database is larger than the backup size cap".to_string());
        }
        from.busy_timeout(COPY_LOCK_WAIT)
            .and_then(|()| to.busy_timeout(COPY_LOCK_WAIT))
            .map_err(|_| "cannot set the lock wait".to_string())?;
        let copy = rusqlite::backup::Backup::new(from, to)
            .map_err(|_| "cannot start the copy".to_string())?;
        loop {
            if left().is_zero() {
                return Err(late());
            }
            match copy
                .step(COPY_PAGES_PER_STEP)
                .map_err(|_| "the copy failed".to_string())?
            {
                StepResult::Done => break,
                StepResult::Busy | StepResult::Locked => {
                    std::thread::sleep(COPY_PAUSE.min(left()));
                }
                _ => {}
            }
        }
        // Pinned by the read transaction: a different size here would be a SQLite bug, reported, not hidden.
        if i64::from(copy.progress().pagecount) != pages {
            return Err("the source changed size during the copy; check the copy".to_string());
        }
        Ok(())
    })();
    let _ = from.execute_batch("COMMIT");
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connector::ConnFacts;

    fn facts(path: &std::path::Path) -> ConnFacts {
        ConnFacts {
            path: Some(path.to_string_lossy().into_owned()),
            ..ConnFacts::default()
        }
    }

    #[test]
    fn read_only_open_and_query_only_each_refuse_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let setup = Connection::open(&path).unwrap();
        setup
            .execute_batch("CREATE TABLE t (n); INSERT INTO t VALUES (1)")
            .unwrap();
        drop(setup);
        let req = Request::new("sqlite", facts(&path), Mode::Read, vec![]);
        let conn = connect(&req).unwrap();
        let readonly = |error: &rusqlite::Error| {
            assert_eq!(
                error.sqlite_error_code(),
                Some(rusqlite::ErrorCode::ReadOnly)
            );
        };
        readonly(&conn.execute("UPDATE t SET n=2", []).unwrap_err());
        // TEMP is writable even with mode=ro; query_only forbids it.
        readonly(&conn.execute("CREATE TEMP TABLE tmp (n)", []).unwrap_err());
        // The authorizer refuses to turn query_only off.
        conn.execute_batch("PRAGMA query_only=OFF").unwrap_err();
        readonly(&conn.execute("CREATE TEMP TABLE tmp (n)", []).unwrap_err());
        readonly(&conn.execute("UPDATE t SET n=2", []).unwrap_err());
        assert_eq!(
            conn.query_row("SELECT n FROM t", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn a_write_that_slipped_past_the_classifier_fails_at_the_server() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE t (n); INSERT INTO t VALUES (1)")
            .unwrap();
        let req = Request::new(
            "sqlite",
            facts(&path),
            Mode::Read,
            vec!["UPDATE t SET n = 2".into()],
        );
        let got = SqliteConnector.run(&req);
        assert!(!got.ok);
        assert_eq!(got.stage, Some(Stage::Execute));
        assert!(got.error.unwrap().contains("readonly"));
        let n: i64 = Connection::open(&path)
            .unwrap()
            .query_row("SELECT n FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    fn seeded(dir: &tempfile::TempDir) -> ConnFacts {
        let path = dir.path().join("t.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE t (n); INSERT INTO t VALUES (1)")
            .unwrap();
        facts(&path)
    }

    #[test]
    fn a_read_cannot_lift_query_only_or_attach_to_write_elsewhere() {
        let dir = tempfile::tempdir().unwrap();
        let conn = seeded(&dir);
        let run = |statements: &[&str]| {
            let statements = statements.iter().map(ToString::to_string).collect();
            SqliteConnector.run(&Request::new(
                "sqlite",
                conn.clone(),
                Mode::Read,
                statements,
            ))
        };
        let got = run(&[
            "PRAGMA query_only=OFF",
            "CREATE TEMP TABLE x (n)",
            "INSERT INTO x VALUES (1)",
        ]);
        assert!(!got.ok);
        assert_eq!(got.stage, Some(Stage::Execute));
        assert!(got.results.unwrap().is_empty(), "the first statement fails");
        for sql in ["ATTACH ':memory:' AS m", "DETACH DATABASE main"] {
            let got = run(&[sql]);
            assert!(!got.ok, "{sql}");
        }
        // Harmless reads of the pragma and its re-assertion still work.
        let got = run(&[
            "PRAGMA query_only",
            "PRAGMA query_only = ON",
            "SELECT n FROM t",
        ]);
        assert!(got.ok, "{:?}", got.error);
    }

    #[test]
    fn an_error_never_carries_the_statement_text() {
        let dir = tempfile::tempdir().unwrap();
        let conn = seeded(&dir);
        for sql in [
            "SELECT * FROM t WHERE n = 'sk-live-TOKEN123' 'sk-live-TOKEN123'",
            "SELECT 'sk-live-TOKEN123' FROM t WHERE FROM",
            "SELECT 'sk-live-TOKEN123' FROM nope_table",
            "SELECT 'sk-live-TOKEN123 FROM t",
            "SELECT sk_live_TOKEN123 FROM t",
        ] {
            for mode in [Mode::Read, Mode::Write] {
                let got = SqliteConnector.run(&Request::new(
                    "sqlite",
                    conn.clone(),
                    mode,
                    vec![sql.into()],
                ));
                let error = got.error.expect(sql);
                assert!(!got.ok);
                if !sql.contains("sk_live") {
                    assert!(!error.contains("TOKEN123"), "{sql}: {error}");
                }
                assert!(!error.contains("SELECT"), "{sql}: {error}");
            }
        }
    }

    #[test]
    fn missing_file_and_missing_path_are_connect_failures() {
        let dir = tempfile::tempdir().unwrap();
        let absent = dir.path().join("nope.db");
        let req = Request::new("sqlite", facts(&absent), Mode::Test, vec![]);
        let got = SqliteConnector.run(&req);
        assert_eq!((got.ok, got.stage), (false, Some(Stage::Connect)));
        let req = Request::new("sqlite", ConnFacts::default(), Mode::Test, vec![]);
        assert_eq!(
            SqliteConnector.run(&req).error.as_deref(),
            Some("missing conn.path")
        );
    }

    #[test]
    fn a_literal_question_mark_in_the_path_is_part_of_the_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a?mode=rw#x.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE t (n)")
            .unwrap();
        let req = Request::new("sqlite", facts(&path), Mode::Test, vec![]);
        assert!(SqliteConnector.run(&req).ok);
    }

    const PW: &str = "pw-Sq1ite-S3cret";
    const USER: &str = "svc_sqlite_user";
    const HOST: &str = "db.sqlite-host.example";

    /// Facts with a password, user and host that must never appear in any output.
    fn secret_facts(path: &std::path::Path) -> ConnFacts {
        ConnFacts {
            password: Some(PW.into()),
            user: Some(USER.into()),
            host: Some(HOST.into()),
            ..facts(path)
        }
    }

    fn run_one(conn: &ConnFacts, mode: Mode, sql: &[&str]) -> ExecResult {
        SqliteConnector.run(&Request::new(
            "sqlite",
            conn.clone(),
            mode,
            sql.iter().map(ToString::to_string).collect(),
        ))
    }

    /// A refusal: `stage`, the error contains `needle`, and neither a credential nor the path shows.
    fn assert_refused(got: &ExecResult, stage: Stage, needle: &str, path: &std::path::Path) {
        assert!(!got.ok, "{got:?}");
        assert_eq!(got.stage, Some(stage), "{got:?}");
        let line = got.to_line();
        assert!(
            got.error.as_deref().unwrap_or("").contains(needle),
            "{line}"
        );
        for secret in [PW, USER, HOST] {
            assert!(!line.contains(secret), "{secret} leaked: {line}");
        }
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(!line.contains(name.as_ref()), "the path leaked: {line}");
    }

    #[test]
    fn a_missing_file_is_refused_in_every_mode_and_never_created() {
        let dir = tempfile::tempdir().unwrap();
        let absent = dir.path().join("absent-secret-name.db");
        for mode in [Mode::Read, Mode::Write, Mode::Test] {
            let got = run_one(&secret_facts(&absent), mode, &["CREATE TABLE t (n)"]);
            assert_refused(&got, Stage::Connect, "unable to open", &absent);
            assert!(!absent.exists(), "{mode:?} created the file");
        }
        let got = run_one(
            &ConnFacts {
                path: Some(String::new()),
                ..ConnFacts::default()
            },
            Mode::Test,
            &[],
        );
        assert_eq!(got.error.as_deref(), Some("missing conn.path"));
    }

    #[test]
    fn a_directory_is_not_a_database_file() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("a-directory-name");
        std::fs::create_dir(&sub).unwrap();
        for mode in [Mode::Read, Mode::Write] {
            let got = run_one(&secret_facts(&sub), mode, &["SELECT 1"]);
            assert_refused(&got, Stage::Connect, "not a regular file", &sub);
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_fifo_is_refused_at_once_instead_of_blocking_the_open() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("a-fifo-name.db");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(made.success());
        let started = Instant::now();
        for mode in [Mode::Read, Mode::Test, Mode::Write] {
            let got = run_one(&secret_facts(&fifo), mode, &["SELECT 1"]);
            assert_refused(&got, Stage::Connect, "not a regular file", &fifo);
        }
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[cfg(unix)]
    #[test]
    fn a_write_never_follows_a_symbolic_link_and_a_read_may() {
        let dir = tempfile::tempdir().unwrap();
        let real = seeded(&dir);
        let link = dir.path().join("a-link-name.db");
        std::os::unix::fs::symlink(real.path.as_deref().unwrap(), &link).unwrap();
        let got = run_one(&secret_facts(&link), Mode::Write, &["DELETE FROM t"]);
        assert_refused(&got, Stage::Connect, "symbolic link", &link);
        let got = run_one(&secret_facts(&link), Mode::Read, &["SELECT n FROM t"]);
        assert!(got.ok, "{got:?}");
        assert_eq!(
            got.results.unwrap()[0].rows,
            Some(vec![vec![Value::from(1)]])
        );
        let n: i64 = Connection::open(real.path.as_deref().unwrap())
            .unwrap()
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "the refused write changed nothing");
    }

    #[test]
    fn a_bad_timeout_on_a_direct_execute_is_refused_not_defaulted() {
        let dir = tempfile::tempdir().unwrap();
        let conn = seeded(&dir);
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, 3601.0] {
            let mut req = Request::new("sqlite", conn.clone(), Mode::Read, vec!["SELECT 1".into()]);
            req.timeout = bad;
            let got = SqliteConnector.execute(&req);
            assert_eq!(got.stage, Some(Stage::Connect), "{bad}");
            assert_eq!(got.error.as_deref(), Some(request::BAD_TIMEOUT));
        }
    }

    #[test]
    fn a_statement_running_past_the_timeout_is_interrupted() {
        let dir = tempfile::tempdir().unwrap();
        let conn = seeded(&dir);
        let mut req = Request::new(
            "sqlite",
            conn,
            Mode::Read,
            vec![
                "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c) SELECT COUNT(*) FROM c"
                    .into(),
            ],
        );
        req.timeout = 0.2;
        let started = Instant::now();
        let got = SqliteConnector.run(&req);
        assert_eq!(got.stage, Some(Stage::Execute), "{got:?}");
        assert!(got.error.unwrap().contains("interrupt"));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn row_counts_select_minus_one_ddl_zero_dml_changes_returning_zero() {
        let dir = tempfile::tempdir().unwrap();
        let conn = seeded(&dir);
        let got = run_one(
            &conn,
            Mode::Write,
            &[
                "INSERT INTO t VALUES (2), (3)",
                "UPDATE t SET n = n + 10 RETURNING n",
                "DELETE FROM t WHERE n = 11 RETURNING n",
                "CREATE TABLE u (x)",
                "PRAGMA user_version = 7",
                "WITH x AS (SELECT 1) DELETE FROM t WHERE n = 12",
                "DELETE FROM t WHERE n = 99",
                "SELECT n FROM t",
            ],
        );
        assert!(got.ok, "{got:?}");
        let counts: Vec<i64> = got.results.unwrap().iter().map(|r| r.rowcount).collect();
        assert_eq!(counts, [2, 0, 0, 0, 0, 1, 0, -1]);
        // A writing PRAGMA that returns rows is not a SELECT: 0 (D-001), and its row comes back.
        let got = run_one(&conn, Mode::Write, &["PRAGMA journal_mode = DELETE"]);
        let one = &got.results.unwrap()[0];
        assert_eq!(
            (one.rowcount, one.rows.as_ref().map(Vec::len)),
            (0, Some(1))
        );
    }

    #[test]
    fn invalid_utf8_text_is_replaced_and_marked_valid_text_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let conn = seeded(&dir);
        let got = run_one(
            &conn,
            Mode::Read,
            &["SELECT CAST(x'636166e920ff' AS TEXT), 'Ana \u{FFFD}', x'00ff'"],
        );
        assert!(got.ok, "{got:?}");
        let row = &got.results.unwrap()[0].rows.clone().unwrap()[0];
        assert_eq!(
            row[0],
            Value::from(format!(
                "caf\u{FFFD} \u{FFFD}{}",
                request::INVALID_UTF8_MARK
            ))
        );
        assert_eq!(row[1], Value::from("Ana \u{FFFD}"));
        assert_eq!(row[2], Value::from("<2 bytes>"));
    }

    #[test]
    fn a_column_name_that_is_not_utf8_is_an_execute_failure_not_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crafted.db");
        let setup = Connection::open(&path).unwrap();
        setup.execute_batch("CREATE TABLE t (ab)").unwrap();
        // A crafted schema: the column name holds the byte 0xff.
        setup
            .execute_batch(
                "PRAGMA writable_schema = ON;
                 UPDATE sqlite_master SET sql = CAST(x'435245415445205441424c4520742028ff6229' AS TEXT) WHERE name = 't';
                 PRAGMA writable_schema = OFF;",
            )
            .unwrap();
        drop(setup);
        // A write has no authorizer: the name reaches rusqlite's column name, which would panic.
        let got = run_one(&secret_facts(&path), Mode::Write, &["SELECT * FROM t"]);
        assert_refused(&got, Stage::Execute, "not valid UTF-8", &path);
        // A read is stopped earlier: the authorizer callback cannot take the name and denies it.
        let got = run_one(&secret_facts(&path), Mode::Read, &["SELECT * FROM t"]);
        assert_refused(&got, Stage::Execute, "prohibited", &path);
    }

    fn rows_of(path: &std::path::Path) -> i64 {
        Connection::open(path)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn copy_database_is_bounded_by_a_lock_and_leaves_the_target_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let from = seeded(&dir);
        let to_path = dir.path().join("to.db");
        Connection::open(&to_path)
            .unwrap()
            .execute_batch("CREATE TABLE t (n); INSERT INTO t VALUES (1), (2), (3)")
            .unwrap();
        // Another connection holds the target exclusively: the copy gives up at the deadline.
        let holder = Connection::open(&to_path).unwrap();
        holder.execute_batch("BEGIN EXCLUSIVE").unwrap();
        let a = Connection::open(from.path.as_deref().unwrap()).unwrap();
        let mut b = Connection::open(&to_path).unwrap();
        let started = Instant::now();
        let err = copy_database(&a, &mut b, Duration::from_millis(300), u64::MAX).unwrap_err();
        assert!(err.contains("did not finish in time"), "{err}");
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "{:?}",
            started.elapsed()
        );
        holder.execute_batch("ROLLBACK").unwrap();
        drop(b);
        assert_eq!(rows_of(&to_path), 3, "the target is untouched");
        // Over the size cap: refused, the target untouched.
        let mut b = Connection::open(&to_path).unwrap();
        let err = copy_database(&a, &mut b, Duration::from_secs(5), 512).unwrap_err();
        assert!(err.contains("size cap"), "{err}");
        drop(b);
        assert_eq!(rows_of(&to_path), 3);
        // Free: the copy goes through.
        let mut b = Connection::open(&to_path).unwrap();
        copy_database(&a, &mut b, Duration::from_secs(5), u64::MAX).unwrap();
        drop(b);
        assert_eq!(rows_of(&to_path), 1);
    }

    #[test]
    fn a_20_mib_text_is_cut_and_marked_and_a_value_past_sqlites_limit_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.db");
        let setup = Connection::open(&path).unwrap();
        setup
            .execute_batch(
                "CREATE TABLE b (v);
                 INSERT INTO b VALUES (CAST(zeroblob(20 * 1024 * 1024) AS TEXT) || 'end');
                 INSERT INTO b VALUES (zeroblob(40 * 1024 * 1024));",
            )
            .unwrap();
        drop(setup);
        let conn = secret_facts(&path);
        let got = run_one(&conn, Mode::Read, &["SELECT v FROM b WHERE rowid = 1"]);
        assert!(got.ok, "{:?}", got.error);
        let rows = got.results.unwrap()[0].rows.clone().unwrap();
        let text = rows[0][0].as_str().unwrap();
        assert!(
            text.len() <= request::MAX_VALUE_BYTES + 64,
            "{}",
            text.len()
        );
        assert!(
            text.ends_with(&format!(
                " <truncated: {} bytes in all>",
                20 * 1024 * 1024 + 3
            )),
            "{}",
            &text[text.len() - 60..]
        );
        // 40 MiB is past SQLite's own per-value limit of this connector: refused, not allocated.
        let got = run_one(&conn, Mode::Read, &["SELECT v FROM b WHERE rowid = 2"]);
        assert_refused(&got, Stage::Execute, "too big", &path);
    }

    #[test]
    fn a_result_past_16_mib_of_values_fails_instead_of_growing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("many.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE m (v);
                 WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 20)
                 INSERT INTO m SELECT CAST(zeroblob(1024 * 1024) AS TEXT) FROM c;",
            )
            .unwrap();
        let conn = secret_facts(&path);
        let got = run_one(&conn, Mode::Read, &["SELECT v FROM m"]);
        assert_refused(&got, Stage::Execute, "larger than 16 MiB", &path);
        // Under the budget, part of the same table is fine.
        let got = run_one(&conn, Mode::Read, &["SELECT v FROM m LIMIT 10"]);
        assert!(got.ok, "{:?}", got.error);
    }

    #[cfg(unix)]
    #[test]
    fn a_proven_file_replaced_before_sqlite_opens_it_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let conn = seeded(&dir);
        let path = std::path::PathBuf::from(conn.path.clone().unwrap());
        let proof = open_checked(&path, false).unwrap();
        assert!(proof.still_at(&path, false).is_ok());
        // Another regular file under the same name: a different inode.
        std::fs::rename(&path, dir.path().join("moved.db")).unwrap();
        std::fs::write(&path, b"").unwrap();
        assert_eq!(proof.still_at(&path, false).unwrap_err(), CHANGED);
        // A link under the same name.
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(dir.path().join("moved.db"), &path).unwrap();
        assert_eq!(proof.still_at(&path, false).unwrap_err(), NOT_A_LINK);
        assert_eq!(open_checked(&path, false).unwrap_err(), NOT_A_LINK);
    }
}
