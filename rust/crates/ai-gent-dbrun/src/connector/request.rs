//! Shared request handling for every connector (spec 043 FR-010, FR-011, FR-012, T005).
//!
//! - [`timeout`]: the request's timeout as a [`Duration`], or the refusal. Missing means 30 s (the
//!   request default); zero, negative, non-finite, non-numeric, below 1 ms or above one hour is a
//!   `connect`-stage failure, never a panic.
//! - [`validate`]: what [`super::Connector::run`] checks before any driver is touched: the timeout,
//!   `max_rows`, and the connection fields of an engine that builds a connect string.
//! - [`cap_rows`]: the `max_rows` and `truncated` rule. Read mode keeps `max_rows` rows and is
//!   `truncated` when one more existed; write mode keeps `max_rows` rows, drains the rest and is never
//!   `truncated` (as `dbrun.py`: `fetchmany(max_rows + 1)` in read mode, `fetchall()` in write mode,
//!   both sliced to `max_rows`).
//! - [`rowcount`]: the row count of a statement, always the server's own (never read from the
//!   statement text, PostgreSQL review finding 1): rows returned `-1`, DDL `0`, DML the server count.
//! - [`verify_counts`]: the write-mode check of the counts against `expect`, with the Python message.
use super::{ExecResult, Request, Stage};
use crate::core::fields;
use std::time::Duration;

/// Timeout used when a request names none.
pub const DEFAULT_TIMEOUT_SECS: f64 = 30.0;
/// Shortest accepted timeout; below it a millisecond-based engine setting would read 0 (no limit).
pub const MIN_TIMEOUT_SECS: f64 = 0.001;
/// Longest accepted timeout (one hour).
pub const MAX_TIMEOUT_SECS: f64 = 3600.0;
/// Largest accepted `max_rows`.
pub const MAX_ROWS_LIMIT: usize = 1_000_000;
/// Longest request line [`super::Connector::run_line`] reads (16 MiB).
pub const MAX_REQUEST_LINE: usize = 16 * 1024 * 1024;
/// Engines that build a connect string or descriptor from profile fields and so need
/// [`fields::validate_conn`] (FR-009). An engine that passes fields as driver arguments does not.
pub const STRING_BUILDING_ENGINES: &[&str] = &["oracle"];

/// Row count of a statement that returns rows (SELECT, `SHOW`, `EXPLAIN`).
pub const ROWS_ROWCOUNT: i64 = -1;
/// Row count of DDL: `0` (an intended change; Python's drivers report `-1`).
pub const DDL_ROWCOUNT: i64 = 0;

/// The fixed text of a timeout refusal.
pub const BAD_TIMEOUT: &str = "invalid timeout: it must be a number of seconds from 0.001 to 3600";

/// The request timeout as a duration.
///
/// # Errors
/// [`BAD_TIMEOUT`] for NaN, infinite, zero, negative, below 1 ms or above one hour.
pub fn timeout(secs: f64) -> Result<Duration, &'static str> {
    if !secs.is_finite() || !(MIN_TIMEOUT_SECS..=MAX_TIMEOUT_SECS).contains(&secs) {
        return Err(BAD_TIMEOUT);
    }
    Duration::try_from_secs_f64(secs).map_err(|_| BAD_TIMEOUT)
}

/// Checks a request before a connector sees it. The message is fixed text, no request value.
///
/// # Errors
/// The `connect`-stage message: bad timeout, `max_rows` above [`MAX_ROWS_LIMIT`], or a connection
/// field of a string-building engine that fails [`fields::validate_conn`].
pub fn validate(req: &Request) -> Result<(), String> {
    timeout(req.timeout).map_err(String::from)?;
    if req.max_rows > MAX_ROWS_LIMIT {
        return Err(format!(
            "invalid max_rows: it must be at most {MAX_ROWS_LIMIT}"
        ));
    }
    if STRING_BUILDING_ENGINES.contains(&req.engine.as_str()) {
        fields::validate_conn(&req.conn).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// [`validate`] as the failure result a connector returns.
#[must_use]
pub fn refusal(message: String) -> ExecResult {
    ExecResult::failure(Stage::Connect, message)
}

/// Keeps the rows a statement result may show. Returns `(rows, truncated)`.
///
/// Read mode pulls at most `max_rows + 1` items from `rows` (it never drains a huge result) and is
/// `truncated` when the extra one arrived. Write mode drains the iterator, keeps the first `max_rows`
/// and is never `truncated`.
pub fn cap_rows<T>(
    rows: impl IntoIterator<Item = T>,
    read: bool,
    max_rows: usize,
) -> (Vec<T>, bool) {
    let mut kept = Vec::new();
    let mut truncated = false;
    for row in rows {
        if kept.len() < max_rows {
            kept.push(row);
        } else if read {
            truncated = true;
            break;
        }
    }
    (kept, truncated)
}

/// What a statement is, for its row count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountKind {
    /// Returns rows: `-1`.
    Rows,
    /// DDL and other statements with no row count: `0`.
    Ddl,
    /// DML: the count the server reported.
    Dml,
}

/// The row count of a statement. `server_count` is what the server or driver reported for it
/// (`CommandComplete`, `OK` packet, `DONE` token, `changes()`); it is never derived from SQL text.
///
/// # Errors
/// A DML statement for which the server gave no (or a negative) count: refusing is safer than
/// guessing, since `expect` verification trusts this number.
pub fn rowcount(kind: CountKind, server_count: Option<i64>) -> Result<i64, &'static str> {
    match kind {
        CountKind::Rows => Ok(ROWS_ROWCOUNT),
        CountKind::Ddl => Ok(DDL_ROWCOUNT),
        CountKind::Dml => match server_count {
            Some(n) if n >= 0 => Ok(n),
            _ => Err("the server gave no row count for a data-changing statement"),
        },
    }
}

/// Compares a write's row counts with the plan's `expect`; `None` skips a statement. Returns the
/// failure message (stage `verify`) or `None` when every checked statement matches. An `expect`
/// entry with no statement behind it is ignored, as the Python runner does (listed in
/// `differences.md` as a question).
#[must_use]
pub fn verify_counts(expect: &[Option<i64>], counts: &[i64]) -> Option<String> {
    let mismatches: Vec<String> = expect
        .iter()
        .enumerate()
        .filter_map(|(i, want)| {
            let (want, got) = ((*want)?, *counts.get(i)?);
            (want != got).then(|| format!("statement {}: expected {want}, got {got}", i + 1))
        })
        .collect();
    (!mismatches.is_empty())
        .then(|| format!("row counts differ from the plan: {}", mismatches.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connector::{ConnFacts, Mode};

    const PASSWORD: &str = "pw-Sup3rS3cret";

    #[test]
    fn a_good_timeout_is_kept_and_the_default_is_thirty() {
        assert_eq!(timeout(5.0), Ok(Duration::from_secs(5)));
        assert_eq!(timeout(0.001), Ok(Duration::from_millis(1)));
        assert_eq!(timeout(3600.0), Ok(Duration::from_secs(3600)));
        let req = Request::new("sqlite", ConnFacts::default(), Mode::Test, vec![]);
        assert!((req.timeout - 30.0).abs() < f64::EPSILON);
        let parsed: Request = serde_json::from_str(r#"{"engine":"sqlite","mode":"test"}"#).unwrap();
        assert!((parsed.timeout - DEFAULT_TIMEOUT_SECS).abs() < f64::EPSILON);
    }

    #[test]
    fn a_bad_timeout_is_refused_never_a_panic() {
        for bad in [
            0.0,
            -0.0,
            -1.0,
            -1e308,
            0.0009,
            f64::MIN_POSITIVE,
            3600.001,
            1e9,
            f64::MAX,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            assert_eq!(timeout(bad), Err(BAD_TIMEOUT), "{bad}");
        }
    }

    #[test]
    fn a_non_numeric_timeout_in_a_request_line_is_a_connect_failure_through_run() {
        struct Never;
        impl crate::connector::Connector for Never {
            fn engines(&self) -> &'static [&'static str] {
                &["postgresql"]
            }
            fn execute(&self, _: &Request) -> ExecResult {
                panic!("a request with a bad timeout reached the driver");
            }
        }
        use crate::connector::Connector;
        for raw in [
            r#""5""#,
            "null",
            "true",
            "[5]",
            r#"{"a":1}"#,
            "0",
            "-3",
            "1e999",
            "1e300",
            "-1e300",
            "99999999999999999999999999999",
        ] {
            let line = format!(
                r#"{{"engine":"postgresql","mode":"test","timeout":{raw},"conn":{{"password":"{PASSWORD}"}}}}"#
            );
            let out = Never.run_line(&line);
            let v: serde_json::Value = serde_json::from_str(&out).unwrap();
            assert_eq!(v["ok"], false, "{raw}: {out}");
            assert!(!out.contains(PASSWORD), "{raw}: {out}");
            assert!(!out.contains('\n'));
            // `1e999` is out of range for any JSON number: refused while the line is parsed.
            let stage = v["stage"].as_str().unwrap();
            let expected = if raw == "1e999" { "request" } else { "connect" };
            assert_eq!(stage, expected, "{raw}: {out}");
        }
    }

    #[test]
    fn max_rows_above_the_limit_and_oversized_lines_are_refused() {
        let mut req = Request::new("sqlite", ConnFacts::default(), Mode::Read, vec![]);
        req.max_rows = MAX_ROWS_LIMIT;
        assert!(validate(&req).is_ok());
        req.max_rows = MAX_ROWS_LIMIT + 1;
        assert!(validate(&req).unwrap_err().contains("max_rows"));
        req.max_rows = usize::MAX;
        assert!(validate(&req).is_err());
    }

    #[test]
    fn oracle_fields_are_validated_and_the_message_has_no_value() {
        let conn = ConnFacts {
            host: Some("db.internal".into()),
            service: Some(format!("x)(ADDRESS=(HOST={PASSWORD})")),
            password: Some(PASSWORD.into()),
            ..ConnFacts::default()
        };
        let req = Request::new("oracle", conn.clone(), Mode::Test, vec![]);
        let err = validate(&req).unwrap_err();
        assert!(err.contains("service"), "{err}");
        assert!(!err.contains(PASSWORD) && !err.contains("ADDRESS"), "{err}");
        // An engine that passes fields as driver arguments is not restricted by the descriptor rule.
        let req = Request::new("postgresql", conn, Mode::Test, vec![]);
        assert!(validate(&req).is_ok());
    }

    #[test]
    fn read_mode_caps_at_max_rows_plus_one_and_flags_truncation() {
        let pulled = std::cell::Cell::new(0);
        let source = (0..1_000_000).inspect(|_| pulled.set(pulled.get() + 1));
        let (rows, truncated) = cap_rows(source, true, 2);
        assert_eq!((rows, truncated), (vec![0, 1], true));
        assert_eq!(pulled.get(), 3, "a read never drains the result");
        assert_eq!(cap_rows(0..2, true, 2), (vec![0, 1], false));
        assert_eq!(cap_rows(0..3, true, 2), (vec![0, 1], true));
        assert_eq!(cap_rows(0..0, true, 0), (vec![], false));
        assert_eq!(cap_rows(0..1, true, 0), (vec![], true));
    }

    #[test]
    fn write_mode_keeps_max_rows_and_is_never_truncated() {
        assert_eq!(cap_rows(0..5, false, 2), (vec![0, 1], false));
        assert_eq!(cap_rows(0..5, false, 0), (vec![], false));
        assert_eq!(cap_rows(0..0, false, 3), (vec![], false));
    }

    #[test]
    fn row_counts_follow_the_rule_and_a_missing_dml_count_is_refused() {
        assert_eq!(rowcount(CountKind::Rows, Some(7)), Ok(-1));
        assert_eq!(rowcount(CountKind::Rows, None), Ok(-1));
        assert_eq!(rowcount(CountKind::Ddl, Some(5)), Ok(0));
        assert_eq!(rowcount(CountKind::Ddl, None), Ok(0));
        assert_eq!(rowcount(CountKind::Dml, Some(0)), Ok(0));
        assert_eq!(rowcount(CountKind::Dml, Some(42)), Ok(42));
        assert!(rowcount(CountKind::Dml, None).is_err());
        assert!(rowcount(CountKind::Dml, Some(-1)).is_err());
    }

    #[test]
    fn verify_counts_matches_the_python_rule() {
        // Equal: nothing to report.
        assert_eq!(verify_counts(&[Some(2), Some(0)], &[2, 0]), None);
        // `null` skips; the message numbers statements from 1 and lists every mismatch.
        assert_eq!(
            verify_counts(&[None, Some(1), Some(9)], &[5, 0, 3]).as_deref(),
            Some(
                "row counts differ from the plan: statement 2: expected 1, got 0, statement 3: expected 9, got 3"
            )
        );
        // Python: an `expect` entry past the statements, and no `expect` at all, pass.
        assert_eq!(verify_counts(&[Some(1), Some(1)], &[1]), None);
        assert_eq!(verify_counts(&[], &[1, 2]), None);
        // -1 against an expected count is a mismatch, not a pass.
        assert!(verify_counts(&[Some(1)], &[-1]).is_some());
    }
}
