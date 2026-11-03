//! The connector contract: the Python runner's `_exec` JSON, as a trait.
//!
//! `plugins/db/scripts/dbrun.py _exec` reads one JSON request on stdin and prints one JSON result line.
//! [`Request`] and [`ExecResult`] are that request and that line; a [`Connector`] turns one into the
//! other against one family of engines. Policy (classification, plans, backups, masking) stays outside
//! the connector: it only executes what it is given, in the mode it is given.
//!
//! Error shapes (the `stage` of a failed result):
//! - `connect`: the connection could not be opened or the engine is not supported (no `results`, no `seconds`);
//! - `execute`: a statement failed; the transaction is rolled back (`results` holds what ran before);
//! - `verify`: a write ran but its row counts differ from `expect`; rolled back, nothing committed;
//! - `request`: the request line was not valid JSON for [`Request`] (an addition: the Python traceback).
//!
//! Every `error` is redacted by [`Connector::run`]: the profile's password, host and user never leave
//! the connector, whatever the driver quoted back.
//!
//! [`contract`] holds the cases every connector must pass (`rust/fixtures/db-exec/`).
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::fmt;

pub mod contract;
#[cfg(feature = "sqlite")]
pub mod sqlite;

/// What the connector may do: `read` opens read-only and never commits, `write` runs one transaction
/// and commits it only if the row counts match, `test` connects and identifies the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Read-only session.
    Read,
    /// One transaction, committed only when `expect` matches.
    Write,
    /// Connect and identify the server; run no statement.
    Test,
}

/// The connection facts of a profile. Port may arrive as a string or a number. `Debug` never prints
/// the password.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnFacts {
    /// Server host.
    #[serde(deserialize_with = "string_or_number")]
    pub host: Option<String>,
    /// Server port.
    #[serde(deserialize_with = "string_or_number")]
    pub port: Option<String>,
    /// Database name.
    #[serde(deserialize_with = "string_or_number")]
    pub database: Option<String>,
    /// Oracle service name.
    #[serde(deserialize_with = "string_or_number")]
    pub service: Option<String>,
    /// Oracle SID.
    #[serde(deserialize_with = "string_or_number")]
    pub sid: Option<String>,
    /// Login name.
    #[serde(deserialize_with = "string_or_number")]
    pub user: Option<String>,
    /// Login password; redacted from every error.
    #[serde(deserialize_with = "string_or_number")]
    pub password: Option<String>,
    /// SQLite file path.
    #[serde(deserialize_with = "string_or_number")]
    pub path: Option<String>,
}

impl fmt::Debug for ConnFacts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnFacts")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("service", &self.service)
            .field("sid", &self.sid)
            .field("user", &self.user)
            .field("password", &self.password.as_ref().map(|_| "<set>"))
            .field("path", &self.path)
            .finish()
    }
}

fn string_or_number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    match Value::deserialize(d)? {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s)),
        Value::Number(n) => Ok(Some(n.to_string())),
        // No value in the message: it may be a secret.
        _ => Err(D::Error::custom("expected a string or a number")),
    }
}

fn null_as_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

const fn default_timeout() -> f64 {
    30.0
}
const fn default_max_rows() -> usize {
    200
}

/// One execution request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// `postgresql`, `mysql`, `mariadb`, `sqlserver`, `oracle` or `sqlite`.
    pub engine: String,
    /// How to reach the database.
    #[serde(default)]
    pub conn: ConnFacts,
    /// Read, write or test.
    pub mode: Mode,
    /// The statements to run, in order, one per entry.
    #[serde(default)]
    pub statements: Vec<String>,
    /// Statement and lock timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout: f64,
    /// Rows returned per statement in read mode.
    #[serde(default = "default_max_rows")]
    pub max_rows: usize,
    /// Run each statement through the engine's `EXPLAIN`.
    #[serde(default, deserialize_with = "null_as_default")]
    pub explain: bool,
    /// Write mode: the row count each statement must affect (`null` skips a statement).
    #[serde(default, deserialize_with = "null_as_default")]
    pub expect: Vec<Option<i64>>,
}

impl Request {
    /// A request with the contract's defaults for everything but the engine, mode and statements.
    #[must_use]
    pub fn new(engine: &str, conn: ConnFacts, mode: Mode, statements: Vec<String>) -> Self {
        Self {
            engine: engine.to_string(),
            conn,
            mode,
            statements,
            timeout: default_timeout(),
            max_rows: default_max_rows(),
            explain: false,
            expect: Vec::new(),
        }
    }
}

/// Where a failed execution stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    /// Opening the connection (or an unsupported engine).
    Connect,
    /// Running a statement.
    Execute,
    /// Row counts differ from the plan.
    Verify,
    /// The request itself was malformed.
    Request,
}

/// The result of one statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatementResult {
    /// Rows affected; `-1` for a statement that returns rows.
    pub rowcount: i64,
    /// Column names, when the statement returns rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
    /// The rows, JSON values only (decimals and dates as strings, blobs as `<N bytes>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<Vec<Vec<Value>>>,
    /// Read mode: more rows existed than `max_rows`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
}

/// The one JSON result line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecResult {
    /// Whether everything ran (and, in write mode, committed).
    pub ok: bool,
    /// Where a failure stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<Stage>,
    /// The (redacted) failure message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Version, host and database of the server, as the engine reports them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<Vec<Value>>,
    /// Foreign servers or database links the database has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links: Option<i64>,
    /// One entry per statement that ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub results: Option<Vec<StatementResult>>,
    /// Write mode: the transaction committed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed: Option<bool>,
    /// Wall time of the execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f64>,
}

impl ExecResult {
    /// A successful result with no statements run yet.
    #[must_use]
    pub fn success() -> Self {
        Self {
            ok: true,
            stage: None,
            error: None,
            server: None,
            links: None,
            results: Some(Vec::new()),
            committed: None,
            seconds: None,
        }
    }

    /// A failure at `stage` with nothing else filled in.
    #[must_use]
    pub fn failure(stage: Stage, error: impl Into<String>) -> Self {
        Self {
            ok: false,
            stage: Some(stage),
            error: Some(error.into()),
            server: None,
            links: None,
            results: None,
            committed: None,
            seconds: None,
        }
    }

    /// The result as the single JSON line `_exec` prints.
    ///
    /// # Panics
    /// Never: every field serializes.
    #[must_use]
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("an ExecResult always serializes")
    }

    fn redact(&mut self, conn: &ConnFacts) {
        let Some(error) = &self.error else { return };
        let value = |v: &Option<String>| v.clone().unwrap_or_default();
        let (password, host, user) = (value(&conn.password), value(&conn.host), value(&conn.user));
        self.error = Some(crate::core::redact(
            error,
            &[
                (&password, "<password>"),
                (&host, "<host>"),
                (&user, "<user>"),
            ],
        ));
    }
}

/// Runs a [`Request`] against one family of engines.
///
/// An implementation provides [`Connector::engines`] and [`Connector::execute`]; callers use
/// [`Connector::run`] or [`Connector::run_line`], which add the engine check and the redaction.
pub trait Connector {
    /// The engine names this connector serves (`mysql` and `mariadb` share one connector).
    fn engines(&self) -> &'static [&'static str];

    /// Executes the request. Never panics and never returns an `Err`: a failure is an
    /// [`ExecResult`] with `ok: false`. Messages are not yet redacted.
    fn execute(&self, req: &Request) -> ExecResult;

    /// [`Connector::execute`] behind the engine check, with the error redacted.
    fn run(&self, req: &Request) -> ExecResult {
        let mut result = if self.engines().contains(&req.engine.as_str()) {
            self.execute(req)
        } else if KNOWN_ENGINES.contains(&req.engine.as_str()) {
            ExecResult::failure(Stage::Connect, format!("unsupported engine {}", req.engine))
        } else {
            // An unknown name is request data and may be a secret: never echoed.
            ExecResult::failure(Stage::Connect, "unsupported engine")
        };
        result.redact(&req.conn);
        result
    }

    /// One request line in, one result line out: the `_exec` contract itself.
    ///
    /// A malformed request is answered with the name of the offending field only, never its value.
    fn run_line(&self, line: &str) -> String {
        match parse_request(line) {
            Ok(req) => self.run(&req).to_line(),
            Err(field) => {
                ExecResult::failure(Stage::Request, format!("invalid request: {field}")).to_line()
            }
        }
    }
}

/// The engine names the runner knows; only these are echoed in an error.
const KNOWN_ENGINES: &[&str] = &[
    "postgresql",
    "mysql",
    "mariadb",
    "sqlserver",
    "oracle",
    "sqlite",
];

/// Parses a request line; on failure returns the name of the field that is wrong (a fixed string).
fn parse_request(line: &str) -> Result<Request, &'static str> {
    let value: Value = serde_json::from_str(line).map_err(|_| "not valid JSON")?;
    let Some(object) = value.as_object() else {
        return Err("not an object");
    };
    fn ok<T: serde::de::DeserializeOwned>(
        object: &serde_json::Map<String, Value>,
        key: &str,
    ) -> bool {
        object
            .get(key)
            .is_none_or(|v| serde_json::from_value::<T>(v.clone()).is_ok())
    }
    let required = |key: &str| object.contains_key(key);
    if !required("engine") || !ok::<String>(object, "engine") {
        return Err("engine");
    }
    if !required("mode") || !ok::<Mode>(object, "mode") {
        return Err("mode");
    }
    if !ok::<ConnFacts>(object, "conn") {
        return Err("conn");
    }
    if !ok::<Vec<String>>(object, "statements") {
        return Err("statements");
    }
    if !ok::<f64>(object, "timeout") {
        return Err("timeout");
    }
    if !ok::<usize>(object, "max_rows") {
        return Err("max_rows");
    }
    if !ok::<Option<bool>>(object, "explain") {
        return Err("explain");
    }
    if !ok::<Option<Vec<Option<i64>>>>(object, "expect") {
        return Err("expect");
    }
    serde_json::from_value(value).map_err(|_| "request")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Fails every request with a driver-style message that quotes the credentials.
    struct Quoting;
    impl Connector for Quoting {
        fn engines(&self) -> &'static [&'static str] {
            &["postgresql"]
        }
        fn execute(&self, req: &Request) -> ExecResult {
            let c = &req.conn;
            ExecResult::failure(
                Stage::Connect,
                format!(
                    "OperationalError: connection to server at {} failed: password authentication failed for user {} (password {})",
                    c.host.as_deref().unwrap(),
                    c.user.as_deref().unwrap(),
                    c.password.as_deref().unwrap()
                ),
            )
        }
    }

    #[test]
    fn redaction_fixture() {
        let case = contract::load_redaction();
        let conn: ConnFacts = serde_json::from_value(case["conn"].clone()).unwrap();
        let req = Request::new("postgresql", conn, Mode::Test, vec![]);
        let got = Quoting.run(&req);
        assert_eq!(got.error.as_deref(), case["expected_error"].as_str());
        assert_eq!(got.stage, Some(Stage::Connect));
        assert!(got.results.is_none() && got.seconds.is_none());
    }

    #[test]
    fn unsupported_engine_is_a_connect_failure() {
        let line = json!({"engine": "oracle", "conn": {}, "mode": "test"}).to_string();
        let out: Value = serde_json::from_str(&Quoting.run_line(&line)).unwrap();
        assert_eq!(
            out,
            json!({"ok": false, "stage": "connect", "error": "unsupported engine oracle"})
        );
    }

    #[test]
    fn malformed_request_is_one_json_line_without_the_value() {
        for bad in ["not json", r#"{"engine":"postgresql","mode":"drop"}"#] {
            let out = Quoting.run_line(bad);
            assert!(!out.contains('\n'));
            let v: Value = serde_json::from_str(&out).unwrap();
            assert_eq!(
                (v["ok"].clone(), v["stage"].clone()),
                (json!(false), json!("request"))
            );
        }
        let secret = r#"{"engine":"postgresql","mode":"test","conn":{"password":["hunter22"]}}"#;
        assert!(!Quoting.run_line(secret).contains("hunter22"));
    }

    #[test]
    fn a_rejected_request_never_echoes_a_value() {
        let secret = "pw-Sup3rS3cret";
        let bad_mode = json!({"engine": "postgresql", "mode": secret}).to_string();
        let out = Quoting.run_line(&bad_mode);
        assert!(!out.contains(secret), "{out}");
        assert!(out.contains("invalid request: mode"), "{out}");
        let bad_engine = json!({"engine": secret, "mode": "test"}).to_string();
        let out = Quoting.run_line(&bad_engine);
        assert!(!out.contains(secret), "{out}");
        assert!(out.contains("unsupported engine"), "{out}");
    }

    #[test]
    fn request_accepts_the_python_shapes() {
        let req: Request = serde_json::from_value(json!({
            "engine": "mysql", "conn": {"host": "h", "port": 3306, "password": null},
            "mode": "write", "statements": ["UPDATE t SET a = 1"], "timeout": 5, "max_rows": 10,
            "explain": null, "expect": [2, null]
        }))
        .unwrap();
        assert_eq!(req.conn.port.as_deref(), Some("3306"));
        assert_eq!(req.expect, vec![Some(2), None]);
        assert!(!req.explain);
        assert!(
            !format!(
                "{:?}",
                ConnFacts {
                    password: Some("pw-secret".into()),
                    ..ConnFacts::default()
                }
            )
            .contains("pw-secret")
        );
    }

    #[test]
    fn result_line_matches_the_python_keys() {
        let mut ok = ExecResult::success();
        ok.seconds = Some(0.5);
        assert_eq!(
            serde_json::to_value(&ok).unwrap(),
            json!({"ok": true, "results": [], "seconds": 0.5})
        );
        assert_eq!(
            serde_json::to_value(ExecResult::failure(Stage::Verify, "x")).unwrap(),
            json!({"ok": false, "stage": "verify", "error": "x"})
        );
    }
}
