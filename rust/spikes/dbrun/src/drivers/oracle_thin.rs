//! E6b: Oracle through `oracledb` 26.0.0-beta.5, Oracle's own pure Rust thin driver (no Oracle Client, no
//! ODPI-C). Synchronous API, so no async runtime.
//!
//! Python still owns policy, plans, backups and restore; this executes one `_exec` request, mirroring the
//! Oracle branch of `dbrun.py`: identity and database-link probes, `SET TRANSACTION READ ONLY` in read mode
//! (not in `--explain`, which writes `PLAN_TABLE` and is local only), rollback after every read, one
//! transaction for apply with the plan's row counts checked before the commit.
//! Differences from `dbrun.py` are listed in `specs/038-db-runner-spike/results.md` (E6).
use crate::core::redact;
use oracledb::{Config, Connection, Error as OraError, Row};
use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

const MAX_TIMEOUT_SECS: f64 = 86_400.0;
const IDENTITY: &str = "SELECT SYS_CONTEXT('USERENV','DB_NAME'), SYS_CONTEXT('USERENV','SERVER_HOST'), SYS_CONTEXT('USERENV','CON_NAME') FROM DUAL";
const LINKS: &str = "SELECT COUNT(*) FROM ALL_DB_LINKS";
const PLAN: &str = "SELECT PLAN_TABLE_OUTPUT FROM TABLE(DBMS_XPLAN.DISPLAY())";

/// The request's timeout in seconds, as in `mssql.rs`: absent is 30, a finite value of zero or more is
/// accepted (capped at one day), anything else is an invalid request.
fn timeout_secs(req: &Value) -> Result<f64, String> {
    match &req["timeout"] {
        Value::Null => Ok(30.0),
        v => match v.as_f64() {
            Some(t) if t.is_finite() && t >= 0.0 => Ok(t.min(MAX_TIMEOUT_SECS)),
            _ => Err("ValueError: invalid timeout".to_string()),
        },
    }
}

fn text(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::to_owned)
        .or_else(|| v.as_u64().map(|n| n.to_string()))
}

/// `oracle_dsn` of `dbrun.py` (service name from SERVICE, else DATABASE, else the SID), always written as a
/// connect descriptor with `RETRY_DELAY=0`. The thin client sleeps its default one second after a failed
/// attempt even when no retry follows, so every refused connection would cost a second that
/// `python-oracledb` does not spend; the setting is honoured in a descriptor but silently ignored as an
/// Easy Connect `?retry_delay=0` parameter (measured, E6).
fn dsn(c: &Value) -> String {
    let host = text(&c["host"]).unwrap_or_default();
    let port = text(&c["port"])
        .filter(|p| !p.is_empty() && p != "0")
        .unwrap_or_else(|| "1521".into());
    let service = text(&c["service"])
        .filter(|s| !s.is_empty())
        .or_else(|| text(&c["database"]).filter(|s| !s.is_empty()));
    let target = match service {
        Some(s) => format!("SERVICE_NAME={s}"),
        None => format!("SID={}", text(&c["sid"]).unwrap_or_default()),
    };
    format!(
        "(DESCRIPTION=(RETRY_DELAY=0)(ADDRESS=(PROTOCOL=TCP)(HOST={host})(PORT={port}))(CONNECT_DATA=({target})))"
    )
}

/// An Oracle number as Python's `oracledb` hands it back with the default settings: an `int` when the text
/// has no decimal point, a `float` otherwise. An integer beyond 64 bits cannot be a JSON number here, so it
/// stays its digits as a string (Python would print them as a bare number).
fn number_json(s: &str) -> Value {
    if !s.contains('.') {
        if let Ok(n) = s.parse::<i64>() {
            return json!(n);
        }
        if let Ok(n) = s.parse::<u64>() {
            return json!(n);
        }
        return json!(s);
    }
    match s.parse::<f64>() {
        Ok(f) if f.is_finite() => json!(f),
        _ => json!(s),
    }
}

/// Python `datetime.isoformat()`: `.ffffff` only when the microseconds are not zero; nanoseconds below a
/// microsecond are dropped, as a Python `datetime` cannot hold them.
fn timestamp_text(y: i16, mo: u8, d: u8, h: u8, mi: u8, s: u8, nanos: u32) -> String {
    let micros = nanos / 1000;
    let base = format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}");
    if micros == 0 {
        base
    } else {
        format!("{base}.{micros:06}")
    }
}

/// `str(datetime.timedelta)` for an INTERVAL DAY TO SECOND: Python normalises to a signed day count and a
/// non-negative remainder (`-1 day, 23:59:59`), writes `1 day` or `N days`, and `.ffffff` only when needed.
fn interval_ds_text(days: i32, hours: i8, minutes: i8, seconds: i8, nanos: i32) -> String {
    let micros_per_day: i128 = 86_400_000_000;
    let total: i128 = i128::from(days) * micros_per_day
        + i128::from(hours) * 3_600_000_000
        + i128::from(minutes) * 60_000_000
        + i128::from(seconds) * 1_000_000
        + i128::from(nanos) / 1000;
    let d = total.div_euclid(micros_per_day);
    let rem = total.rem_euclid(micros_per_day);
    let (secs, micros) = (rem / 1_000_000, rem % 1_000_000);
    let clock = format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
    let clock = if micros == 0 {
        clock
    } else {
        format!("{clock}.{micros:06}")
    };
    match d {
        0 => clock,
        1 | -1 => format!("{d} day, {clock}"),
        _ => format!("{d} days, {clock}"),
    }
}

fn float_json(n: f64) -> Value {
    if n.is_finite() {
        json!(n)
    } else {
        json!(n.to_string())
    }
}

/// One cell as `to_jsonable` renders what python-oracledb returns, picked by the column's declared type.
fn cell(row: &Row, i: usize, ty: &str) -> Result<Value, OraError> {
    Ok(match ty {
        "DB_TYPE_NUMBER" => row
            .get::<Option<oracledb::OracleNumber>>(i)?
            .map_or(Value::Null, |n| number_json(&n.to_string())),
        "DB_TYPE_BINARY_DOUBLE" => row.get::<Option<f64>>(i)?.map_or(Value::Null, float_json),
        "DB_TYPE_BINARY_FLOAT" => row
            .get::<Option<f32>>(i)?
            .map_or(Value::Null, |n| float_json(f64::from(n))),
        "DB_TYPE_CHAR"
        | "DB_TYPE_NCHAR"
        | "DB_TYPE_VARCHAR"
        | "DB_TYPE_NVARCHAR"
        | "DB_TYPE_LONG"
        | "DB_TYPE_LONG_NVARCHAR"
        | "DB_TYPE_ROWID"
        | "DB_TYPE_UROWID" => row
            .get::<Option<String>>(i)?
            .map_or(Value::Null, |s| json!(s)),
        "DB_TYPE_RAW" | "DB_TYPE_LONG_RAW" => row
            .get::<Option<Vec<u8>>>(i)?
            .map_or(Value::Null, |b| json!(format!("<{} bytes>", b.len()))),
        "DB_TYPE_DATE" | "DB_TYPE_TIMESTAMP" | "DB_TYPE_TIMESTAMP_LTZ" | "DB_TYPE_TIMESTAMP_TZ" => {
            row.get::<Option<oracledb::OracleTimestamp>>(i)?
                .map_or(Value::Null, |t| {
                    json!(timestamp_text(
                        t.year(),
                        t.month(),
                        t.day(),
                        t.hour(),
                        t.minute(),
                        t.second(),
                        t.nanoseconds()
                    ))
                })
        }
        "DB_TYPE_INTERVAL_DS" => {
            row.get::<Option<oracledb::OracleIntervalDS>>(i)?
                .map_or(Value::Null, |v| {
                    json!(interval_ds_text(
                        v.days(),
                        v.hours(),
                        v.minutes(),
                        v.seconds(),
                        v.nanoseconds()
                    ))
                })
        }
        // python-oracledb's own object, which `to_jsonable` turns into its repr.
        "DB_TYPE_INTERVAL_YM" => {
            row.get::<Option<oracledb::OracleIntervalYM>>(i)?
                .map_or(Value::Null, |v| {
                    json!(format!(
                        "IntervalYM(years={}, months={})",
                        v.years(),
                        v.months()
                    ))
                })
        }
        "DB_TYPE_BOOLEAN" => row
            .get::<Option<bool>>(i)?
            .map_or(Value::Null, |b| json!(b)),
        // LOBs, JSON, VECTOR, objects, XML, intervals by month: python-oracledb returns objects that
        // `to_jsonable` turns into their `str()`; this spike names the type instead of fetching them.
        other => json!(format!("<{}>", other.trim_start_matches("DB_TYPE_"))),
    })
}

/// One result set: its column names and the converted rows (at most `cap + 1` are kept).
struct Set {
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
}

enum Outcome {
    Query(Set),
    Count(i64),
}

fn describe(e: &OraError) -> String {
    e.to_string()
}

/// A call timeout surfaces as `CallTimeoutExceeded`, or as `UnableToRecover` when the client's attempt to
/// interrupt the call times out too (what a long PL/SQL call does); both are the statement timeout.
fn describe_statement(e: &OraError, timeout: f64) -> String {
    match e.kind() {
        oracledb::ErrorKind::CallTimeoutExceeded | oracledb::ErrorKind::UnableToRecover => {
            format!("TimeoutError: statement exceeded {timeout} s ({e})")
        }
        _ => describe(e),
    }
}

/// Runs one statement the way `cursor.execute` does: a query is fetched (`cap + 1` rows in read mode, all in
/// write mode, of which `cap + 1` are converted), anything else reports the server's affected-row count.
fn run_statement(
    conn: &Connection,
    sql: &str,
    cap: usize,
    read: bool,
) -> Result<Outcome, OraError> {
    let mut stmt = conn.statement(sql)?.build()?;
    if !stmt.is_query() {
        let affected = stmt.execute(&[])?.rows_affected() as i64;
        // python-oracledb leaves `cursor.rowcount` at 0 after a PL/SQL block, whatever it updated.
        return Ok(Outcome::Count(if stmt.is_plsql() { 0 } else { affected }));
    }
    let cursor = stmt.query(&[])?;
    let meta: Vec<(String, String)> = cursor
        .columns()
        .iter()
        .map(|m| (m.name().to_owned(), m.db_type().name().to_owned()))
        .collect();
    let mut set = Set {
        columns: meta.iter().map(|(n, _)| n.clone()).collect(),
        rows: vec![],
    };
    let mut fetched = 0usize;
    for row in cursor {
        let row = row?;
        fetched += 1;
        if set.rows.len() <= cap {
            let cells = meta
                .iter()
                .enumerate()
                .map(|(i, (_, ty))| cell(&row, i, ty))
                .collect::<Result<Vec<_>, _>>()?;
            set.rows.push(cells);
        }
        if read && fetched > cap {
            break; // fetchmany(max_rows + 1)
        }
    }
    Ok(Outcome::Query(set))
}

fn shape_entry(out: Outcome, cap: usize, read: bool) -> Value {
    match out {
        Outcome::Count(n) => json!({"rowcount": n}),
        Outcome::Query(set) => {
            let truncated = read && set.rows.len() > cap;
            let mut rows = set.rows;
            rows.truncate(cap);
            // `dbrun.py` reads `cursor.rowcount` before it fetches, and python-oracledb reports 0 for a query.
            json!({"rowcount": 0, "columns": set.columns, "rows": rows, "truncated": truncated})
        }
    }
}

/// The connection fields go into a connect descriptor, so a character that structures one (parentheses,
/// `=`, whitespace, a quote) would let a profile value add an address and send the credentials elsewhere.
/// Python builds an Easy Connect string for service names, which has no such structure.
fn check_fields(c: &Value) -> Result<(), String> {
    for key in ["host", "port", "service", "database", "sid"] {
        let v = text(&c[key]).unwrap_or_default();
        if v.chars()
            .any(|ch| matches!(ch, '(' | ')' | '=' | '\'' | '"' | '\\') || ch.is_whitespace())
        {
            return Err(format!("invalid character in the connection field {key}"));
        }
    }
    Ok(())
}

fn connect(req: &Value, limit: Option<Duration>) -> Result<Connection, String> {
    let c = &req["conn"];
    check_fields(c)?;
    let config = Config::default()
        .set_credentials(
            &text(&c["user"]).unwrap_or_default(),
            &text(&c["password"]).unwrap_or_default(),
        )
        .set_connect_string(&dsn(c))
        .map_err(|e| describe(&e))?
        .set_transport_connect_timeout(Some(Duration::from_secs(10))); // tcp_connect_timeout=10
                                                                       // The thin client indexes the server's verifier fields with `unwrap`, so a server that offers only an
                                                                       // old verifier ends in a panic rather than an error. The catch below turns it into a connect failure
                                                                       // ONLY in a build that unwinds (the tests, debug); the release profile has `panic = "abort"`, where
                                                                       // this adapter would die without a JSON line. Not measured either way (results.md, E6).
    let conn = catch_unwind(AssertUnwindSafe(|| oracledb::connect(config)))
        .map_err(|_| {
            "the thin driver panicked while logging in (the account's password verifier is probably older than 12c)"
                .to_string()
        })?
        .map_err(|e| describe(&e))?;
    conn.set_call_timeout(limit).map_err(|e| describe(&e))?;
    Ok(conn)
}

pub fn execute(req: &Value) -> Value {
    let mut result = run(req);
    // The contract's redaction: password, host and user, whatever the server quoted back.
    if let Some(error) = result["error"].as_str() {
        let c = &req["conn"];
        let get = |k: &str| text(&c[k]).unwrap_or_default();
        let (password, host, user) = (get("password"), get("host"), get("user"));
        result["error"] = json!(redact(
            error,
            &[
                (&password, "<password>"),
                (&host, "<host>"),
                (&user, "<user>")
            ]
        ));
    }
    result
}

fn run(req: &Value) -> Value {
    let started = Instant::now();
    let timeout = match timeout_secs(req) {
        Ok(t) => t,
        Err(e) => return json!({"ok": false, "stage": "connect", "error": e}),
    };
    let mut result = session(req, timeout);
    result["seconds"] = json!(started.elapsed().as_secs_f64());
    result
}

fn one(conn: &Connection, sql: &str) -> Result<Row, OraError> {
    conn.query_row(sql, &[])
}

fn session(req: &Value, timeout: f64) -> Value {
    // oracledb's `call_timeout` bounds each round trip; zero means none there too.
    let limit = (timeout > 0.0).then(|| Duration::from_secs_f64(timeout));
    let conn = match connect(req, limit) {
        Ok(c) => c,
        Err(e) => return json!({"ok": false, "stage": "connect", "error": e}),
    };
    let mut result = json!({"ok": true, "results": []});
    let read = req["mode"] != "write";
    let explain = req["explain"] == true;
    let cap = req["max_rows"].as_u64().unwrap_or(200) as usize;
    let outcome: Result<(), OraError> = (|| {
        let ident = one(&conn, IDENTITY)?;
        let server = (0..ident.columns().len())
            .map(|i| cell(&ident, i, "DB_TYPE_VARCHAR"))
            .collect::<Result<Vec<_>, _>>()?;
        result["server"] = json!(server);
        let links = one(&conn, LINKS)?;
        result["links"] = cell(&links, 0, "DB_TYPE_NUMBER")?;
        conn.rollback()?;
        if req["mode"] == "test" {
            return Ok(());
        }
        if read && !explain {
            conn.execute("SET TRANSACTION READ ONLY", &[])?;
        }
        for stmt in req["statements"].as_array().into_iter().flatten() {
            let sql = stmt.as_str().unwrap_or_default();
            let out = if explain {
                conn.execute(&format!("EXPLAIN PLAN FOR {sql}"), &[])?;
                run_statement(&conn, PLAN, cap, read)?
            } else {
                run_statement(&conn, sql, cap, read)?
            };
            result["results"]
                .as_array_mut()
                .unwrap()
                .push(shape_entry(out, cap, read));
        }
        if read {
            conn.rollback()?;
            return Ok(());
        }
        let done = result["results"].as_array().unwrap().len();
        let mismatches: Vec<String> = req["expect"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter(|(i, x)| !x.is_null() && *i < done && **x != result["results"][*i]["rowcount"])
            .map(|(i, x)| {
                format!(
                    "statement {}: expected {}, got {}",
                    i + 1,
                    x,
                    result["results"][i]["rowcount"]
                )
            })
            .collect();
        if mismatches.is_empty() {
            conn.commit()?;
            result["committed"] = json!(true);
        } else {
            conn.rollback()?;
            result["ok"] = json!(false);
            result["stage"] = json!("verify");
            result["error"] = json!(format!(
                "row counts differ from the plan: {}",
                mismatches.join(", ")
            ));
        }
        Ok(())
    })();
    if let Err(e) = outcome {
        // Best effort, as `dbrun.py`: after a timeout the connection is dead and this fails quietly; the
        // server rolls the open transaction back when it sees the session closed.
        let _ = conn.rollback();
        result["ok"] = json!(false);
        result["stage"] = json!("execute");
        result["error"] = json!(describe_statement(&e, timeout));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dsn_matches_oracle_dsn_of_dbrun() {
        let by_service = json!({"host": "db.example", "port": "1522", "service": "FREEPDB1"});
        assert_eq!(
            dsn(&by_service),
            "(DESCRIPTION=(RETRY_DELAY=0)(ADDRESS=(PROTOCOL=TCP)(HOST=db.example)(PORT=1522))(CONNECT_DATA=(SERVICE_NAME=FREEPDB1)))"
        );
        // database stands in for the service, the port defaults to 1521
        assert_eq!(
            dsn(&json!({"host": "h", "database": "ORCL"})),
            "(DESCRIPTION=(RETRY_DELAY=0)(ADDRESS=(PROTOCOL=TCP)(HOST=h)(PORT=1521))(CONNECT_DATA=(SERVICE_NAME=ORCL)))"
        );
        // service wins over database, a numeric port is accepted
        assert_eq!(
            dsn(&json!({"host": "h", "port": 1600, "service": "S", "database": "D"})),
            "(DESCRIPTION=(RETRY_DELAY=0)(ADDRESS=(PROTOCOL=TCP)(HOST=h)(PORT=1600))(CONNECT_DATA=(SERVICE_NAME=S)))"
        );
        assert_eq!(
            dsn(&json!({"host": "h", "sid": "XE"})),
            "(DESCRIPTION=(RETRY_DELAY=0)(ADDRESS=(PROTOCOL=TCP)(HOST=h)(PORT=1521))(CONNECT_DATA=(SID=XE)))"
        );
    }

    #[test]
    fn numbers_follow_python_oracledb() {
        assert_eq!(number_json("0"), json!(0));
        assert_eq!(number_json("-42"), json!(-42));
        assert_eq!(number_json("9223372036854775807"), json!(i64::MAX));
        assert_eq!(number_json("18446744073709551615"), json!(u64::MAX));
        assert_eq!(
            number_json("12345678901234567890123"),
            json!("12345678901234567890123")
        );
        assert_eq!(number_json("1.5"), json!(1.5));
        assert_eq!(number_json("0.05"), json!(0.05));
        assert_eq!(number_json("-3.25"), json!(-3.25));
    }

    #[test]
    fn timestamps_are_isoformat() {
        assert_eq!(
            timestamp_text(2024, 2, 29, 23, 59, 58, 0),
            "2024-02-29T23:59:58"
        );
        assert_eq!(
            timestamp_text(2024, 2, 29, 0, 0, 0, 123_456_789),
            "2024-02-29T00:00:00.123456"
        );
        assert_eq!(
            timestamp_text(2024, 2, 29, 0, 0, 0, 999),
            "2024-02-29T00:00:00"
        );
        assert_eq!(
            timestamp_text(987, 1, 2, 3, 4, 5, 1_000),
            "0987-01-02T03:04:05.000001"
        );
    }

    #[test]
    fn intervals_are_str_of_a_timedelta() {
        // str(timedelta(days=1, hours=2, minutes=3, seconds=4)) == '1 day, 2:03:04'
        assert_eq!(interval_ds_text(1, 2, 3, 4, 0), "1 day, 2:03:04");
        assert_eq!(interval_ds_text(0, 0, 0, 0, 0), "0:00:00");
        assert_eq!(
            interval_ds_text(3, 0, 0, 0, 500_000_000),
            "3 days, 0:00:00.500000"
        );
        // str(timedelta(hours=-1)) == '-1 day, 23:00:00'
        assert_eq!(interval_ds_text(0, -1, 0, 0, 0), "-1 day, 23:00:00");
        // str(timedelta(days=-2, hours=-3, minutes=-4, seconds=-5)) == '-3 days, 20:55:55'
        assert_eq!(interval_ds_text(-2, -3, -4, -5, 0), "-3 days, 20:55:55");
        // str(timedelta(days=100, hours=25)) style hours stay below 24
        assert_eq!(interval_ds_text(100, 23, 59, 59, 0), "100 days, 23:59:59");
    }

    #[test]
    fn timeouts_validate_like_the_other_drivers() {
        assert_eq!(timeout_secs(&json!({})), Ok(30.0));
        assert_eq!(timeout_secs(&json!({"timeout": 0})), Ok(0.0));
        assert_eq!(timeout_secs(&json!({"timeout": 2.5})), Ok(2.5));
        assert_eq!(timeout_secs(&json!({"timeout": 1e300})), Ok(86_400.0));
        for bad in [
            json!(-6),
            json!("NaN"),
            json!(-1e-9),
            json!(null),
            json!(true),
        ] {
            let r = timeout_secs(&json!({"timeout": bad}));
            // null is "absent": the default
            assert!(r.is_err() || bad.is_null(), "{bad}");
        }
    }

    #[test]
    fn shape_entry_reports_what_dbrun_reports() {
        let set = |n: usize| {
            Outcome::Query(Set {
                columns: vec!["a".into()],
                rows: (0..n).map(|i| vec![json!(i)]).collect(),
            })
        };
        // read, cap 2, three rows kept (cap + 1): truncated, two rows out, rowcount 0 as dbrun.py reads it
        // before fetching
        let v = shape_entry(set(3), 2, true);
        assert_eq!(v["truncated"], true);
        assert_eq!(v["rowcount"], 0);
        assert_eq!(v["rows"], json!([[0], [1]]));
        // write mode: never truncated, rows still capped
        let v = shape_entry(set(3), 2, false);
        assert_eq!(v["truncated"], false);
        assert_eq!(v["rows"], json!([[0], [1]]));
        // read, exactly the cap: not truncated
        assert_eq!(shape_entry(set(2), 2, true)["truncated"], false);
        assert_eq!(
            shape_entry(Outcome::Count(7), 2, false),
            json!({"rowcount": 7})
        );
    }
}
