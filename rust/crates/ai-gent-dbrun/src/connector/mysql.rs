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
//!   the connector itself reads every item with a lexer that follows the server's rules ([`server_view`])
//!   and refuses, before connecting, an item with a `;` outside text and comments (one trailing is
//!   fine), an executable comment (`/*! */`, `/*M! */`), a backslash outside text, `DELIMITER`,
//!   `LOCAL INFILE`, an unterminated quote or comment, and anything that changes how the server reads
//!   text (`sql_mode`, the connection character set). The session also drops `NO_BACKSLASH_ESCAPES` and
//!   `ANSI_QUOTES`. In read mode the session is checked to be read only after every statement;
//! - the request timeout is validated before any socket opens. The socket reads and writes are bounded
//!   by `timeout + 5` s each, and reading rows stops at the request timeout; but dropping a result set
//!   makes the crate read the remaining rows and sets, so the bound on a whole call is the child
//!   deadline of T012 (not built yet), not this module;
//! - TLS is on for every connection and goes through [`crate::core::tls`]: the policy (verify by default;
//!   `unverified` only for a proven-local loopback profile, never production), the CA file checks (bounded
//!   read, no private key) and the warning. See [`MysqlConnector::new`] for what the crate cannot do.
use super::{Connector, ExecResult, Mode, Request, Stage, StatementResult, request};
use crate::core::explain_sql;
use crate::core::redact::Redactor;
use crate::core::tls::{
    self, Authorized, ClassKind, TlsContext, TlsMode, TlsSettings, UNVERIFIED_WARNING,
};
use mysql::consts::ColumnType;
use mysql::prelude::{FromRow, Queryable};
use mysql::{Column, Conn, OptsBuilder, SslOpts, Value as My};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::Once;
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
            match server_view(sql) {
                Err(m) => return ExecResult::failure(Stage::Execute, m),
                Ok(view) if req.mode != Mode::Write => {
                    if let Err(m) = read_statement_allowed(&view) {
                        return ExecResult::failure(Stage::Execute, m);
                    }
                }
                Ok(_) => {}
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
        if let Some(error) = &result.error {
            result.error = Some(redact_error(&req.conn, error));
        }
        result
    }
}

/// Redacts an error with the profile's secrets, plus the bracket-less form of an IPv6 host: the crate
/// prints `2001:db8::5:3306` for the host `[2001:db8::5]`. (`Connector::run` redacts again.)
fn redact_error(conn: &super::ConnFacts, message: &str) -> String {
    let mut r = Redactor::from_parts(
        conn.password.as_deref(),
        conn.host.as_deref(),
        conn.user.as_deref(),
    );
    if let Some(h) = conn.host.as_deref() {
        r.add_host(h.trim().trim_start_matches('[').trim_end_matches(']'));
    }
    r.redact(message)
}

const MORE: &str = "refused: an item holds more than one statement; send one statement per item";

/// What the server reads of one request item.
struct Lexed {
    /// The code: comments become a space, text stays as it is, a trailing `;` is gone.
    view: String,
    /// The same with every quoted text and identifier emptied, for word checks.
    bare: String,
}

/// Reads `sql` the way MySQL and MariaDB do, and refuses what it cannot be sure of. The shared scanner
/// of [`crate::core`] follows the Python runner and differs from the server (`SELECT 1--1` has no
/// comment in MySQL, `#` starts one), and this connection allows several statements per item, so a
/// difference here would let a second statement through a read.
///
/// Rules: `'` and `"` text end at a matching quote, a doubled quote is an escaped one, a backslash
/// escapes the next character; `` ` `` identifiers double the backtick; `#` and `-- ` (dash dash and a
/// whitespace, control character or the end) run to the end of the line; `/* */` does not nest.
/// Refused, with a fixed message that holds no part of the text: a `;` in code other than one trailing,
/// code after it, an executable comment (`/*!`, `/*M!`: it is code on the server, so it is not
/// accepted), a backslash or NUL outside text, an unterminated quote or comment.
/// What MySQL and MariaDB count as a blank after `--`: ASCII space, tab, line breaks, other ASCII
/// control characters. (Rust's `is_whitespace` also accepts U+00A0, U+2028, U+3000 ..., which the
/// server does not, so it must not be used here.)
fn is_server_blank(c: char) -> bool {
    c.is_ascii() && (c == ' ' || c.is_ascii_control())
}

fn lex(sql: &str) -> Result<Lexed, &'static str> {
    let b: Vec<char> = sql.chars().collect();
    if b.contains(&'\0') {
        return Err("refused: the statement holds a NUL character");
    }
    let (mut view, mut bare) = (String::new(), String::new());
    let (mut i, mut ended) = (0, false);
    while i < b.len() {
        let c = b[i];
        let at = |k: usize| b.get(i + k).copied();
        let dash_comment = c == '-' && at(1) == Some('-') && at(2).is_none_or(is_server_blank);
        if c == '#' || dash_comment {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            view.push(' ');
            bare.push(' ');
            continue;
        }
        if c == '/' && at(1) == Some('*') {
            if at(2) == Some('!') || (at(2) == Some('M') && at(3) == Some('!')) {
                return Err(
                    "refused: an executable comment (/*! */) is code on the server and is not accepted",
                );
            }
            let mut j = i + 2;
            while j + 1 < b.len() && !(b[j] == '*' && b[j + 1] == '/') {
                j += 1;
            }
            if j + 1 >= b.len() {
                return Err("refused: an unterminated comment");
            }
            i = j + 2;
            view.push(' ');
            bare.push(' ');
            continue;
        }
        if is_server_blank(c) && c.is_ascii() {
            view.push(c);
            bare.push(' ');
            i += 1;
            continue;
        }
        // Unicode spaces and other control characters: the server and Rust disagree about which of
        // them separate tokens or end a `--` comment, so none is accepted outside quoted text.
        if c.is_whitespace() || c.is_control() {
            return Err("refused: a control or non-ASCII space character outside quoted text");
        }
        if ended {
            return Err(MORE);
        }
        match c {
            ';' => {
                ended = true;
                i += 1;
            }
            '\\' => {
                return Err(
                    "refused: a backslash outside text is not valid SQL and may be a client command (\\G)",
                );
            }
            '\'' | '"' | '`' => {
                let mut j = i + 1;
                loop {
                    let Some(&ch) = b.get(j) else {
                        return Err("refused: an unterminated quote");
                    };
                    if ch == '\\' && c != '`' {
                        j += 2;
                    } else if ch == c {
                        if b.get(j + 1) == Some(&c) {
                            j += 2;
                        } else {
                            break;
                        }
                    } else {
                        j += 1;
                    }
                }
                view.extend(&b[i..=j]);
                bare.push(c);
                bare.push(c);
                i = j + 1;
            }
            _ => {
                view.push(c);
                bare.push(c);
                i += 1;
            }
        }
    }
    Ok(Lexed {
        view: view.trim().to_string(),
        bare: bare.to_ascii_lowercase(),
    })
}

/// The SQL as the server reads it, or the refusal. A caller that classifies a statement (read or
/// write) must classify this text and not the item as given, so the classifier and the server agree on
/// what the statement is. The message is fixed text: no part of the statement is echoed.
///
/// # Errors
/// The fixed refusal message, see [`lex`] and the words below. Refused as well: `DELIMITER` (a client
/// command), `LOCAL INFILE` (the crate offers the file-reading capability to the server), and any
/// mention of `sql_mode`, `character_set_client` or `character_set_connection`, `SET NAMES`, `SET
/// CHARACTER SET` and `SET CHARSET`, which would change how the next items are read.
pub fn server_view(sql: &str) -> Result<String, &'static str> {
    let l = lex(sql)?;
    let words: Vec<&str> = l
        .bare
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .collect();
    if words.first() == Some(&"delimiter") {
        return Err("refused: DELIMITER is a client command");
    }
    if words.windows(2).any(|w| w == ["local", "infile"]) {
        return Err("refused: LOCAL INFILE would read a file of this machine");
    }
    let rereads = words.iter().any(|w| {
        matches!(
            *w,
            "sql_mode" | "character_set_client" | "character_set_connection"
        )
    }) || words
        .windows(2)
        .any(|w| w[0] == "set" && matches!(w[1], "names" | "character" | "charset"));
    if rereads {
        return Err(
            "refused: the statement changes how the server reads SQL text (sql_mode or the connection character set)",
        );
    }
    Ok(l.view)
}

/// In read mode the connector itself refuses any statement that does not start with a reading word,
/// so it never depends on a caller's classifier alone (a `CALL` can commit and write inside the
/// server, which a check made after the statement cannot undo). `EXPLAIN ANALYZE` and MariaDB's
/// `ANALYZE <statement>` execute the statement and are refused.
fn read_statement_allowed(view: &str) -> Result<(), &'static str> {
    let lower = view.to_ascii_lowercase();
    let mut words = lower
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty());
    let first = words.next().unwrap_or("");
    let allowed = matches!(
        first,
        "select" | "with" | "show" | "describe" | "desc" | "explain" | "table" | "values"
    );
    if !allowed || (first == "explain" && lower.contains("analyze")) {
        return Err(
            "refused: read mode runs only SELECT, WITH, SHOW, DESCRIBE, EXPLAIN, TABLE and VALUES",
        );
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
    // The lexer reads text the default way: no `NO_BACKSLASH_ESCAPES`, no `ANSI_QUOTES`, and a known charset.
    sql.push(
        "SET SESSION sql_mode = REPLACE(REPLACE(@@sql_mode, 'NO_BACKSLASH_ESCAPES', ''), 'ANSI_QUOTES', '')"
            .to_string(),
    );
    sql.push("SET NAMES utf8mb4".to_string());
    if read {
        sql.push("SET SESSION TRANSACTION READ ONLY".to_string());
    }
    sql
}

/// True when a `sql_mode` value turns on `ANSI_QUOTES` or `NO_BACKSLASH_ESCAPES`, directly or through a
/// composite mode.
fn sql_mode_changes_quoting(mode: &str) -> bool {
    mode.to_ascii_uppercase().split(',').any(|m| {
        matches!(
            m.trim(),
            "ANSI_QUOTES"
                | "NO_BACKSLASH_ESCAPES"
                | "ANSI"
                | "ORACLE"
                | "MSSQL"
                | "DB2"
                | "POSTGRESQL"
                | "MAXDB"
        )
    })
}

/// Runs a query that gives one value or row; a value that cannot be read is a failure that says so
/// and holds none of it (`query_first` would panic with the row in the message).
fn first<T: FromRow>(conn: &mut Conn, sql: &str) -> Result<Option<T>, Failed> {
    match conn.query_first_opt::<T, _>(sql).map_err(Failed::from)? {
        None => Ok(None),
        Some(Ok(v)) => Ok(Some(v)),
        Some(Err(_)) => Err(Failed {
            message: "the server returned a value this runner cannot read".into(),
            connection_lost: false,
        }),
    }
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
    install_crypto_provider();
    let tls = ssl_opts(policy, host)?;
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
        .ssl_opts(Some(tls.ssl.clone()))
        .connect_attrs(Some(attrs))
        .tcp_connect_timeout(Some(CONNECT_TIMEOUT))
        .read_timeout(Some(socket_timeout))
        .write_timeout(Some(socket_timeout));
    let mut conn = Conn::new(opts).map_err(|e| describe_connect(&e))?;
    let setup = |conn: &mut Conn| -> Result<(), Failed> {
        conn.query_drop("SET autocommit = 0")?;
        let version: Option<String> = first(conn, "SELECT VERSION()")?;
        let maria = version.is_some_and(|v| v.to_lowercase().contains("mariadb"));
        for sql in session_statements(maria, timeout, req.mode != Mode::Write) {
            conn.query_drop(sql)?;
        }
        // The lexer assumes default quoting. `REPLACE` removes the two literal flags only, so a
        // composite mode (ANSI, ORACLE, MSSQL, DB2, POSTGRESQL, MAXDB) that implies `ANSI_QUOTES` would
        // survive: read the mode back and refuse to run anything in such a session.
        let mode: Option<String> = first(conn, "SELECT @@session.sql_mode")?;
        if sql_mode_changes_quoting(mode.as_deref().unwrap_or("")) {
            return Err(Failed {
                message: "refused: the session sql_mode changes how quotes are read (ANSI_QUOTES, NO_BACKSLASH_ESCAPES or a mode that implies them)".into(),
                connection_lost: false,
            });
        }
        Ok(())
    };
    setup(&mut conn).map_err(|e| e.message)?;
    // `tls` (and the private copy of the CA file in it) lives until here: the handshake is over.
    drop(tls);
    Ok(conn)
}

/// Installs the rustls `ring` provider once, so a second provider in the build (a future `aws-lc`
/// dependency) cannot make every connect panic on "no default provider". An error means another
/// provider is already installed: that one is used.
fn install_crypto_provider() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// The TLS options of one connection, and the private copy of the CA file they point at.
struct Tls {
    ssl: SslOpts,
    /// Held so the file lives while the client reads it.
    #[allow(dead_code)]
    ca_copy: Option<tempfile::NamedTempFile>,
}

/// The TLS options for one connection, after the policy and the CA file are checked through
/// [`crate::core::tls`].
///
/// The CA file is read once, bounded, and the validated bytes go to a private temporary file whose path
/// the crate gets: the crate reads that file (not a path that could change, and without a size limit of
/// its own). Verify mode with no CA file builds no configuration here: the crate uses its own roots.
fn ssl_opts(policy: &TlsPolicy, host: &str) -> Result<Tls, String> {
    let ctx = TlsContext {
        declared: policy.declared,
        proven: policy.proven,
        host,
    };
    let auth: Authorized = policy.settings.authorize(&ctx).map_err(|e| e.to_string())?;
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if let Some(name) = &auth.settings().server_name
        && !name.eq_ignore_ascii_case(bare)
    {
        return Err("tls_server_name: MySQL and MariaDB check the certificate against the host the connection goes to; set host to the name in the certificate (a different tls_server_name is not supported)".into());
    }
    let mut ssl = SslOpts::default();
    let mut copy = None;
    match auth.settings().mode {
        TlsMode::Verify => {
            if let Some(ca) = &auth.settings().ca_file {
                let bytes = crate::fsutil::read_bytes_bounded(ca, tls::MAX_CA_FILE_BYTES)
                    .map_err(|why| format!("tls_ca_file: {why}"))?;
                let mut file = tempfile::NamedTempFile::new()
                    .map_err(|_| "tls_ca_file: cannot make a private copy for the client")?;
                file.write_all(&bytes)
                    .and_then(|()| file.flush())
                    .map_err(|_| "tls_ca_file: cannot make a private copy for the client")?;
                // The shared checks run on the copy the client will read: no private key, at least
                // one certificate, at most the allowed number.
                let checked = TlsSettings {
                    ca_file: Some(file.path().to_path_buf()),
                    ..auth.settings().clone()
                };
                let auth = checked.authorize(&ctx).map_err(|e| e.to_string())?;
                tls::client_config(&auth).map_err(|e| e.to_string())?;
                ssl = ssl.with_root_cert_path(Some(file.path().to_path_buf()));
                copy = Some(file);
            }
        }
        TlsMode::Unverified => {
            eprintln!("{UNVERIFIED_WARNING}");
            ssl = ssl
                .with_danger_accept_invalid_certs(true)
                .with_danger_skip_domain_validation(true);
        }
    }
    Ok(Tls { ssl, ca_copy: copy })
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
    // A server answer (error 1043 "Bad handshake" among them) is not a certificate failure. A failed
    // handshake is a TLS error or, once the crate wraps it, a codec or I/O error.
    if !matches!(e, mysql::Error::MySqlError(_))
        && (matches!(e, mysql::Error::TlsError(_)) || looks_like_tls(&text))
    {
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
    if is_timeout(e) {
        return format!("{text} (the server did not answer within the request timeout plus 5 s)");
    }
    text
}

/// Whether the error is a socket read or write timeout: by the I/O error kind, so it holds on every
/// platform (`EAGAIN` on Unix, `WSAETIMEDOUT` on Windows).
fn is_timeout(e: &mysql::Error) -> bool {
    use std::io::ErrorKind::{TimedOut, WouldBlock};
    match e {
        mysql::Error::IoError(io)
        | mysql::Error::CodecError(mysql_common::proto::codec::error::PacketCodecError::Io(io)) => {
            matches!(io.kind(), TimedOut | WouldBlock)
        }
        mysql::Error::DriverError(
            mysql::DriverError::Timeout | mysql::DriverError::ConnectTimeout,
        ) => true,
        _ => false,
    }
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
    let (version, host, database): (Option<String>, Option<String>, Option<String>) =
        first(conn, "SELECT VERSION(), @@hostname, DATABASE()")?.unwrap_or_default();
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
    result.links = Some(first::<i64>(conn, links)?.unwrap_or(0));
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
        if read {
            // Defense in depth: whatever the statement was, the session must still be read only.
            let var = if maria {
                "SELECT @@session.tx_read_only"
            } else {
                "SELECT @@session.transaction_read_only"
            };
            if first::<i64>(conn, var)? != Some(1) {
                return Err(Failed {
                    message: "the session is no longer read only after a statement; stopped and rolled back"
                        .into(),
                    connection_lost: false,
                });
            }
        }
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
    fn a_non_ascii_space_after_dash_dash_is_refused_not_read_as_a_comment() {
        // The server reads `--` followed by one of these as two minus signs and the rest as code.
        for blank in ['\u{a0}', '\u{2028}', '\u{3000}', '\u{85}', '\u{2003}'] {
            let sql = format!("SELECT 1 --{blank}; DELETE FROM t");
            assert!(server_view(&sql).is_err(), "{blank:?} accepted");
        }
        assert!(server_view("SELECT 1 -- real comment").is_ok());
        assert!(server_view("SELECT 1 --\n").is_ok());
        // Inside quoted text such characters are just data.
        assert!(server_view("SELECT '\u{a0}'").is_ok());
    }

    #[test]
    fn composite_sql_modes_that_imply_ansi_quotes_are_detected() {
        for m in [
            "ANSI",
            "STRICT_TRANS_TABLES,ORACLE",
            "mssql",
            "ANSI_QUOTES",
            "NO_BACKSLASH_ESCAPES,X",
        ] {
            assert!(sql_mode_changes_quoting(m), "{m}");
        }
        assert!(!sql_mode_changes_quoting(
            "STRICT_TRANS_TABLES,NO_ENGINE_SUBSTITUTION"
        ));
        assert!(!sql_mode_changes_quoting(""));
    }

    #[test]
    fn read_mode_runs_only_reading_statements_and_no_explain_analyze() {
        for ok in [
            "SELECT 1",
            "with x as (select 1) select * from x",
            "SHOW TABLES",
            "DESC t",
            "EXPLAIN SELECT 1",
            "TABLE t",
            "VALUES ROW(1)",
        ] {
            assert!(read_statement_allowed(ok).is_ok(), "{ok}");
        }
        for bad in [
            "CALL p()",
            "DO SLEEP(1)",
            "SET @a = 1",
            "EXPLAIN ANALYZE SELECT 1",
            "ANALYZE DELETE FROM t",
            "DELETE FROM t",
            "COMMIT",
            "LOCK TABLES t WRITE",
            "",
        ] {
            assert!(read_statement_allowed(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_timeout_variable_follows_the_server_family() {
        const SQL_MODE: &str = "SET SESSION sql_mode = REPLACE(REPLACE(@@sql_mode, 'NO_BACKSLASH_ESCAPES', ''), 'ANSI_QUOTES', '')";
        let t = Duration::from_millis(2500);
        assert_eq!(
            session_statements(true, t, false),
            [
                "SET SESSION max_statement_time = 2.5",
                "SET SESSION innodb_lock_wait_timeout = 5",
                "SET SESSION lock_wait_timeout = 5",
                SQL_MODE,
                "SET NAMES utf8mb4",
            ]
        );
        assert_eq!(
            session_statements(false, t, false),
            [
                "SET SESSION MAX_EXECUTION_TIME = 2500",
                "SET SESSION innodb_lock_wait_timeout = 5",
                "SET SESSION lock_wait_timeout = 5",
                SQL_MODE,
                "SET NAMES utf8mb4",
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

    /// Runs the request as given (a refusal must come before any DNS or connect: bounded in time, no
    /// secret in the line), then again pointed at a local listener that must never be connected to.
    /// The second run only happens for a request that has the test host and the default port.
    fn refused_fast(req: &Request, connector: &MysqlConnector) -> serde_json::Value {
        let started = Instant::now();
        let line = connector.run(req).to_line();
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "no connection may be tried: {line}"
        );
        assert_no_secret(&line);
        // A refusal that depends on the host itself (`tls_mode` on a non-loopback host) would not hold
        // for the listener's loopback address, so those are covered by the time bound only.
        let host_bound = line.contains("\"error\":\"tls_mode:");
        if !host_bound
            && req.conn.host.as_deref() == Some(HOST)
            && req.conn.port.as_deref() == Some("3306")
        {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let mut redirected = req.clone();
            redirected.conn.host = Some("127.0.0.1".into());
            redirected.conn.port = Some(listener.local_addr().unwrap().port().to_string());
            let again = connector.run(&redirected).to_line();
            let first: serde_json::Value = serde_json::from_str(&line).unwrap();
            let second: serde_json::Value = serde_json::from_str(&again).unwrap();
            assert_eq!(first["stage"], second["stage"], "{again}");
            assert!(
                matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock),
                "the refused request connected to the listener: {again}"
            );
        }
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

    /// Items the server would run as more than one statement, or that are code the connector cannot
    /// classify. Each must be refused before any connection. The first two are the inputs the shared
    /// scanner read as one statement (`--1` is not a comment in MySQL, `#` is).
    const ADVERSARIAL: &[&str] = &[
        "SELECT 1--1; SET SESSION TRANSACTION READ WRITE; COMMIT; DELETE FROM t; COMMIT",
        "SELECT 1 #'\n; SET SESSION transaction_read_only = 0; COMMIT; DELETE FROM t; COMMIT; #'",
        "SELECT 1 /*!50000 FOR UPDATE */",
        "SELECT 1 /*! INTO OUTFILE '/tmp/x' */",
        "SELECT 1 /*M!100000 FOR UPDATE */",
        "SELECT 1 /*! ; DELETE FROM t */",
        "SELECT 1\\G",
        "SELECT 1 \\g",
        "DELIMITER //",
        "SELECT 1; SELECT 2",
        "SELECT 1;;",
        "SELECT 1 /* x */; DELETE FROM customer",
        "SELECT 'it''s'; DELETE FROM t",
        "SELECT 1 -- x\n; DELETE FROM t",
        "SELECT `a``b`; DELETE FROM t",
        "SELECT 1; /*! */",
        "SELECT 1 /* never closed",
        "SELECT 'never closed",
        "SELECT \"never closed",
        "SELECT `never closed",
        "SELECT 1\0; SELECT 2",
        "SET SESSION sql_mode = 'NO_BACKSLASH_ESCAPES'",
        "SET @@sql_mode = ''",
        "SET NAMES gbk",
        "SET character_set_client = gbk",
        "SET CHARACTER SET gbk",
        "LOAD DATA LOCAL INFILE '/etc/passwd' INTO TABLE t",
        "load data   local\n infile '/etc/hostname' into table t",
        "LOAD DATA/**/LOCAL/**/INFILE '/x' INTO TABLE t",
    ];

    #[test]
    fn every_adversarial_item_is_refused_before_connecting_and_leaks_nothing() {
        for sql in ADVERSARIAL {
            // the pure refusal
            let refusal = server_view(sql).expect_err(sql);
            assert!(refusal.starts_with("refused:"), "{refusal}");
            for secret in [PASSWORD, HOST, USER, "DELETE", "customer"] {
                assert!(!refusal.contains(secret), "{secret} in {refusal}");
            }
            // through the connector, in write mode after a harmless first statement: nothing runs
            for mode in [Mode::Read, Mode::Write] {
                let req = request(mode, &["SELECT 0", sql]);
                let out = refused_fast(&req, &MysqlConnector::default());
                assert_eq!(out["ok"], false, "{sql:?}");
                assert_eq!(out["stage"], "execute", "{sql:?}");
                assert!(out.get("results").is_none(), "nothing may have run: {out}");
                assert!(out.get("server").is_none(), "no connection was made: {out}");
                assert!(
                    out["error"].as_str().unwrap().starts_with("refused:"),
                    "{out}"
                );
            }
        }
    }

    #[test]
    fn text_comments_and_one_trailing_semicolon_are_one_statement() {
        for (ok, view) in [
            ("SELECT 'a;b'", "SELECT 'a;b'"),
            ("SELECT \"a;b\"", "SELECT \"a;b\""),
            ("SELECT `a;b` FROM t", "SELECT `a;b` FROM t"),
            ("SELECT 'it''s;'", "SELECT 'it''s;'"),
            ("SELECT 'a\\'; b'", "SELECT 'a\\'; b'"),
            ("SELECT 1 -- ; DROP TABLE t", "SELECT 1"),
            ("SELECT 1 # ; DROP TABLE t", "SELECT 1"),
            ("SELECT 1 /* ; DROP TABLE t */", "SELECT 1"),
            ("SELECT 1 /*+ MAX_EXECUTION_TIME(10) */", "SELECT 1"),
            ("SELECT 1;", "SELECT 1"),
            ("SELECT 1 ; ", "SELECT 1"),
            ("SELECT 1; -- done", "SELECT 1"),
            ("SELECT 1; /* done */", "SELECT 1"),
            ("SELECT 1 - -1", "SELECT 1 - -1"),
            (
                "SELECT 'local infile', local_x FROM t",
                "SELECT 'local infile', local_x FROM t",
            ),
            (
                "SELECT infile FROM t WHERE local = 1",
                "SELECT infile FROM t WHERE local = 1",
            ),
            (
                "SELECT 'sql_mode', `sql_mode` FROM t",
                "SELECT 'sql_mode', `sql_mode` FROM t",
            ),
        ] {
            let got = server_view(ok).unwrap_or_else(|e| panic!("{ok}: {e}"));
            assert_eq!(
                got.split_whitespace().collect::<Vec<_>>().join(" "),
                view,
                "{ok}"
            );
        }
        // `--1` is code in MySQL, so the view keeps it: the classifier sees what the server sees
        assert_eq!(server_view("SELECT 1--1").unwrap(), "SELECT 1--1");
        // `#` and `-- ` hide the rest of the line from the classifier, as they hide it from the server
        assert!(
            !server_view("SELECT 1 # UPDATE t SET a = 1")
                .unwrap()
                .contains("UPDATE")
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
    fn a_socket_timeout_is_told_apart_by_the_io_error_kind() {
        use mysql_common::proto::codec::error::PacketCodecError;
        use std::io::{Error, ErrorKind};
        for kind in [ErrorKind::WouldBlock, ErrorKind::TimedOut] {
            assert!(
                is_timeout(&mysql::Error::IoError(Error::from(kind))),
                "{kind:?}"
            );
            assert!(
                is_timeout(&mysql::Error::CodecError(PacketCodecError::Io(
                    Error::from(kind)
                ))),
                "{kind:?}"
            );
        }
        assert!(is_timeout(&mysql::Error::DriverError(
            mysql::DriverError::Timeout
        )));
        for kind in [
            ErrorKind::ConnectionRefused,
            ErrorKind::BrokenPipe,
            ErrorKind::UnexpectedEof,
        ] {
            assert!(
                !is_timeout(&mysql::Error::IoError(Error::from(kind))),
                "{kind:?}"
            );
        }
        // the text alone proves nothing: a message that says "timed out" but is not an I/O timeout
        let other = mysql::Error::IoError(Error::other("os error 11) timed out"));
        assert!(!is_timeout(&other));
    }

    #[test]
    fn a_server_answer_is_not_given_the_certificate_hint() {
        // error 1043 "Bad handshake" is the server speaking: no TLS hint
        let bad = mysql::Error::MySqlError(mysql::MySqlError {
            state: "08S01".into(),
            message: "Bad handshake".into(),
            code: 1043,
        });
        let text = describe_connect(&bad);
        assert!(
            text.contains("1043") && !text.contains("tls_ca_file"),
            "{text}"
        );
    }

    #[test]
    fn an_ipv6_host_is_redacted_in_the_form_the_crate_prints() {
        let conn = ConnFacts {
            host: Some("[2001:db8::5]".into()),
            user: Some(USER.into()),
            password: Some(PASSWORD.into()),
            ..ConnFacts::default()
        };
        let crate_text =
            "DriverError { Could not connect to address `2001:db8::5:3306': timed out }";
        let out = redact_error(&conn, crate_text);
        assert!(!out.contains("2001:db8::5"), "{out}");
        assert!(out.contains("<host>"), "{out}");
    }

    #[test]
    fn the_ca_copy_handed_to_the_client_is_the_validated_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let ca = crate::core::tls::testca::TestCa::new("copy").write_ca(dir.path());
        let policy = TlsPolicy {
            settings: TlsSettings::from_fields(None, ca.to_str(), None, None).unwrap(),
            ..TlsPolicy::default()
        };
        let tls = ssl_opts(&policy, "db.example").unwrap();
        let copy = tls.ca_copy.as_ref().expect("a private copy");
        assert_ne!(copy.path(), ca.as_path());
        assert_eq!(
            std::fs::read(copy.path()).unwrap(),
            std::fs::read(&ca).unwrap()
        );
        // no CA file: nothing is copied and no system roots are asked for
        let none = ssl_opts(&TlsPolicy::default(), "db.example").unwrap();
        assert!(none.ca_copy.is_none());
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
