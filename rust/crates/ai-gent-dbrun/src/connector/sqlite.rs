//! The SQLite connector (feature `sqlite`): rusqlite with bundled SQLite, no system library.
//!
//! A read opens the file with `mode=ro` and sets `query_only`: the server itself refuses a write the
//! classifier missed (`mode=ro` blocks the main file, `query_only` blocks TEMP). A write runs one
//! transaction and commits it only when the row counts match `expect`.
use super::{Connector, ExecResult, Mode, Request, Stage, StatementResult};
use crate::core::explain_sql;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::time::{Duration, Instant};

/// Lock wait, as `LOCK_TIMEOUT_MS` in `dbrun.py`.
const LOCK_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

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

fn connect(req: &Request) -> Result<Connection, String> {
    let path = req.conn.path.as_deref().ok_or("missing conn.path")?;
    // Reserved URI characters are percent-encoded so a literal file name cannot add parameters.
    let encoded: String = path
        .bytes()
        .map(|b| match b {
            b'%' | b'?' | b'#' => format!("%{b:02X}"),
            _ if !b.is_ascii() => format!("%{b:02X}"),
            _ => char::from(b).to_string(),
        })
        .collect();
    let read = req.mode != Mode::Write;
    let flags = if read {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    };
    let uri = format!("file:{encoded}{}", if read { "?mode=ro" } else { "" });
    let mut conn = Connection::open_with_flags(uri, flags | OpenFlags::SQLITE_OPEN_URI)
        .map_err(|e| format!("sqlite: {}", describe(&e)))?;
    let setup = |conn: &mut Connection| -> rusqlite::Result<()> {
        conn.busy_timeout(LOCK_TIMEOUT)?;
        // A request timeout that is not a finite positive number falls back to the default.
        let timeout = Duration::try_from_secs_f64(req.timeout)
            .ok()
            .filter(|d| !d.is_zero())
            .unwrap_or(DEFAULT_TIMEOUT);
        let deadline = Instant::now().checked_add(timeout);
        conn.progress_handler(
            10_000,
            Some(move || deadline.is_some_and(|d| Instant::now() > d)),
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

/// Python's `sqlite3` counts changes only for statements that begin with INSERT, UPDATE, DELETE or
/// REPLACE (after whitespace and comments).
fn is_dml(sql: &str) -> bool {
    let mut rest = sql;
    loop {
        rest = rest.trim_start();
        if let Some(r) = rest.strip_prefix("--") {
            rest = r.split_once('\n').map_or("", |(_, tail)| tail);
        } else if let Some(r) = rest.strip_prefix("/*") {
            rest = r.split_once("*/").map_or("", |(_, tail)| tail);
        } else {
            break;
        }
    }
    let word: String = rest
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .collect::<String>()
        .to_ascii_uppercase();
    matches!(word.as_str(), "INSERT" | "UPDATE" | "DELETE" | "REPLACE")
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
        let one = run_statement(conn, &sql, read, req.max_rows)?;
        result.results.get_or_insert_with(Vec::new).push(one);
    }
    if read {
        return Ok(());
    }
    let results = result.results.as_deref().unwrap_or_default();
    let mismatches: Vec<String> = req
        .expect
        .iter()
        .enumerate()
        .filter_map(|(i, want)| {
            let (want, got) = ((*want)?, results.get(i)?.rowcount);
            (want != got).then(|| format!("statement {}: expected {want}, got {got}", i + 1))
        })
        .collect();
    if mismatches.is_empty() {
        conn.execute_batch("COMMIT")?;
        result.committed = Some(true);
    } else {
        conn.execute_batch("ROLLBACK")?;
        result.ok = false;
        result.stage = Some(Stage::Verify);
        result.error = Some(format!(
            "row counts differ from the plan: {}",
            mismatches.join(", ")
        ));
    }
    Ok(())
}

fn run_statement(
    conn: &Connection,
    sql: &str,
    read: bool,
    cap: usize,
) -> rusqlite::Result<StatementResult> {
    let mut statement = conn.prepare(sql)?;
    let dml = is_dml(sql);
    if statement.column_count() == 0 {
        let n = statement.execute([])?;
        // Like Python: DML reports its changes, anything else (DDL, PRAGMA, ...) -1; `changes()` of
        // an earlier statement is never carried over.
        return Ok(StatementResult {
            rowcount: if dml {
                i64::try_from(n).unwrap_or(i64::MAX)
            } else {
                -1
            },
            columns: None,
            rows: None,
            truncated: None,
        });
    }
    let columns: Vec<String> = statement.column_names().map(String::from).collect();
    let mut query = statement.query([])?;
    let mut rows: Vec<Vec<Value>> = Vec::new();
    let mut truncated = false;
    while let Some(row) = query.next()? {
        if read && rows.len() == cap {
            truncated = true;
            break;
        }
        let mut values = Vec::with_capacity(columns.len());
        for i in 0..columns.len() {
            values.push(match row.get_ref(i)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(v) => Value::from(v),
                ValueRef::Real(v) => Value::from(v),
                ValueRef::Text(v) => Value::from(String::from_utf8_lossy(v).into_owned()),
                ValueRef::Blob(v) => Value::from(format!("<{} bytes>", v.len())),
            });
        }
        rows.push(values);
    }
    // Only a read is capped; a write returns everything its statement produced, like the Python runner
    // (which slices to `max_rows` in both cases).
    rows.truncate(cap);
    // Python reads `rowcount` before fetching: a row-producing DML (RETURNING) is still 0 then.
    Ok(StatementResult {
        rowcount: if dml { 0 } else { -1 },
        columns: Some(columns),
        rows: Some(rows),
        truncated: Some(truncated),
    })
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
}
