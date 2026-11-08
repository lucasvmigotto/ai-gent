//! The PostgreSQL connector (feature `postgres`, spec 043 T030-T032): the sync `postgres` client over
//! `tokio-postgres`, TLS by rustls through [`crate::core::tls`].
//!
//! Promoted from the spike (`rust/spikes/dbrun/src/drivers/pg.rs`) with its review findings as tests:
//!
//! - the row count is the server's `CommandComplete` number, never read from the statement text (finding 1);
//! - `max_rows` caps the rows in both modes and `truncated` is only ever set in read mode (finding 2);
//! - errors go through `as_db_error()`: SQLSTATE and message, never DETAIL (which can quote row data).
//!
//! Read mode is refused a write by the server itself (SQLSTATE `25006`), in three steps. The session
//! option `default_transaction_read_only=on` and a `BEGIN READ ONLY` transaction make the server refuse a
//! write. A statement can still end the transaction (`COMMIT`) or turn the session default off (`SET
//! default_transaction_read_only = off`, `SET SESSION CHARACTERISTICS ...`), so after every statement the
//! connector asks `SHOW transaction_read_only` and, when it is not `on`, rolls back and fails before any
//! further statement runs. What is enforced is therefore: no statement runs while the session or the
//! transaction is writable, except the one that made it writable, which is transaction control or a
//! `SET` (a write would have been refused while read-only).
//!
//! TLS (T032): the connection is always encrypted (`sslmode=require` semantics: a server that answers
//! `N` to the SSL request is a failure, never a plain-text fallback). The certificate is verified by
//! default, against the system roots or `tls_ca_file`; `unverified` exists only through
//! [`TlsSettings::authorize`], which refuses it unless the profile is proven local and the host is
//! loopback. A unix-socket host (a path starting with `/`, Unix only) never leaves the machine and is
//! the one case that uses no TLS.
//!
//! Every wait is bound. The client's `connect_timeout` covers the TCP connect only, so the connector
//! waits for the worker's "connected" signal for `connect_timeout` plus a small grace and fails at the
//! `connect` stage when it does not come; the statements have `statement_timeout`, `lock_timeout` and
//! `idle_in_transaction_session_timeout` on the server, and the whole request has a deadline the
//! connector enforces itself, since the client has no read timeout.
//!
//! An abandoned request never commits: the worker and the caller share a gate. The caller sets
//! `abandoned` on a timeout; the worker checks it before `BEGIN`, before each statement and before
//! `COMMIT`, and rolls back when it is set. If the worker had already passed the commit point, the
//! caller reports `commit outcome unknown` (stage `execute`) instead.
use super::{Connector, ExecResult, Mode, Request, Stage, StatementResult, request};
use crate::core::explain_sql;
use crate::core::tls::{self, Authorized, TlsContext, TlsError, TlsSettings};
use ::postgres::tls::MakeTlsConnect;
use ::postgres::types::Type;
use ::postgres::{Client, Config, SimpleQueryMessage, config::SslMode};
use serde_json::{Value, json};
use std::error::Error as _;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use tokio_postgres_rustls::MakeRustlsConnect;

/// Lock wait, as `LOCK_TIMEOUT_MS` in `dbrun.py`.
const LOCK_TIMEOUT_MS: u64 = 5000;
/// `idle_in_transaction_session_timeout`, as `dbrun.py`.
const IDLE_IN_TRANSACTION_MS: u64 = 60_000;
/// TCP connect timeout, as `connect_timeout=10` in `dbrun.py`.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Extra time over the statements' own timeouts before the connector gives up on a silent server.
const GRACE: Duration = Duration::from_secs(5);
/// Most of the grace the connect phase may add to `connect_timeout` (authentication, startup).
const CONNECT_GRACE: Duration = Duration::from_secs(2);
/// `application_name`, as `dbrun.py`.
const APPLICATION_NAME: &str = "ai-gent-dbrun";
const IDENTITY: &str =
    "SELECT version(), COALESCE(inet_server_addr()::text, 'socket'), current_database()";
const LINKS: &str = "SELECT (SELECT count(*) FROM pg_extension WHERE extname IN ('postgres_fdw','dblink','mysql_fdw','oracle_fdw','tds_fdw','mongo_fdw','file_fdw')) + (SELECT count(*) FROM pg_foreign_server)";

/// Executes requests against a PostgreSQL server over TLS.
///
/// There is no `Default`: a connector exists only with TLS settings that passed the policy
/// ([`TlsSettings::authorize`]), which also fixes the host it may connect to.
#[derive(Debug, Clone)]
pub struct PostgresConnector {
    tls: Authorized,
    connect_timeout: Duration,
    grace: Duration,
}

impl PostgresConnector {
    /// A connector for settings that passed the policy. The request's host must be the one the policy
    /// was checked against.
    #[must_use]
    pub fn new(tls: Authorized) -> Self {
        Self {
            tls,
            connect_timeout: CONNECT_TIMEOUT,
            grace: GRACE,
        }
    }

    /// Applies the TLS policy to a profile's settings and builds the connector.
    ///
    /// # Errors
    /// [`TlsError`] naming the field when the settings are refused (for instance `unverified` on a
    /// production profile or on a remote host).
    pub fn authorized(settings: &TlsSettings, ctx: &TlsContext<'_>) -> Result<Self, TlsError> {
        Ok(Self::new(settings.authorize(ctx)?))
    }

    /// Overrides the TCP connect timeout and the grace over the statements' timeouts (tests).
    #[must_use]
    pub fn with_limits(mut self, connect_timeout: Duration, grace: Duration) -> Self {
        self.connect_timeout = connect_timeout;
        self.grace = grace;
        self
    }
}

impl Connector for PostgresConnector {
    fn engines(&self) -> &'static [&'static str] {
        &["postgresql"]
    }

    fn execute(&self, req: &Request) -> ExecResult {
        let started = Instant::now();
        let plan = match self.prepare(req) {
            Ok(p) => p,
            Err(e) => return ExecResult::failure(Stage::Connect, e),
        };
        let statements = u32::try_from(req.statements.len()).unwrap_or(u32::MAX);
        let per_statement = request::timeout(req.timeout).unwrap_or(Duration::from_secs(30));
        let run_deadline = self.grace + per_statement.saturating_mul(statements.saturating_add(3));
        let connect_wait = self.connect_timeout + self.grace.min(CONNECT_GRACE);
        let gate = Arc::new(Mutex::new(Gate::default()));
        let (tx, rx) = mpsc::channel();
        let request = req.clone();
        let worker_gate = Arc::clone(&gate);
        let spawned = std::thread::Builder::new()
            .name("dbrun-postgres".into())
            .spawn(move || {
                let done = run(&request, &plan, &worker_gate, &tx);
                let _ = tx.send(Msg::Done(done));
            });
        if spawned.is_err() {
            return ExecResult::failure(Stage::Connect, "could not start the connection thread");
        }
        // Phase 1: connected (the client's own timeout covers the TCP connect only).
        match rx.recv_timeout(connect_wait) {
            Ok(Msg::Connected) => {}
            Ok(Msg::Done(r)) => return finish(r, started),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                abandon(&gate);
                return ExecResult::failure(
                    Stage::Connect,
                    "no connection within the connect timeout; the attempt was abandoned",
                );
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return ExecResult::failure(Stage::Connect, "the connection thread failed");
            }
        }
        // Phase 2: the probes, the statements and the closing round trips.
        let result = match rx.recv_timeout(run_deadline) {
            Ok(Msg::Done(r)) => r,
            // The thread may still hold its socket; the result never waits for it.
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let committing = abandon(&gate);
                ExecResult::failure(
                    Stage::Execute,
                    if committing {
                        "commit outcome unknown: the server did not answer COMMIT within the request's deadline; check the data before running it again"
                    } else {
                        "the server did not answer within the request's deadline; the request was abandoned and nothing was committed"
                    },
                )
            }
            Ok(Msg::Connected) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                ExecResult::failure(Stage::Execute, "the connection thread failed")
            }
        };
        finish(result, started)
    }
}

fn finish(mut result: ExecResult, started: Instant) -> ExecResult {
    if result.seconds.is_none() && result.stage != Some(Stage::Connect) {
        result.seconds = Some(started.elapsed().as_secs_f64());
    }
    result
}

/// What the caller and the worker share.
#[derive(Default)]
struct Gate {
    /// The caller gave up: the worker must not begin, run another statement or commit.
    abandoned: bool,
    /// The worker is past the point of no return: `COMMIT` was sent.
    committing: bool,
}

enum Msg {
    Connected,
    Done(ExecResult),
}

type Shared = Arc<Mutex<Gate>>;

/// Marks the request abandoned; returns whether the worker was already committing.
fn abandon(gate: &Shared) -> bool {
    let mut g = gate
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if g.committing {
        return true;
    }
    g.abandoned = true;
    false
}

fn proceed(gate: &Shared) -> bool {
    !gate
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .abandoned
}

/// The worker's last look before `COMMIT`: false when abandoned, otherwise marks the commit point.
fn begin_commit(gate: &Shared) -> bool {
    let mut g = gate
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if g.abandoned {
        return false;
    }
    g.committing = true;
    true
}

/// Everything `run` needs that is decided before a thread starts: the driver configuration and the
/// TLS connector (or none for a unix socket).
struct Plan {
    config: Config,
    tls: Option<TlsConnect>,
}

/// The rustls connector, with the name the certificate is checked against.
#[derive(Clone)]
struct TlsConnect {
    inner: MakeRustlsConnect,
    name: String,
}

impl<S> MakeTlsConnect<S> for TlsConnect
where
    MakeRustlsConnect: MakeTlsConnect<S>,
{
    type Stream = <MakeRustlsConnect as MakeTlsConnect<S>>::Stream;
    type TlsConnect = <MakeRustlsConnect as MakeTlsConnect<S>>::TlsConnect;
    type Error = <MakeRustlsConnect as MakeTlsConnect<S>>::Error;

    fn make_tls_connect(&mut self, _host: &str) -> Result<Self::TlsConnect, Self::Error> {
        // `tls_server_name` (or the host) decides the name, not what the driver passes.
        self.inner.make_tls_connect(&self.name)
    }
}

impl PostgresConnector {
    /// Validates the request's connection facts and builds the driver configuration. Messages are fixed
    /// text or name a field, never a value.
    fn prepare(&self, req: &Request) -> Result<Plan, String> {
        let c = &req.conn;
        let host = c.host.as_deref().map(str::trim).unwrap_or_default();
        if host.is_empty() {
            return Err("missing conn.host".into());
        }
        if host != self.tls.host().trim() {
            return Err(
                "the TLS policy was checked for another host than the request names; connect through the profile's own host"
                    .into(),
            );
        }
        let port = match c.port.as_deref() {
            None | Some("") => 5432,
            Some(p) => p
                .parse::<u16>()
                .ok()
                .filter(|p| *p != 0)
                .ok_or("invalid port")?,
        };
        let timeout = request::timeout(req.timeout).map_err(String::from)?;
        let mut options = format!(
            "-c statement_timeout={} -c lock_timeout={LOCK_TIMEOUT_MS} -c idle_in_transaction_session_timeout={IDLE_IN_TRANSACTION_MS}",
            timeout.as_millis().max(1)
        );
        // Pinned so the value normalisation of `text_value` cannot be defeated by a server's own setting.
        options.push_str(" -c DateStyle=ISO -c bytea_output=hex");
        if req.mode != Mode::Write {
            options.push_str(" -c default_transaction_read_only=on");
        }
        let mut config = Config::new();
        config
            .host(host)
            .port(port)
            .connect_timeout(self.connect_timeout)
            .application_name(APPLICATION_NAME)
            .options(&options);
        if let Some(v) = &c.database {
            config.dbname(v);
        }
        if let Some(v) = &c.user {
            config.user(v);
        }
        if let Some(v) = &c.password {
            config.password(v);
        }
        if is_unix_socket(host) {
            config.ssl_mode(SslMode::Disable);
            return Ok(Plan { config, tls: None });
        }
        // `Require`: a server that does not offer TLS is a failure, never a plain-text session.
        config.ssl_mode(SslMode::Require);
        let client = tls::client_config(&self.tls).map_err(|e| e.to_string())?;
        let name = tls::server_name(self.tls.settings(), host).map_err(|e| e.to_string())?;
        Ok(Plan {
            config,
            tls: Some(TlsConnect {
                inner: MakeRustlsConnect::new(client),
                name: name.to_str().into_owned(),
            }),
        })
    }
}

fn is_unix_socket(host: &str) -> bool {
    cfg!(unix) && host.starts_with('/')
}

/// The message of a failure. A server error goes through the client's database-error type (SQLSTATE and
/// message, never DETAIL or HINT, which can quote row data); anything else is the error and its source
/// chain, with the profile field to set when the cause is the TLS handshake.
fn describe(e: &::postgres::Error) -> String {
    if let Some(db) = e.as_db_error() {
        return format!("DbError {}: {}", db.code().code(), db.message());
    }
    let mut text = e.to_string();
    let mut source = e.source();
    while let Some(s) = source {
        text.push_str(": ");
        text.push_str(&s.to_string());
        source = s.source();
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains("does not support tls") {
        text.push_str(" (the server must offer TLS; this runner never falls back to plain text)");
    } else if lower.contains("tls")
        || lower.contains("certificate")
        || lower.contains("invalid peer")
    {
        text.push_str(" (check tls_ca_file and tls_server_name in the profile)");
    }
    text
}

fn connect(plan: &Plan) -> Result<Client, ::postgres::Error> {
    match &plan.tls {
        Some(tls) => plan.config.connect(tls.clone()),
        None => plan.config.connect(::postgres::NoTls),
    }
}

/// Converts a text value by its column type, as `to_jsonable` shows what psycopg returns.
fn text_value(ty: &Type, v: Option<&str>) -> Value {
    let Some(v) = v else { return Value::Null };
    match *ty {
        Type::INT2 | Type::INT4 | Type::INT8 | Type::OID => {
            v.parse::<i64>().map_or_else(|_| json!(v), |n| json!(n))
        }
        Type::FLOAT4 | Type::FLOAT8 => match v.parse::<f64>() {
            Ok(f) if f.is_finite() => json!(f),
            _ => json!(v),
        },
        Type::BOOL => json!(v == "t"),
        Type::BYTEA => json!(format!(
            "<{} bytes>",
            v.strip_prefix("\\x").map_or(v.len(), |h| h.len() / 2)
        )),
        // Python's isoformat(): `T` separator and a `+HH:MM` offset.
        Type::TIMESTAMP | Type::TIMESTAMPTZ => {
            let mut s = v.replacen(' ', "T", 1);
            if *ty == Type::TIMESTAMPTZ
                && s.len() > 3
                && matches!(s.as_bytes()[s.len() - 3], b'+' | b'-')
            {
                s.push_str(":00");
            }
            json!(s)
        }
        _ => json!(v),
    }
}

/// The result of one statement from what the server reported. `count` is the command-tag number of
/// `CommandComplete` (never derived from the statement text, so comments, CTEs and `RETURNING` cannot
/// change it). A statement whose tag carries no number (DDL, `EXPLAIN`, `SHOW`) reports `0`: the
/// contract's DDL count (D-001), where psycopg reports `-1`. `max_rows` caps the rows in both modes;
/// `truncated` is set in read mode only.
fn shape_entry(
    columns: &[(String, Type)],
    rows: Vec<Vec<Value>>,
    count: u64,
    cap: usize,
    read: bool,
) -> Result<StatementResult, &'static str> {
    let rowcount = request::rowcount(request::CountKind::Dml, i64::try_from(count).ok())?;
    if columns.is_empty() {
        return Ok(StatementResult {
            rowcount,
            columns: None,
            rows: None,
            truncated: None,
        });
    }
    let (rows, truncated) = request::cap_rows(rows, read, cap);
    Ok(StatementResult {
        rowcount,
        columns: Some(columns.iter().map(|c| c.0.clone()).collect()),
        rows: Some(rows),
        truncated: Some(truncated),
    })
}

/// Runs one statement with the simple query protocol (text values, converted by the column type from a
/// prepare, which also refuses a second statement in the same text, as psycopg's extended protocol).
fn run_statement(
    client: &mut Client,
    sql: &str,
    cap: usize,
    read: bool,
) -> Result<Result<StatementResult, &'static str>, ::postgres::Error> {
    let columns: Vec<(String, Type)> = client
        .prepare(sql)?
        .columns()
        .iter()
        .map(|c| (c.name().to_owned(), c.type_().clone()))
        .collect();
    let (mut rows, mut count) = (vec![], 0u64);
    for message in client.simple_query(sql)? {
        match message {
            SimpleQueryMessage::Row(row) => {
                // One past the cap is kept so read mode can tell it truncated; the rest are not converted.
                // `simple_query` has already buffered every row the server sent: `max_rows` bounds the
                // output, not memory (as psycopg's client-side cursor; recorded in differences.md).
                if rows.len() > cap {
                    continue;
                }
                let mut values = vec![];
                for (i, (_, ty)) in columns.iter().enumerate() {
                    values.push(text_value(ty, row.try_get(i)?));
                }
                rows.push(values);
            }
            SimpleQueryMessage::CommandComplete(n) => count = n,
            _ => {}
        }
    }
    Ok(shape_entry(&columns, rows, count, cap, read))
}

fn first_row(client: &mut Client, sql: &str) -> Result<Vec<Option<String>>, ::postgres::Error> {
    for message in client.simple_query(sql)? {
        if let SimpleQueryMessage::Row(row) = message {
            return (0..row.len())
                .map(|i| row.try_get(i).map(|v| v.map(str::to_owned)))
                .collect();
        }
    }
    Ok(vec![])
}

fn run(req: &Request, plan: &Plan, gate: &Shared, tx: &mpsc::Sender<Msg>) -> ExecResult {
    let started = Instant::now();
    let mut client = match connect(plan) {
        Ok(c) => c,
        Err(e) => return ExecResult::failure(Stage::Connect, describe(&e)),
    };
    // Connected after the caller gave up: close without running anything.
    if !proceed(gate) {
        return ExecResult::failure(Stage::Connect, "abandoned");
    }
    let _ = tx.send(Msg::Connected);
    let mut result = ExecResult::success();
    let outcome = execute_in(&mut client, req, &mut result, gate);
    match outcome {
        Ok(()) => {}
        Err(Failure::Driver(e)) => {
            let _ = client.batch_execute("ROLLBACK");
            result.ok = false;
            result.stage = Some(Stage::Execute);
            result.error = Some(describe(&e));
        }
        Err(Failure::Refused(message)) => {
            let _ = client.batch_execute("ROLLBACK");
            result.ok = false;
            result.stage = Some(Stage::Execute);
            result.error = Some(message.into());
        }
        Err(Failure::Abandoned) => {
            // The caller has its answer already; nothing may stay open or commit.
            let _ = client.batch_execute("ROLLBACK");
            result.ok = false;
            result.stage = Some(Stage::Execute);
            result.error = Some("abandoned".into());
        }
    }
    result.seconds = Some(started.elapsed().as_secs_f64());
    result
}

enum Failure {
    Abandoned,
    Driver(::postgres::Error),
    Refused(&'static str),
}

impl From<::postgres::Error> for Failure {
    fn from(e: ::postgres::Error) -> Self {
        Self::Driver(e)
    }
}

fn execute_in(
    client: &mut Client,
    req: &Request,
    result: &mut ExecResult,
    gate: &Shared,
) -> Result<(), Failure> {
    let ident = first_row(client, IDENTITY)?;
    result.server = Some(
        ident
            .into_iter()
            .map(|v| v.map_or(Value::Null, Value::String))
            .collect(),
    );
    let links = first_row(client, LINKS)?;
    result.links = Some(
        links
            .first()
            .and_then(|v| v.as_deref())
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0),
    );
    if req.mode == Mode::Test {
        return Ok(());
    }
    let read = req.mode != Mode::Write;
    if !proceed(gate) {
        return Err(Failure::Abandoned);
    }
    // Read: the session default is already read-only and the transaction is opened READ ONLY; both are
    // checked again after every statement (see the module docs).
    client.batch_execute(if read { "BEGIN READ ONLY" } else { "BEGIN" })?;
    let mut counts = Vec::new();
    for stmt in &req.statements {
        if !proceed(gate) {
            return Err(Failure::Abandoned);
        }
        let sql = if req.explain {
            explain_sql("postgresql", stmt)
        } else {
            stmt.clone()
        };
        let entry = run_statement(client, &sql, req.max_rows, read)?.map_err(Failure::Refused)?;
        counts.push(entry.rowcount);
        result.results.get_or_insert_with(Vec::new).push(entry);
        if read {
            // A statement can end the transaction or turn the session default off: never run another
            // one in a writable session.
            let state = first_row(client, "SHOW transaction_read_only")?;
            if state.first().and_then(|v| v.as_deref()) != Some("on") {
                return Err(Failure::Refused(
                    "read-only enforcement was lost (transaction_read_only is not on after a statement); rolled back, no further statement ran",
                ));
            }
        }
    }
    if read {
        client.batch_execute("ROLLBACK")?;
        return Ok(());
    }
    if let Some(error) = request::verify_counts(&req.expect, &counts) {
        client.batch_execute("ROLLBACK")?;
        result.ok = false;
        result.stage = Some(Stage::Verify);
        result.error = Some(error);
    } else {
        if !begin_commit(gate) {
            return Err(Failure::Abandoned);
        }
        client.batch_execute("COMMIT")?;
        result.committed = Some(true);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cols() -> Vec<(String, Type)> {
        vec![("id".to_string(), Type::INT4)]
    }

    fn rows(n: i32) -> Vec<Vec<Value>> {
        (1..=n).map(|i| vec![json!(i)]).collect()
    }

    #[test]
    fn text_values_follow_to_jsonable() {
        assert_eq!(text_value(&Type::INT4, Some("7")), json!(7));
        assert_eq!(text_value(&Type::FLOAT8, Some("1.5")), json!(1.5));
        assert_eq!(text_value(&Type::FLOAT8, Some("NaN")), json!("NaN"));
        assert_eq!(text_value(&Type::BOOL, Some("t")), json!(true));
        assert_eq!(text_value(&Type::BOOL, Some("f")), json!(false));
        assert_eq!(text_value(&Type::NUMERIC, Some("1.50")), json!("1.50"));
        assert_eq!(
            text_value(&Type::BYTEA, Some("\\x0102")),
            json!("<2 bytes>")
        );
        assert_eq!(
            text_value(&Type::TIMESTAMPTZ, Some("2026-01-02 03:04:05+00")),
            json!("2026-01-02T03:04:05+00:00")
        );
        assert_eq!(
            text_value(&Type::TIMESTAMP, Some("2026-01-02 03:04:05")),
            json!("2026-01-02T03:04:05")
        );
        assert_eq!(text_value(&Type::TEXT, None), Value::Null);
    }

    #[test]
    fn a_count_the_server_could_not_give_is_refused_never_guessed() {
        // The counts themselves come from the wire (see tests/connector_postgres.rs, command tags).
        assert!(shape_entry(&[], vec![], u64::MAX, 200, false).is_err());
        assert!(shape_entry(&[], vec![], u64::try_from(i64::MAX).unwrap(), 200, false).is_ok());
    }

    #[test]
    fn max_rows_caps_returned_rows_in_both_modes() {
        // Write mode (UPDATE ... RETURNING): capped, truncated stays false as in dbrun.py, rowcount is the server's.
        let w = shape_entry(&cols(), rows(5), 5, 2, false).unwrap();
        assert_eq!(w.rows, Some(rows(2)));
        assert_eq!((w.truncated, w.rowcount), (Some(false), 5));
        // Read mode: capped and flagged.
        let r = shape_entry(&cols(), rows(5), 5, 2, true).unwrap();
        assert_eq!((r.rows, r.truncated), (Some(rows(2)), Some(true)));
        // Exactly at the cap is not truncated; zero keeps nothing and flags a non-empty result.
        assert_eq!(
            shape_entry(&cols(), rows(2), 2, 2, true).unwrap().truncated,
            Some(false)
        );
        let zero = shape_entry(&cols(), rows(1), 1, 0, true).unwrap();
        assert_eq!((zero.rows, zero.truncated), (Some(vec![]), Some(true)));
    }

    #[test]
    fn a_unix_socket_host_is_recognised_only_on_unix() {
        assert_eq!(is_unix_socket("/var/run/postgresql"), cfg!(unix));
        assert!(!is_unix_socket("db.internal"));
        assert!(!is_unix_socket("127.0.0.1"));
    }
}
