//! The MySQL and MariaDB connector (feature `mysql`, spec 043 T040-T042): one client, the `mysql` crate,
//! for both families. The timeout setting is chosen after `SELECT VERSION()` names the server.
//!
//! What this connector does as `dbrun.py` did with `PyMySQL` (each point has a test below or in
//! `tests/mysql_engine.rs`):
//! - connect over TCP only (a host `localhost` never becomes a socket), 10 s to connect, `timeout + 5` s
//!   for every read and write on the socket, `autocommit = 0`, the connect attribute `program_name`;
//! - after `SELECT VERSION()`: `max_statement_time = <seconds>` on MariaDB (every statement),
//!   `MAX_EXECUTION_TIME = <milliseconds>` on MySQL (SELECT only), `innodb_lock_wait_timeout` and
//!   `lock_wait_timeout` of 5 s; a read adds `SET SESSION TRANSACTION READ ONLY`;
//! - identity (`SELECT VERSION(), @@hostname, DATABASE()`), the federated-table count, a rollback, and
//!   `test` mode stops there; a read then runs `START TRANSACTION READ ONLY`, so a write the classifier
//!   missed fails at the server with `ERROR 1792 (25006)`;
//! - values like `PyMySQL`'s decoders and `to_jsonable`: integers and floats as numbers, `DECIMAL` as its
//!   text, `DATETIME` as `isoformat()` (microseconds padded to six digits, none when zero), a zero or
//!   impossible date left as the text `PyMySQL` returns, `TIME` as `str(timedelta)`, binary string columns
//!   (`BLOB`, `BINARY`, `BIT`, `GEOMETRY`) as `<N bytes>`, `JSON` as its text;
//! - a statement that returns rows reports the number of rows it returned (`PyMySQL` buffers the result);
//!   any other statement reports the OK packet's affected rows (`0` for DDL); only the first result set of
//!   a statement is reported, the rest are read and dropped.
//!
//! What it adds, all fail-closed:
//! - the crate always sets `CLIENT_MULTI_STATEMENTS` and `CLIENT_LOCAL_FILES` (`PyMySQL` sets neither), so
//!   the connector itself refuses, before connecting, a request item that holds more than one statement
//!   and one that names `LOCAL INFILE`;
//! - the request timeout is validated before any socket opens; reading a result is bounded by the same
//!   timeout (the whole result is counted, never buffered past `max_rows`);
//! - TLS is on for every connection and goes through [`crate::core::tls`]: the policy (verify by default;
//!   `unverified` only for a proven-local loopback profile, never production), the CA file checks (bounded
//!   read, no private key) and the warning. See [`MysqlConnector::new`] for what the crate cannot do.
use super::{Connector, ExecResult, Mode, Request, Stage, StatementResult, request};
use crate::core::tls::{
    self, Authorized, ClassKind, TlsContext, TlsMode, TlsSettings, UNVERIFIED_WARNING,
};
use crate::core::{explain_sql, split_statements};
use mysql::consts::ColumnType;
use mysql::prelude::Queryable;
use mysql::{Column, Conn, OptsBuilder, SslOpts, Value as My};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::time::{Duration, Instant};

/// Lock wait in seconds, as `LOCK_TIMEOUT_MS // 1000` in `dbrun.py`.
const LOCK_WAIT_SECS: u64 = 5;
/// TCP connect limit, as `connect_timeout=10`.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Added to the request timeout for every socket read and write, as `read_timeout=timeout + 5`.
const SOCKET_SLACK: Duration = Duration::from_secs(5);
/// MySQL's binary character set number.
const BINARY_CHARSET: u16 = 63;

/// The TLS policy a connector connects with: the profile's settings and the class facts the
/// `unverified` rule needs.
#[derive(Debug, Clone)]
pub struct TlsPolicy {
    /// `tls_mode`, `tls_ca_file`, `tls_server_name`.
    pub settings: TlsSettings,
    /// The class the profile file declares.
    pub declared: ClassKind,
    /// The class after classification (`Local` only when proven).
    pub proven: ClassKind,
}

impl Default for TlsPolicy {
    /// Verify, and production: the strictest reading, so a caller that sets nothing never gets less.
    fn default() -> Self {
        Self {
            settings: TlsSettings::default(),
            declared: ClassKind::Production,
            proven: ClassKind::Production,
        }
    }
}

/// Executes requests against MySQL and MariaDB servers.
#[derive(Debug, Default, Clone)]
pub struct MysqlConnector {
    tls: TlsPolicy,
}

impl MysqlConnector {
    /// A connector with the profile's TLS policy.
    ///
    /// The `mysql` crate builds its own rustls configuration, so what this connector can promise differs
    /// from [`crate::core::tls::client_config`] in two ways, both listed in `differences.md`:
    /// the certificate is also accepted when it chains to the crate's bundled Mozilla roots (not the
    /// operating system's store), even when `tls_ca_file` is set; and a `tls_server_name` that differs
    /// from the host is refused (the crate checks the certificate against the host it connects to).
    /// The CA file and the policy are checked by [`crate::core::tls`] before the crate sees the path.
    #[must_use]
    pub fn new(tls: TlsPolicy) -> Self {
        Self { tls }
    }
}

impl Connector for MysqlConnector {
    fn engines(&self) -> &'static [&'static str] {
        &["mysql", "mariadb"]
    }

    fn execute(&self, req: &Request) -> ExecResult {
        let started = Instant::now();
        // `Connector::run` validates too; `execute` is public, so it never trusts the caller.
        let timeout = match request::timeout(req.timeout) {
            Ok(t) => t,
            Err(m) => return request::refusal(m.to_string()),
        };
        for sql in &req.statements {
            if let Err(m) = single_statement(sql) {
                return ExecResult::failure(Stage::Execute, m);
            }
        }
        let mut conn = match connect(req, timeout, &self.tls) {
            Ok(c) => c,
            Err(e) => return ExecResult::failure(Stage::Connect, e),
        };
        let mut result = ExecResult::success();
        if let Err(e) = run(&mut conn, req, timeout, &mut result) {
            // A lost connection (a socket timeout included) is dropped as it is: a ROLLBACK sent into
            // a stream the server is still answering would wait for another timeout. The server
            // rolls the open transaction back when the connection closes.
            if !e.connection_lost {
                let _ = conn.query_drop("ROLLBACK");
            }
            result.ok = false;
            result.stage = Some(Stage::Execute);
            result.error = Some(e.message);
        }
        result.seconds = Some((started.elapsed().as_secs_f64() * 1000.0).round() / 1000.0);
        result
    }
}

/// Refuses an item the server would take as several statements, or that asks the client for a local
/// file. The crate sets `CLIENT_MULTI_STATEMENTS` and `CLIENT_LOCAL_FILES` and offers no way to unset
/// them, so a string with a second statement would otherwise run as one request item (and be counted as
/// one). The message is fixed text: no part of the statement is echoed.
fn single_statement(sql: &str) -> Result<(), &'static str> {
    let parts = split_statements(sql, "mysql").map_err(
        |_| "refused: the statement splits differently with and without backslash escaping",
    )?;
    if parts.len() > 1 {
        return Err("refused: an item holds more than one statement; send one statement per item");
    }
    let blanked = crate::core::blank_literals(sql, "mysql").to_ascii_lowercase();
    let words: Vec<&str> = blanked
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .collect();
    if words.windows(2).any(|w| w == ["local", "infile"]) {
        return Err("refused: LOCAL INFILE would read a file of this machine");
    }
    Ok(())
}

/// The session setup after `SELECT VERSION()`, in `dbrun.py`'s order (after `autocommit = 0`).
fn session_statements(maria: bool, timeout: Duration, read: bool) -> Vec<String> {
    let mut sql = vec![if maria {
        // seconds, fractions allowed; applies to every statement
        format!("SET SESSION max_statement_time = {}", timeout.as_secs_f64())
    } else {
        // milliseconds, an integer; applies to SELECT only
        format!("SET SESSION MAX_EXECUTION_TIME = {}", timeout.as_millis())
    }];
    sql.push(format!(
        "SET SESSION innodb_lock_wait_timeout = {LOCK_WAIT_SECS}"
    ));
    sql.push(format!("SET SESSION lock_wait_timeout = {LOCK_WAIT_SECS}"));
    if read {
        sql.push("SET SESSION TRANSACTION READ ONLY".to_string());
    }
    sql
}

fn text(v: Option<&str>) -> Option<&str> {
    v.map(str::trim).filter(|s| !s.is_empty())
}

fn connect(req: &Request, timeout: Duration, policy: &TlsPolicy) -> Result<Conn, String> {
    let c = &req.conn;
    let host = text(c.host.as_deref()).ok_or("missing conn.host")?;
    let port: u16 = match text(c.port.as_deref()) {
        None => 3306,
        Some(p) => p.parse().ok().filter(|p| *p != 0).ok_or("invalid port")?,
    };
    let ssl = ssl_opts(policy, host)?;
    let bare_host = host.trim_start_matches('[').trim_end_matches(']');
    let attrs = HashMap::from([("program_name", "ai-gent-dbrun")]);
    let socket_timeout = timeout.saturating_add(SOCKET_SLACK);
    let opts = OptsBuilder::new()
        .ip_or_hostname(Some(bare_host))
        .tcp_port(port)
        .user(text(c.user.as_deref()))
        .pass(Some(c.password.as_deref().unwrap_or("")))
        .db_name(text(c.database.as_deref()))
        .prefer_socket(false)
        .ssl_opts(Some(ssl))
        .connect_attrs(Some(attrs))
        .tcp_connect_timeout(Some(CONNECT_TIMEOUT))
        .read_timeout(Some(socket_timeout))
        .write_timeout(Some(socket_timeout));
    let mut conn = Conn::new(opts).map_err(|e| describe_connect(&e))?;
    let setup = |conn: &mut Conn| -> mysql::Result<()> {
        conn.query_drop("SET autocommit = 0")?;
        let version: Option<String> = conn.query_first("SELECT VERSION()")?;
        let maria = version.is_some_and(|v| v.to_lowercase().contains("mariadb"));
        for sql in session_statements(maria, timeout, req.mode != Mode::Write) {
            conn.query_drop(sql)?;
        }
        Ok(())
    };
    setup(&mut conn).map_err(|e| describe(&e))?;
    Ok(conn)
}

/// The TLS options for one connection, after the policy and the CA file are checked through
/// [`crate::core::tls`].
fn ssl_opts(policy: &TlsPolicy, host: &str) -> Result<SslOpts, String> {
    let auth: Authorized = policy
        .settings
        .authorize(&TlsContext {
            declared: policy.declared,
            proven: policy.proven,
            host,
        })
        .map_err(|e| e.to_string())?;
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if let Some(name) = &auth.settings().server_name
        && !name.eq_ignore_ascii_case(bare)
    {
        return Err("tls_server_name: MySQL and MariaDB check the certificate against the host the connection goes to; set host to the name in the certificate (a different tls_server_name is not supported)".into());
    }
    // Builds the shared configuration only to run its checks: the CA file is read bounded and refused
    // when it holds a private key or no certificate; verify mode with no CA file needs system roots.
    tls::client_config(&auth).map_err(|e| e.to_string())?;
    let mut ssl = SslOpts::default();
    match auth.settings().mode {
        TlsMode::Verify => {
            if let Some(ca) = &auth.settings().ca_file {
                ssl = ssl.with_root_cert_path(Some(ca.clone()));
            }
        }
        TlsMode::Unverified => {
            eprintln!("{UNVERIFIED_WARNING}");
            ssl = ssl
                .with_danger_accept_invalid_certs(true)
                .with_danger_skip_domain_validation(true);
        }
    }
    Ok(ssl)
}

/// Names the profile fields that fix a TLS failure.
const TLS_HINT: &str = "the server certificate is verified by default: set tls_ca_file to the CA that signed it (tls_mode = unverified is accepted only on a proven-local loopback profile)";

fn describe_connect(e: &mysql::Error) -> String {
    let text = describe(e);
    if matches!(
        e,
        mysql::Error::DriverError(mysql::DriverError::TlsNotSupported)
    ) {
        return format!(
            "{text} (the server does not offer TLS, which every connection requires: enable it on the server)"
        );
    }
    // A failed handshake surfaces as a TLS error or, once the crate wraps it, as a codec or I/O error.
    if matches!(e, mysql::Error::TlsError(_)) || looks_like_tls(&text) {
        return format!("{text} ({TLS_HINT})");
    }
    text
}

/// Whether a connect error text is about the TLS handshake or the certificate.
fn looks_like_tls(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    [
        "peer certificate",
        "tls",
        "handshake",
        "unknownissuer",
        "alert",
    ]
    .iter()
    .any(|k| t.contains(k))
}

/// A server error carries code, SQLSTATE and message; the other kinds say what failed.
fn describe(e: &mysql::Error) -> String {
    let text = match e {
        mysql::Error::MySqlError(s) => {
            return format!("MysqlError: ERROR {} ({}): {}", s.code, s.state, s.message);
        }
        mysql::Error::IoError(_) => format!("IoError: {e}"),
        mysql::Error::DriverError(_) => format!("DriverError: {e}"),
        mysql::Error::TlsError(_) => format!("TlsError: {e}"),
        _ => format!("Error: {e}"),
    };
    if looks_like_timeout(&text) {
        return format!("{text} (the server did not answer within the request timeout plus 5 s)");
    }
    text
}

/// Whether an I/O error text is a socket read or write timeout (`EAGAIN` on Unix, `ETIMEDOUT`).
fn looks_like_timeout(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    t.contains("os error 11)") || t.contains("timed out") || t.contains("would block")
}

/// A failed statement: the message, and whether the connection can no longer be trusted to be in step.
struct Failed {
    message: String,
    connection_lost: bool,
}

impl From<mysql::Error> for Failed {
    fn from(e: mysql::Error) -> Self {
        Self {
            message: describe(&e),
            connection_lost: !matches!(e, mysql::Error::MySqlError(_)),
        }
    }
}

fn run(
    conn: &mut Conn,
    req: &Request,
    timeout: Duration,
    result: &mut ExecResult,
) -> Result<(), Failed> {
    let sql_err = Failed::from;
    let (version, host, database): (Option<String>, Option<String>, Option<String>) = conn
        .query_first("SELECT VERSION(), @@hostname, DATABASE()")
        .map_err(sql_err)?
        .unwrap_or_default();
    let maria = version
        .as_deref()
        .is_some_and(|v| v.to_lowercase().contains("mariadb"));
    result.server = Some(vec![
        Value::from(version),
        Value::from(host),
        Value::from(database),
    ]);
    let links = if maria {
        "SELECT COUNT(*) FROM information_schema.TABLES WHERE ENGINE IN ('FEDERATED', 'CONNECT')"
    } else {
        "SELECT COUNT(*) FROM information_schema.TABLES WHERE ENGINE = 'FEDERATED'"
    };
    result.links = Some(
        conn.query_first::<i64, _>(links)
            .map_err(sql_err)?
            .unwrap_or(0),
    );
    conn.query_drop("ROLLBACK").map_err(sql_err)?;
    if req.mode == Mode::Test {
        return Ok(());
    }
    let read = req.mode == Mode::Read;
    if read {
        conn.query_drop("START TRANSACTION READ ONLY")
            .map_err(sql_err)?;
    }
    // Results go straight into `result`, so a failure at statement N keeps the ones before it.
    for stmt in &req.statements {
        let sql = if req.explain {
            explain_sql(&req.engine, stmt)
        } else {
            stmt.clone()
        };
        let one = run_statement(conn, &sql, read, req.max_rows, timeout)?;
        result.results.get_or_insert_with(Vec::new).push(one);
    }
    if read {
        conn.query_drop("ROLLBACK").map_err(sql_err)?;
        return Ok(());
    }
    let counts: Vec<i64> = result
        .results
        .iter()
        .flatten()
        .map(|r| r.rowcount)
        .collect();
    if let Some(error) = request::verify_counts(&req.expect, &counts) {
        conn.query_drop("ROLLBACK").map_err(sql_err)?;
        result.ok = false;
        result.stage = Some(Stage::Verify);
        result.error = Some(error);
    } else {
        conn.query_drop("COMMIT").map_err(sql_err)?;
        result.committed = Some(true);
    }
    Ok(())
}

/// Runs one statement and reports its first result set. The statement's other result sets (a `CALL`
/// can return several) are read to the end, so the connection stays in step, and their errors count.
/// Reading rows stops with an error at the request timeout: every wait has a bound.
fn run_statement(
    conn: &mut Conn,
    sql: &str,
    read: bool,
    cap: usize,
    timeout: Duration,
) -> Result<StatementResult, Failed> {
    let deadline = Instant::now() + timeout;
    let too_slow = || Failed {
        message: format!(
            "reading the result took longer than the request timeout ({} s)",
            timeout.as_secs_f64()
        ),
        connection_lost: true,
    };
    let mut rs = conn.query_iter(sql).map_err(Failed::from)?;
    let columns: Vec<Column> = rs.columns().as_ref().to_vec();
    let affected = rs.affected_rows();
    let mut kept: Vec<Vec<Value>> = Vec::new();
    let mut total: u64 = 0;
    let mut first = true;
    while let Some(mut set) = rs.iter() {
        for row in set.by_ref() {
            let row = row.map_err(Failed::from)?;
            if first {
                total += 1;
                if kept.len() < cap {
                    kept.push(
                        columns
                            .iter()
                            .enumerate()
                            .map(|(i, c)| to_json(&row[i], c))
                            .collect(),
                    );
                }
            }
            if Instant::now() > deadline {
                return Err(too_slow());
            }
        }
        first = false;
    }
    let returns_rows = !columns.is_empty();
    let rowcount = if returns_rows {
        i64::try_from(total).unwrap_or(i64::MAX)
    } else {
        i64::try_from(affected).unwrap_or(i64::MAX)
    };
    Ok(StatementResult {
        rowcount,
        columns: returns_rows.then(|| columns.iter().map(|c| c.name_str().to_string()).collect()),
        truncated: returns_rows.then_some(read && total > cap as u64),
        rows: returns_rows.then_some(kept),
    })
}

/// `.ffffff` microseconds from a fraction of 1 to 6 digits, or `None` when absent or zero.
fn micros(fraction: Option<&str>) -> Option<u32> {
    let f = fraction?;
    if f.is_empty() || f.len() > 6 || !f.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let us: u32 = format!("{f:0<6}").parse().ok()?;
    (us != 0).then_some(us)
}

/// Whether `YYYY-MM-DD` is a real calendar date, as Python's `datetime.date` needs.
fn valid_date(date: &str) -> bool {
    let mut it = date.split('-');
    let (Some(y), Some(m), Some(d), None) = (it.next(), it.next(), it.next(), it.next()) else {
        return false;
    };
    let (Ok(y), Ok(m), Ok(d)) = (y.parse::<u32>(), m.parse::<u32>(), d.parse::<u32>()) else {
        return false;
    };
    if y == 0 || y > 9999 || !(1..=12).contains(&m) || d == 0 {
        return false;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = match m {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    d <= days
}

/// Python `datetime.isoformat()` of `YYYY-MM-DD hh:mm:ss[.f]`: a `T`, and microseconds padded to six
/// digits only when nonzero. A zero or impossible date is left as it came: `PyMySQL` returns the text.
fn iso_datetime(s: &str) -> String {
    let Some((date, time)) = s.split_once(' ') else {
        return s.to_string();
    };
    if !valid_date(date) {
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

/// `PyMySQL` decodes `TIME` to a `timedelta`, and `to_jsonable` renders it with `str()`: `H:MM:SS[.ffffff]`,
/// `N day(s), H:MM:SS` from 24 hours, and a negative value as `-1 day, 22:57:57`.
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
        let (h, m, sec): (i64, i64, i64) = (
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        );
        if parts.next().is_some()
            || fraction.is_some_and(|f| {
                f.len() > 6 || f.is_empty() || f.bytes().any(|b| !b.is_ascii_digit())
            })
        {
            return None;
        }
        let us = i64::from(micros(fraction).unwrap_or(0));
        let total = ((h * 60 + m) * 60 + sec) * 1_000_000 + us;
        Some(if neg { -total } else { total })
    };
    let Some(total) = parse() else {
        return s.to_string();
    };
    const DAY: i64 = 86_400 * 1_000_000;
    let days = total.div_euclid(DAY);
    let rem = total.rem_euclid(DAY);
    let (secs, us) = (rem / 1_000_000, rem % 1_000_000);
    let mut out = String::new();
    if days != 0 {
        let _ = write!(
            out,
            "{days} day{}, ",
            if days.abs() == 1 { "" } else { "s" }
        );
    }
    let _ = write!(
        out,
        "{}:{:02}:{:02}",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    );
    if us != 0 {
        let _ = write!(out, ".{us:06}");
    }
    out
}

/// The text protocol returns every value as bytes: convert by column type like `PyMySQL`'s decoders.
fn to_json(value: &My, col: &Column) -> Value {
    let bytes = match value {
        My::NULL => return Value::Null,
        My::Bytes(b) => b,
        My::Int(i) => return Value::from(*i),
        My::UInt(u) => return Value::from(*u),
        My::Float(f) => return Value::from(*f),
        My::Double(d) => return Value::from(*d),
        other => return Value::from(other.as_sql(true)),
    };
    let s = String::from_utf8_lossy(bytes);
    use ColumnType::{
        MYSQL_TYPE_BIT, MYSQL_TYPE_BLOB, MYSQL_TYPE_DATETIME, MYSQL_TYPE_DATETIME2,
        MYSQL_TYPE_DOUBLE, MYSQL_TYPE_FLOAT, MYSQL_TYPE_GEOMETRY, MYSQL_TYPE_INT24,
        MYSQL_TYPE_LONG, MYSQL_TYPE_LONG_BLOB, MYSQL_TYPE_LONGLONG, MYSQL_TYPE_MEDIUM_BLOB,
        MYSQL_TYPE_SHORT, MYSQL_TYPE_STRING, MYSQL_TYPE_TIME, MYSQL_TYPE_TIME2,
        MYSQL_TYPE_TIMESTAMP, MYSQL_TYPE_TIMESTAMP2, MYSQL_TYPE_TINY, MYSQL_TYPE_TINY_BLOB,
        MYSQL_TYPE_VAR_STRING, MYSQL_TYPE_VARCHAR, MYSQL_TYPE_YEAR,
    };
    match col.column_type() {
        MYSQL_TYPE_TINY | MYSQL_TYPE_SHORT | MYSQL_TYPE_LONG | MYSQL_TYPE_LONGLONG
        | MYSQL_TYPE_INT24 | MYSQL_TYPE_YEAR => s
            .parse::<i64>()
            .map(Value::from)
            .or_else(|_| s.parse::<u64>().map(Value::from))
            .unwrap_or_else(|_| Value::from(s.into_owned())),
        MYSQL_TYPE_FLOAT | MYSQL_TYPE_DOUBLE => s
            .parse::<f64>()
            .map_or_else(|_| Value::from(s.into_owned()), Value::from),
        // DECIMAL (Python Decimal -> str) and JSON (text) fall to the default arm below.
        MYSQL_TYPE_DATETIME
        | MYSQL_TYPE_TIMESTAMP
        | MYSQL_TYPE_DATETIME2
        | MYSQL_TYPE_TIMESTAMP2 => Value::from(iso_datetime(&s)),
        MYSQL_TYPE_TIME | MYSQL_TYPE_TIME2 => Value::from(py_timedelta(&s)),
        // Only string-like columns can be binary data; a DATE or TIME also reports the binary charset.
        MYSQL_TYPE_BLOB
        | MYSQL_TYPE_TINY_BLOB
        | MYSQL_TYPE_MEDIUM_BLOB
        | MYSQL_TYPE_LONG_BLOB
        | MYSQL_TYPE_VARCHAR
        | MYSQL_TYPE_VAR_STRING
        | MYSQL_TYPE_STRING
        | MYSQL_TYPE_BIT
        | MYSQL_TYPE_GEOMETRY
            if col.character_set() == BINARY_CHARSET =>
        {
            Value::from(format!("<{} bytes>", bytes.len()))
        }
        _ => Value::from(s.into_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connector::ConnFacts;
    use serde_json::json;

    const PASSWORD: &str = "pw-Sup3rS3cret-4711";
    const HOST: &str = "db.mysql-secret-host.example";
    const USER: &str = "svc_secret_user";

    fn facts() -> ConnFacts {
        ConnFacts {
            host: Some(HOST.into()),
            port: Some("3306".into()),
            database: Some("shop".into()),
            user: Some(USER.into()),
            password: Some(PASSWORD.into()),
            ..ConnFacts::default()
        }
    }

    fn request(mode: Mode, statements: &[&str]) -> Request {
        let mut r = Request::new(
            "mysql",
            facts(),
            mode,
            statements.iter().map(|s| (*s).to_string()).collect(),
        );
        r.timeout = 5.0;
        r
    }

    /// No password, host or user in the whole line; the check every refusal below makes.
    fn assert_no_secret(line: &str) {
        for secret in [PASSWORD, HOST, USER] {
            assert!(!line.contains(secret), "{secret} leaked in {line}");
        }
    }

    // ---- values (parity with `PyMySQL`'s decoders and to_jsonable) ----

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
            ("2024-02-29 00:00:00", "2024-02-29T00:00:00"),
            // `PyMySQL` leaves these as the text the server sent (datetime() raises ValueError)
            ("0000-00-00 00:00:00", "0000-00-00 00:00:00"),
            ("2026-00-10 01:02:03", "2026-00-10 01:02:03"),
            ("2026-10-00 01:02:03", "2026-10-00 01:02:03"),
            ("2026-02-30 01:02:03", "2026-02-30 01:02:03"),
            ("2025-02-29 01:02:03", "2025-02-29 01:02:03"),
            ("2026-13-01 01:02:03", "2026-13-01 01:02:03"),
            ("not a date", "not a date"),
        ] {
            assert_eq!(iso_datetime(mysql), python, "{mysql}");
        }
    }

    #[test]
    fn time_matches_python_str_of_timedelta() {
        // `PyMySQL` returns a timedelta for TIME; dbrun renders str(timedelta)
        for (mysql, python) in [
            ("00:00:00", "0:00:00"),
            ("01:02:03", "1:02:03"),
            ("01:02:03.12", "1:02:03.120000"),
            ("01:02:03.000000", "1:02:03"),
            ("01:02:03.123456", "1:02:03.123456"),
            ("24:00:00", "1 day, 0:00:00"),
            ("838:59:59", "34 days, 22:59:59"),
            ("-838:59:59", "-35 days, 1:00:01"),
            ("-01:02:03", "-1 day, 22:57:57"),
            ("-48:00:00", "-2 days, 0:00:00"),
            ("-00:00:00.5", "-1 day, 23:59:59.500000"),
            ("garbage", "garbage"),
            ("01:02:03.1234567", "01:02:03.1234567"),
            ("01:02", "01:02"),
        ] {
            assert_eq!(py_timedelta(mysql), python, "{mysql}");
        }
    }

    // ---- session setup ----

    #[test]
    fn the_timeout_variable_follows_the_server_family() {
        let t = Duration::from_millis(2500);
        assert_eq!(
            session_statements(true, t, false),
            [
                "SET SESSION max_statement_time = 2.5",
                "SET SESSION innodb_lock_wait_timeout = 5",
                "SET SESSION lock_wait_timeout = 5",
            ]
        );
        assert_eq!(
            session_statements(false, t, false),
            [
                "SET SESSION MAX_EXECUTION_TIME = 2500",
                "SET SESSION innodb_lock_wait_timeout = 5",
                "SET SESSION lock_wait_timeout = 5",
            ]
        );
        // whole seconds print without a fraction on MariaDB, as Python's int does
        assert_eq!(
            session_statements(true, Duration::from_secs(30), false)[0],
            "SET SESSION max_statement_time = 30"
        );
        // the shortest accepted timeout is one millisecond, never 0 (no limit)
        assert_eq!(
            session_statements(false, Duration::from_secs_f64(0.001), false)[0],
            "SET SESSION MAX_EXECUTION_TIME = 1"
        );
    }

    #[test]
    fn a_read_session_is_read_only_and_a_write_session_is_not() {
        let t = Duration::from_secs(5);
        for maria in [false, true] {
            let read = session_statements(maria, t, true);
            assert_eq!(read.last().unwrap(), "SET SESSION TRANSACTION READ ONLY");
            let write = session_statements(maria, t, false);
            assert!(write.iter().all(|s| !s.contains("READ ONLY")), "{write:?}");
        }
    }

    // ---- refusals that need no server (and prove no connection is tried) ----

    /// The unroutable test address would hold a connect for 10 s: a refusal must come first.
    fn refused_fast(req: &Request, connector: &MysqlConnector) -> serde_json::Value {
        let started = Instant::now();
        let line = connector.run(req).to_line();
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "no connection may be tried: {line}"
        );
        assert_no_secret(&line);
        serde_json::from_str(&line).unwrap()
    }

    #[test]
    fn timeouts_are_validated_before_any_socket_opens() {
        for bad in [-6.0, 0.0, 1e-9, f64::NAN, f64::INFINITY, 1e300, 3601.0] {
            let mut req = request(Mode::Read, &["SELECT 1"]);
            req.timeout = bad;
            let out = refused_fast(&req, &MysqlConnector::default());
            assert_eq!(out["ok"], false, "{bad}");
            assert_eq!(out["stage"], "connect", "{bad}");
            assert!(
                out["error"].as_str().unwrap().contains("invalid timeout"),
                "{bad}"
            );
        }
        // `execute` is public: it validates for itself when a caller skips `run`
        let mut req = request(Mode::Read, &["SELECT 1"]);
        req.timeout = -6.0;
        let out = MysqlConnector::default().execute(&req);
        assert_eq!(out.stage, Some(Stage::Connect));
    }

    #[test]
    fn a_statement_item_with_a_second_statement_is_refused_before_connecting() {
        for tricks in [
            "SELECT 1; UPDATE customer SET status = 'x'",
            "SELECT 1;DROP TABLE customer",
            "SELECT 1 /* x */; DELETE FROM customer",
            "UPDATE customer SET a = 1; UPDATE customer SET a = 2",
            "SELECT 'x'; SELECT 'y'",
        ] {
            let req = request(Mode::Write, &["SELECT 0", tricks]);
            let out = refused_fast(&req, &MysqlConnector::default());
            assert_eq!(out["ok"], false, "{tricks}");
            assert_eq!(out["stage"], "execute", "{tricks}");
            let error = out["error"].as_str().unwrap();
            assert!(error.contains("more than one statement"), "{error}");
            assert!(
                !error.contains("DROP") && !error.contains("customer"),
                "{error}"
            );
            assert!(out.get("results").is_none(), "nothing may have run: {out}");
        }
    }

    #[test]
    fn semicolons_inside_text_and_a_trailing_one_are_one_statement() {
        for ok in [
            "SELECT 'a;b'",
            "SELECT \"a;b\"",
            "SELECT `a;b` FROM t",
            "SELECT 1 -- ; DROP TABLE t",
            "SELECT 1 /* ; DROP TABLE t */",
            "SELECT 1;",
            "SELECT 1 ; ",
        ] {
            assert_eq!(single_statement(ok), Ok(()), "{ok}");
        }
    }

    #[test]
    fn a_local_infile_is_refused_before_connecting() {
        for sql in [
            "LOAD DATA LOCAL INFILE '/etc/passwd' INTO TABLE t",
            "load data   local\n infile '/etc/hostname' into table t",
            "LOAD DATA/**/LOCAL/**/INFILE '/x' INTO TABLE t",
        ] {
            let req = request(Mode::Write, &[sql]);
            let out = refused_fast(&req, &MysqlConnector::default());
            assert_eq!(out["stage"], "execute", "{sql}");
            assert!(out["error"].as_str().unwrap().contains("LOCAL INFILE"));
        }
        // text and an unrelated column named like the words stay allowed
        assert_eq!(
            single_statement("SELECT 'local infile', local_x FROM t"),
            Ok(())
        );
        assert_eq!(
            single_statement("SELECT infile FROM t WHERE local = 1"),
            Ok(())
        );
    }

    #[test]
    fn missing_host_and_bad_ports_are_refused_before_connecting() {
        let mut req = request(Mode::Read, &["SELECT 1"]);
        req.conn.host = None;
        let out = refused_fast(&req, &MysqlConnector::default());
        assert_eq!(out["stage"], "connect");
        for bad in ["0", "65536", "-1", "abc", "3306; DROP"] {
            let mut req = request(Mode::Read, &["SELECT 1"]);
            req.conn.port = Some(bad.into());
            let out = refused_fast(&req, &MysqlConnector::default());
            assert_eq!(out["stage"], "connect", "{bad}");
            assert!(
                out["error"].as_str().unwrap().contains("invalid port"),
                "{bad}"
            );
        }
    }

    // ---- TLS policy (FR-015..FR-017), refusals that need no server ----

    fn policy(
        mode: &str,
        ca: Option<&str>,
        declared: ClassKind,
        proven: ClassKind,
    ) -> MysqlConnector {
        MysqlConnector::new(TlsPolicy {
            settings: TlsSettings::from_fields(Some(mode), ca, None, None).unwrap(),
            declared,
            proven,
        })
    }

    #[test]
    fn unverified_is_refused_for_production_and_for_anything_not_proven_local_loopback() {
        // (declared, proven, host): each must be refused, naming tls_mode, before a socket opens
        for (declared, proven, host) in [
            (ClassKind::Production, ClassKind::Production, "127.0.0.1"),
            (ClassKind::Production, ClassKind::Local, "127.0.0.1"),
            (ClassKind::Local, ClassKind::Production, "127.0.0.1"),
            (ClassKind::Staging, ClassKind::Staging, "127.0.0.1"),
            (ClassKind::Development, ClassKind::Development, "localhost"),
            (ClassKind::Local, ClassKind::Local, HOST),
            (ClassKind::Local, ClassKind::Local, "127.0.0.1.evil.example"),
        ] {
            let mut req = request(Mode::Read, &["SELECT 1"]);
            req.conn.host = Some(host.into());
            let out = refused_fast(&req, &policy("unverified", None, declared, proven));
            assert_eq!(out["ok"], false, "{declared:?} {proven:?} {host}");
            assert_eq!(out["stage"], "connect");
            assert!(
                out["error"].as_str().unwrap().starts_with("tls_mode:"),
                "{declared:?} {proven:?} {host}: {out}"
            );
        }
    }

    #[test]
    fn the_default_connector_verifies_and_is_production() {
        let p = TlsPolicy::default();
        assert_eq!(p.settings.mode, TlsMode::Verify);
        assert_eq!(
            (p.declared, p.proven),
            (ClassKind::Production, ClassKind::Production)
        );
        // and `unverified` cannot be reached from it without saying so
        assert!(
            TlsSettings {
                mode: TlsMode::Unverified,
                ..TlsSettings::default()
            }
            .authorize(&TlsContext {
                declared: p.declared,
                proven: p.proven,
                host: "127.0.0.1",
            })
            .is_err()
        );
    }

    #[test]
    fn a_ca_file_that_is_a_private_key_is_refused_without_echoing_it() {
        let dir = tempfile::tempdir().unwrap();
        let key = dir.path().join("server-key.pem");
        let body = "-----BEGIN PRIVATE KEY-----\nTUlHVEFnRUFNQk1HQnlxR1NNNDlBZ0VHQ0NxR1NNNDlBd0VIQkc0d2JBSUJBUVF=\n-----END PRIVATE KEY-----\n";
        std::fs::write(&key, body).unwrap();
        let path = key.to_str().unwrap();
        let c = policy(
            "verify",
            Some(path),
            ClassKind::Production,
            ClassKind::Production,
        );
        let out = refused_fast(&request(Mode::Read, &["SELECT 1"]), &c);
        assert_eq!(out["stage"], "connect");
        let error = out["error"].as_str().unwrap();
        assert!(error.starts_with("tls_ca_file:"), "{error}");
        assert!(error.contains("private key"), "{error}");
        assert!(
            !error.contains("TUlHVEFn") && !error.contains(path),
            "{error}"
        );
        // a missing file and an empty one refuse as well, naming the same field
        for name in ["missing.pem", "empty.pem"] {
            let p = dir.path().join(name);
            if name == "empty.pem" {
                std::fs::write(&p, "").unwrap();
            }
            let c = policy(
                "verify",
                p.to_str(),
                ClassKind::Production,
                ClassKind::Production,
            );
            let out = refused_fast(&request(Mode::Read, &["SELECT 1"]), &c);
            assert!(
                out["error"].as_str().unwrap().starts_with("tls_ca_file:"),
                "{name}: {out}"
            );
        }
    }

    #[test]
    fn a_tls_server_name_that_differs_from_the_host_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let ca = crate::core::tls::testca::TestCa::new("mysql-unit").write_ca(dir.path());
        let settings =
            TlsSettings::from_fields(None, ca.to_str(), Some("other.example"), None).unwrap();
        let c = MysqlConnector::new(TlsPolicy {
            settings,
            ..TlsPolicy::default()
        });
        let out = refused_fast(&request(Mode::Read, &["SELECT 1"]), &c);
        assert_eq!(out["stage"], "connect");
        assert!(
            out["error"]
                .as_str()
                .unwrap()
                .starts_with("tls_server_name:"),
            "{out}"
        );
    }

    #[test]
    fn a_handshake_failure_in_any_wrapper_names_the_tls_fields() {
        for text in [
            "Error: CodecError { IO error: `invalid peer certificate: UnknownIssuer' }",
            "IoError: received fatal alert: HandshakeFailure",
            "TlsError: bad certificate",
        ] {
            assert!(looks_like_tls(text), "{text}");
        }
        for text in [
            "MysqlError: ERROR 1045 (28000): Access denied",
            "IoError: connection refused",
        ] {
            assert!(!looks_like_tls(text), "{text}");
        }
    }

    #[test]
    fn a_socket_timeout_is_told_apart_from_other_failures() {
        assert!(looks_like_timeout(
            "Error: CodecError { IO error: `Resource temporarily unavailable (os error 11)' }"
        ));
        assert!(looks_like_timeout("IoError: IoError { timed out }"));
        assert!(!looks_like_timeout(
            "MysqlError: ERROR 1205 (HY000): Lock wait timeout exceeded"
        ));
        assert!(!looks_like_timeout(
            "IoError: Connection refused (os error 111)"
        ));
    }

    #[test]
    fn engines_and_an_unknown_engine_name() {
        assert_eq!(MysqlConnector::default().engines(), ["mysql", "mariadb"]);
        let mut req = request(Mode::Read, &["SELECT 1"]);
        req.engine = PASSWORD.into();
        let out = refused_fast(&req, &MysqlConnector::default());
        assert_eq!(out["error"], json!("unsupported engine"));
    }

    // ---- row counting and the cap (no server: the pure helpers it relies on) ----

    #[test]
    fn a_closed_port_fails_as_a_redacted_connect_failure_never_a_panic() {
        // 127.0.0.1 and localhost are not secrets; the password and user still are
        let mut req = request(Mode::Read, &["SELECT 1"]);
        req.conn.host = Some("127.0.0.1".into());
        req.conn.port = Some("1".into());
        let c = policy("unverified", None, ClassKind::Local, ClassKind::Local);
        let line = c.run(&req).to_line();
        let out: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            (out["ok"].clone(), out["stage"].clone()),
            (json!(false), json!("connect"))
        );
        assert!(!line.contains(PASSWORD) && !line.contains(USER), "{line}");
    }
}
