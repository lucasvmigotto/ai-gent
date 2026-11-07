//! E3: MySQL and MariaDB through one `mysql` crate path; Python still owns policy, plans and backups.
//!
//! TLS: not enabled (`mysql` with `default-features = false, features = ["minimal"]`), the same as the
//! Python runner's PyMySQL call, which passes no `ssl` argument. The crate offers `rustls-tls` and
//! `rustls-tls-ring`; neither is built or tested here.
use mysql::consts::ColumnType;
use mysql::prelude::Queryable;
use mysql::{Column, Conn, OptsBuilder, Value as My};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const LOCK_TIMEOUT_MS: u64 = 5000; // dbrun.py LOCK_TIMEOUT_MS
const BINARY_CHARSET: u16 = 63;

fn text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Longest timeout accepted, in seconds. dbrun.py has no cap (its CLI bounds the value); the spike clamps
/// here so that no request value can overflow a `Duration`.
const MAX_TIMEOUT_SECS: f64 = 86_400.0;

/// The request's timeout in seconds. Absent means 30 (the contract default). A value that is not a finite,
/// non-negative number is an invalid request: dbrun.py would fail at stage `connect` (the server refuses a
/// negative `MAX_EXECUTION_TIME`, `int(nan)` raises), so this does too, and never panics.
fn timeout_secs(req: &Value) -> Result<f64, String> {
    match &req["timeout"] {
        Value::Null => Ok(30.0),
        v => match v.as_f64() {
            Some(t) if t.is_finite() && t >= 0.0 => Ok(t.min(MAX_TIMEOUT_SECS)),
            _ => Err("ValueError: invalid timeout".to_string()),
        },
    }
}

/// Connection with the per-session limits set after `SELECT VERSION()` identifies the server.
fn connect(req: &Value, read: bool, timeout: f64) -> mysql::Result<Conn> {
    let c = &req["conn"];
    let opts = OptsBuilder::new()
        .ip_or_hostname(text(&c["host"]))
        .tcp_port(text(&c["port"]).and_then(|p| p.parse().ok()).unwrap_or(3306))
        .user(text(&c["user"]))
        .pass(Some(c["password"].as_str().unwrap_or("")))
        .db_name(text(&c["database"]))
        .prefer_socket(false)
        .tcp_connect_timeout(Some(Duration::from_secs(10)))
        .read_timeout(Some(Duration::from_secs_f64(timeout + 5.0)))
        .write_timeout(Some(Duration::from_secs_f64(timeout + 5.0)));
    let mut conn = Conn::new(opts)?;
    let version: String = conn.query_first("SELECT VERSION()")?.unwrap_or_default();
    if version.to_lowercase().contains("mariadb") {
        // seconds (fractions allowed)
        conn.query_drop(format!("SET SESSION max_statement_time = {timeout}"))?;
    } else {
        // milliseconds, integer
        conn.query_drop(format!("SET SESSION MAX_EXECUTION_TIME = {}", (timeout * 1000.0) as u64))?;
    }
    conn.query_drop(format!("SET SESSION innodb_lock_wait_timeout = {}", LOCK_TIMEOUT_MS / 1000))?;
    conn.query_drop(format!("SET SESSION lock_wait_timeout = {}", LOCK_TIMEOUT_MS / 1000))?;
    if read {
        conn.query_drop("SET SESSION TRANSACTION READ ONLY")?;
    }
    conn.query_drop("SET autocommit = 0")?; // pymysql autocommit=False
    Ok(conn)
}

/// `.ffffff` microseconds from a fraction of 1 to 6 digits, or None when absent or zero.
fn micros(fraction: Option<&str>) -> Option<u32> {
    let f = fraction?;
    if f.is_empty() || f.len() > 6 || !f.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let us: u32 = format!("{f:0<6}").parse().ok()?;
    (us != 0).then_some(us)
}

/// Python `datetime.isoformat()` of `YYYY-MM-DD hh:mm:ss[.f]`: a `T`, and microseconds padded to six digits
/// only when nonzero. Anything else (a zero date, which PyMySQL returns as a string) is left as it came.
fn iso_datetime(s: &str) -> String {
    let Some((date, time)) = s.split_once(' ') else { return s.to_string() };
    if date.starts_with("0000") || date.contains("-00") {
        return s.to_string();
    }
    let (clock, fraction) = match time.split_once('.') {
        Some((c, f)) => (c, Some(f)),
        None => (time, None),
    };
    match micros(fraction) {
        Some(us) => format!("{date}T{clock}.{us:06}"),
        None => format!("{date}T{clock}"),
    }
}

/// PyMySQL decodes TIME to a `timedelta`, and dbrun's `to_jsonable` renders it with `str()`:
/// `H:MM:SS[.ffffff]`, `N day(s), H:MM:SS` from 24 hours, and a negative value as `-1 day, 22:57:57`.
fn py_timedelta(s: &str) -> String {
    let parse = || -> Option<i64> {
        let (neg, body) = match s.strip_prefix('-') {
            Some(b) => (true, b),
            None => (false, s),
        };
        let (clock, fraction) = match body.split_once('.') {
            Some((c, f)) => (c, Some(f)),
            None => (body, None),
        };
        let mut parts = clock.split(':');
        let (h, m, sec): (i64, i64, i64) =
            (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
        if parts.next().is_some() || fraction.is_some_and(|f| micros(Some(f)).is_none() && f.bytes().any(|b| !b.is_ascii_digit())) {
            return None;
        }
        let us = micros(fraction).unwrap_or(0) as i64;
        let total = ((h * 60 + m) * 60 + sec) * 1_000_000 + us;
        Some(if neg { -total } else { total })
    };
    let Some(total) = parse() else { return s.to_string() };
    const DAY: i64 = 86_400 * 1_000_000;
    let days = total.div_euclid(DAY);
    let rem = total.rem_euclid(DAY);
    let (secs, us) = (rem / 1_000_000, rem % 1_000_000);
    let mut out = String::new();
    if days != 0 {
        out += &format!("{days} day{}, ", if days.abs() == 1 { "" } else { "s" });
    }
    out += &format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
    if us != 0 {
        out += &format!(".{us:06}");
    }
    out
}

/// The text protocol returns every value as bytes: convert by column type like PyMySQL's decoders.
fn to_json(value: &My, col: &Column) -> Value {
    let bytes = match value {
        My::NULL => return Value::Null,
        My::Bytes(b) => b,
        My::Int(i) => return json!(i),
        My::UInt(u) => return json!(u),
        My::Float(f) => return json!(f),
        My::Double(d) => return json!(d),
        other => return json!(other.as_sql(true)),
    };
    let s = String::from_utf8_lossy(bytes);
    use ColumnType::*;
    match col.column_type() {
        MYSQL_TYPE_TINY | MYSQL_TYPE_SHORT | MYSQL_TYPE_LONG | MYSQL_TYPE_LONGLONG | MYSQL_TYPE_INT24
        | MYSQL_TYPE_YEAR => s
            .parse::<i64>()
            .map(|n| json!(n))
            .or_else(|_| s.parse::<u64>().map(|n| json!(n)))
            .unwrap_or_else(|_| json!(s)),
        MYSQL_TYPE_FLOAT | MYSQL_TYPE_DOUBLE => s.parse::<f64>().map(|n| json!(n)).unwrap_or_else(|_| json!(s)),
        MYSQL_TYPE_DECIMAL | MYSQL_TYPE_NEWDECIMAL => json!(s), // Python Decimal -> str
        MYSQL_TYPE_DATETIME | MYSQL_TYPE_TIMESTAMP | MYSQL_TYPE_DATETIME2 | MYSQL_TYPE_TIMESTAMP2 => {
            json!(iso_datetime(&s))
        }
        MYSQL_TYPE_TIME | MYSQL_TYPE_TIME2 => json!(py_timedelta(&s)),
        MYSQL_TYPE_JSON => json!(s),
        // Only string-like columns can be binary data; a DATE or TIME also reports the binary charset.
        MYSQL_TYPE_BLOB | MYSQL_TYPE_TINY_BLOB | MYSQL_TYPE_MEDIUM_BLOB | MYSQL_TYPE_LONG_BLOB
        | MYSQL_TYPE_VARCHAR | MYSQL_TYPE_VAR_STRING | MYSQL_TYPE_STRING | MYSQL_TYPE_BIT
        | MYSQL_TYPE_GEOMETRY
            if col.character_set() == BINARY_CHARSET =>
        {
            json!(format!("<{} bytes>", bytes.len()))
        }
        _ => json!(s),
    }
}

fn run(conn: &mut Conn, req: &Value, read: bool, result: &mut Value) -> mysql::Result<()> {
    let (version, host, database): (String, Option<String>, Option<String>) = conn
        .query_first("SELECT VERSION(), @@hostname, DATABASE()")?
        .unwrap_or_default();
    result["server"] = json!([version, host, database]);
    let links = if version.to_lowercase().contains("mariadb") {
        "SELECT COUNT(*) FROM information_schema.TABLES WHERE ENGINE IN ('FEDERATED', 'CONNECT')"
    } else {
        "SELECT COUNT(*) FROM information_schema.TABLES WHERE ENGINE = 'FEDERATED'"
    };
    result["links"] = json!(conn.query_first::<i64, _>(links)?.unwrap_or(0));
    conn.query_drop("ROLLBACK")?;
    if req["mode"] == "test" {
        return Ok(());
    }
    if read {
        conn.query_drop("START TRANSACTION READ ONLY")?;
    }
    let cap = req["max_rows"].as_u64().unwrap_or(200) as usize;
    for stmt in req["statements"].as_array().into_iter().flatten() {
        let stmt = stmt.as_str().unwrap_or_default();
        let sql = if req["explain"] == true { crate::explain_sql("mysql", stmt) } else { stmt.to_owned() };
        let mut rs = conn.query_iter(sql)?;
        let columns: Vec<Column> = rs.columns().as_ref().to_vec();
        let entry = if columns.is_empty() {
            json!({"rowcount": rs.affected_rows() as i64})
        } else {
            let (mut rows, mut total) = (vec![], 0usize);
            for row in rs.by_ref() {
                let row = row?;
                total += 1;
                if rows.len() < cap {
                    rows.push(columns.iter().enumerate().map(|(i, c)| to_json(&row[i], c)).collect::<Vec<_>>());
                }
            }
            // PyMySQL buffers the result: rowcount is the number of rows the SELECT returned.
            json!({"rowcount": total, "columns": columns.iter().map(|c| c.name_str().to_string()).collect::<Vec<_>>(),
                   "truncated": read && total > cap, "rows": rows})
        };
        drop(rs); // drains any further result sets
        result["results"].as_array_mut().unwrap().push(entry);
    }
    if read {
        conn.query_drop("ROLLBACK")?;
        return Ok(());
    }
    let counts: Vec<Value> = result["results"].as_array().unwrap().iter().map(|r| r["rowcount"].clone()).collect();
    let mismatch: Vec<String> = req["expect"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter(|(i, e)| !e.is_null() && *i < counts.len() && **e != counts[*i])
        .map(|(i, e)| format!("statement {}: expected {}, got {}", i + 1, e, counts[i]))
        .collect();
    if mismatch.is_empty() {
        conn.query_drop("COMMIT")?;
        result["committed"] = json!(true);
    } else {
        conn.query_drop("ROLLBACK")?;
        result["ok"] = json!(false);
        result["stage"] = json!("verify");
        result["error"] = json!(format!("row counts differ from the plan: {}", mismatch.join(", ")));
    }
    Ok(())
}

fn describe(e: &mysql::Error) -> String {
    // The crate's error type: a server error carries code, SQLSTATE and message.
    match e {
        mysql::Error::MySqlError(s) => format!("MySqlError: ERROR {} ({}): {}", s.code, s.state, s.message),
        mysql::Error::IoError(_) => format!("IoError: {e}"),
        mysql::Error::DriverError(_) => format!("DriverError: {e}"),
        _ => format!("Error: {e}"),
    }
}

/// The contract redacts the password, host and user from every error, in that order.
fn redact(req: &Value, message: &str) -> String {
    let c = &req["conn"];
    let (password, host, user) = (
        c["password"].as_str().unwrap_or("").to_string(),
        text(&c["host"]).unwrap_or_default(),
        text(&c["user"]).unwrap_or_default(),
    );
    crate::redact(message, &[(&password, "<password>"), (&host, "<host>"), (&user, "<user>")])
}

pub fn execute(req: &Value) -> Value {
    let started = Instant::now();
    let read = req["mode"] != "write";
    let timeout = match timeout_secs(req) {
        Ok(t) => t,
        Err(message) => return json!({"ok": false, "stage": "connect", "error": message}),
    };
    let mut out = match connect(req, read, timeout) {
        Err(e) => json!({"ok": false, "stage": "connect", "error": describe(&e)}),
        Ok(mut conn) => {
            let mut result = json!({"ok": true, "results": []});
            if let Err(e) = run(&mut conn, req, read, &mut result) {
                let _ = conn.query_drop("ROLLBACK");
                result["ok"] = json!(false);
                result["stage"] = json!("execute");
                result["error"] = json!(describe(&e));
            }
            result["seconds"] = json!((started.elapsed().as_secs_f64() * 1000.0).round() / 1000.0);
            result
        }
    };
    if let Some(error) = out["error"].as_str() {
        out["error"] = json!(redact(req, error));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datetime_matches_python_isoformat() {
        // expected values are Python's datetime.isoformat() for the same instants
        for (mysql, python) in [
            ("2026-11-07 14:59:58", "2026-11-07T14:59:58"),
            ("2026-11-07 14:59:58.000", "2026-11-07T14:59:58"),
            ("2026-11-07 14:59:58.000000", "2026-11-07T14:59:58"),
            ("2026-11-07 14:59:58.12", "2026-11-07T14:59:58.120000"),
            ("2026-11-07 14:59:58.123", "2026-11-07T14:59:58.123000"),
            ("2026-11-07 14:59:58.123456", "2026-11-07T14:59:58.123456"),
            ("2026-11-07 14:59:58.000001", "2026-11-07T14:59:58.000001"),
            ("0000-00-00 00:00:00", "0000-00-00 00:00:00"), // PyMySQL leaves a zero date as a string
        ] {
            assert_eq!(iso_datetime(mysql), python, "{mysql}");
        }
    }

    #[test]
    fn time_matches_python_str_of_timedelta() {
        // PyMySQL returns a timedelta for TIME; dbrun renders str(timedelta)
        for (mysql, python) in [
            ("00:00:00", "0:00:00"),
            ("01:02:03", "1:02:03"),
            ("01:02:03.12", "1:02:03.120000"),
            ("01:02:03.000000", "1:02:03"),
            ("01:02:03.123456", "1:02:03.123456"),
            ("24:00:00", "1 day, 0:00:00"),
            ("838:59:59", "34 days, 22:59:59"),
            ("-01:02:03", "-1 day, 22:57:57"),
            ("-48:00:00", "-2 days, 0:00:00"),
            ("-00:00:00.5", "-1 day, 23:59:59.500000"),
            ("garbage", "garbage"),
        ] {
            assert_eq!(py_timedelta(mysql), python, "{mysql}");
        }
    }

    #[test]
    fn timeout_is_validated_and_clamped() {
        assert_eq!(timeout_secs(&json!({})), Ok(30.0));
        assert_eq!(timeout_secs(&json!({"timeout": 5})), Ok(5.0));
        assert_eq!(timeout_secs(&json!({"timeout": 0})), Ok(0.0));
        assert_eq!(timeout_secs(&json!({"timeout": 1e300})), Ok(MAX_TIMEOUT_SECS));
        for bad in [json!(-6), json!(-0.5), json!("NaN"), json!("5"), json!(true), json!([1])] {
            assert!(timeout_secs(&json!({ "timeout": bad })).is_err(), "{bad}");
        }
    }

    #[test]
    fn invalid_timeouts_fail_as_json_and_never_panic() {
        // no server is contacted: validation fails first (the host below is unroutable on purpose)
        let conn = json!({"host": "192.0.2.1", "port": "1", "user": "u", "password": "p"});
        for timeout in [json!(-6), json!(-1e-9), json!(f64::MIN), json!("NaN"), json!(null)] {
            let req = json!({"engine": "mysql", "conn": conn, "mode": "read", "statements": ["SELECT 1"], "timeout": timeout});
            if timeout.is_null() {
                continue; // absent means the default; it would try to connect
            }
            let out = execute(&req);
            assert_eq!(out["ok"], false);
            assert_eq!(out["stage"], "connect");
            assert!(out["error"].as_str().unwrap().contains("invalid timeout"));
        }
    }

    #[test]
    fn a_huge_timeout_does_not_panic_before_connecting() {
        // 1e300 is clamped; a closed local port then fails at connect, as JSON
        let conn = json!({"host": "127.0.0.1", "port": "1", "user": "u", "password": "p"});
        let out = execute(&json!({"engine": "mysql", "conn": conn, "mode": "read", "statements": [], "timeout": 1e300}));
        assert_eq!(out["ok"], false);
        assert_eq!(out["stage"], "connect");
    }
}
