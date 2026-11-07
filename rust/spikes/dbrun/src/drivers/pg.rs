//! E2: PostgreSQL through the sync `postgres` crate (a thin wrapper over `tokio-postgres`), no TLS.
//!
//! Python still owns policy, plans, backups and restore; this executes one `_exec` request.
//! Differences from `dbrun.py` `execute()` are listed in `specs/038-db-runner-spike/results.md` (E2).
use crate::core::redact;
use postgres::types::Type;
use postgres::{Client, Config, NoTls, SimpleQueryMessage};
use serde_json::{json, Value};
use std::error::Error as _;
use std::time::{Duration, Instant};

const LOCK_TIMEOUT_MS: u64 = 5000; // as dbrun.py LOCK_TIMEOUT_MS
const IDENTITY: &str =
    "SELECT version(), COALESCE(inet_server_addr()::text, 'socket'), current_database()";
const LINKS: &str = "SELECT (SELECT count(*) FROM pg_extension WHERE extname IN ('postgres_fdw','dblink','mysql_fdw','oracle_fdw','tds_fdw','mongo_fdw','file_fdw')) + (SELECT count(*) FROM pg_foreign_server)";

/// The message of a failure. A server error goes through the client's database-error type (SQLSTATE and
/// message, never DETAIL, which can quote row data); anything else is the error and its source chain.
fn describe(e: &postgres::Error) -> String {
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
    text
}

fn connect(req: &Value) -> Result<Client, postgres::Error> {
    let c = &req["conn"];
    let timeout_ms = (req["timeout"].as_f64().unwrap_or(30.0) * 1000.0) as u64;
    let mut options = format!(
        "-c statement_timeout={timeout_ms} -c lock_timeout={LOCK_TIMEOUT_MS} -c idle_in_transaction_session_timeout=60000"
    );
    if req["mode"] != "write" {
        options.push_str(" -c default_transaction_read_only=on");
    }
    let text = |k: &str| {
        c[k].as_str()
            .map(str::to_owned)
            .or_else(|| c[k].as_u64().map(|n| n.to_string()))
    };
    let mut config = Config::new();
    config
        .host(&text("host").unwrap_or_default())
        .port(text("port").and_then(|p| p.parse().ok()).unwrap_or(5432))
        .connect_timeout(Duration::from_secs(10))
        .application_name("ai-gent-dbrun")
        .options(&options);
    if let Some(v) = text("database") {
        config.dbname(&v);
    }
    if let Some(v) = text("user") {
        config.user(&v);
    }
    if let Some(v) = text("password") {
        config.password(&v);
    }
    config.connect(NoTls)
}

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

/// Runs one statement with the simple query protocol (text values, converted by the column type from a
/// prepare). Returns the result entry. `cap` is `max_rows`, applied in both modes.
fn run_statement(
    client: &mut Client,
    sql: &str,
    cap: usize,
    read: bool,
) -> Result<Value, postgres::Error> {
    // Prepare: one statement only (as psycopg's extended protocol), and the column types.
    let columns: Vec<(String, Type)> = client
        .prepare(sql)?
        .columns()
        .iter()
        .map(|c| (c.name().to_owned(), c.type_().clone()))
        .collect();
    let (mut rows, mut count) = (vec![], 0i64);
    for message in client.simple_query(sql)? {
        match message {
            SimpleQueryMessage::Row(row) => {
                // One past the cap is kept so read mode can tell it truncated; the rest are not converted.
                if rows.len() > cap {
                    continue;
                }
                let mut values = vec![];
                for (i, (_, ty)) in columns.iter().enumerate() {
                    values.push(text_value(ty, row.try_get(i)?));
                }
                rows.push(values);
            }
            SimpleQueryMessage::CommandComplete(n) => count = n as i64,
            _ => {}
        }
    }
    Ok(shape_entry(&columns, rows, count, cap, read))
}

/// The result entry from what the server reported. The row count is the server's command-tag number
/// (never derived from the statement text, so comments, CTEs and RETURNING cannot change it). Like
/// `dbrun.py`, `max_rows` caps the returned rows in both modes, but only read mode sets `truncated`.
/// Known gap: the client crate drops the tag text, so DDL reports 0 where psycopg reports -1.
fn shape_entry(
    columns: &[(String, Type)],
    mut rows: Vec<Vec<Value>>,
    count: i64,
    cap: usize,
    read: bool,
) -> Value {
    if columns.is_empty() {
        return json!({"rowcount": count});
    }
    let truncated = read && rows.len() > cap;
    rows.truncate(cap);
    json!({"rowcount": count, "columns": columns.iter().map(|c| &c.0).collect::<Vec<_>>(),
           "rows": rows, "truncated": truncated})
}

fn first_row(client: &mut Client, sql: &str) -> Result<Vec<Option<String>>, postgres::Error> {
    for message in client.simple_query(sql)? {
        if let SimpleQueryMessage::Row(row) = message {
            return (0..row.len())
                .map(|i| row.try_get(i).map(|v| v.map(str::to_owned)))
                .collect();
        }
    }
    Ok(vec![])
}

pub fn execute(req: &Value) -> Value {
    let mut result = run(req);
    // The contract's redaction: password, host and user, whatever the server quoted back.
    if let Some(error) = result["error"].as_str() {
        let c = &req["conn"];
        let get = |k: &str| c[k].as_str().unwrap_or_default().to_owned();
        let (password, host, user) = (get("password"), get("host"), get("user"));
        result["error"] = json!(redact(
            error,
            &[(&password, "<password>"), (&host, "<host>"), (&user, "<user>")]
        ));
    }
    result
}

fn run(req: &Value) -> Value {
    let started = Instant::now();
    let mut client = match connect(req) {
        Ok(c) => c,
        Err(e) => return json!({"ok": false, "stage": "connect", "error": describe(&e)}),
    };
    let mut result = json!({"ok": true, "results": []});
    let read = req["mode"] != "write";
    let outcome = (|| -> Result<(), postgres::Error> {
        let ident = first_row(&mut client, IDENTITY)?;
        result["server"] = json!(ident);
        let links = first_row(&mut client, LINKS)?;
        result["links"] = json!(links
            .first()
            .and_then(|v| v.as_deref())
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0));
        if req["mode"] == "test" {
            return Ok(());
        }
        // Read: the session default is already read-only (`default_transaction_read_only=on`); the
        // transaction is also opened READ ONLY, so `SET default_transaction_read_only=off` cannot undo it.
        client.batch_execute(if read { "BEGIN READ ONLY" } else { "BEGIN" })?;
        let cap = req["max_rows"].as_u64().unwrap_or(200) as usize;
        for stmt in req["statements"].as_array().into_iter().flatten() {
            let stmt = stmt.as_str().unwrap_or_default();
            let sql = if req["explain"] == true {
                crate::explain_sql("postgresql", stmt)
            } else {
                stmt.to_owned()
            };
            let entry = run_statement(&mut client, &sql, cap, read)?;
            result["results"].as_array_mut().unwrap().push(entry);
        }
        if read {
            client.batch_execute("ROLLBACK")?;
            return Ok(());
        }
        let done = result["results"].as_array().unwrap().len();
        let mismatches: Vec<String> = req["expect"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter(|(i, e)| !e.is_null() && *i < done && **e != result["results"][*i]["rowcount"])
            .map(|(i, e)| {
                format!(
                    "statement {}: expected {}, got {}",
                    i + 1,
                    e,
                    result["results"][i]["rowcount"]
                )
            })
            .collect();
        if mismatches.is_empty() {
            client.batch_execute("COMMIT")?;
            result["committed"] = json!(true);
        } else {
            client.batch_execute("ROLLBACK")?;
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
        let _ = client.batch_execute("ROLLBACK");
        result["ok"] = json!(false);
        result["stage"] = json!("execute");
        result["error"] = json!(describe(&e));
    }
    result["seconds"] = json!(started.elapsed().as_secs_f64());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_values_follow_to_jsonable() {
        assert_eq!(text_value(&Type::INT4, Some("7")), json!(7));
        assert_eq!(text_value(&Type::FLOAT8, Some("1.5")), json!(1.5));
        assert_eq!(text_value(&Type::BOOL, Some("t")), json!(true));
        assert_eq!(text_value(&Type::NUMERIC, Some("1.50")), json!("1.50"));
        assert_eq!(text_value(&Type::BYTEA, Some("\\x0102")), json!("<2 bytes>"));
        assert_eq!(
            text_value(&Type::TIMESTAMPTZ, Some("2026-01-02 03:04:05+00")),
            json!("2026-01-02T03:04:05+00:00")
        );
        assert_eq!(text_value(&Type::TEXT, None), Value::Null);
    }

    #[test]
    fn rowcount_is_the_servers_count_whatever_the_statement_text() {
        // No columns: the entry is the server's count, as for `/* c */ UPDATE ...` or `WITH ... DELETE`.
        assert_eq!(shape_entry(&[], vec![], 1, 200, false), json!({"rowcount": 1}));
        assert_eq!(shape_entry(&[], vec![], 0, 200, false), json!({"rowcount": 0}));
    }

    #[test]
    fn max_rows_caps_returned_rows_in_both_modes() {
        let cols = vec![("id".to_string(), Type::INT4)];
        let rows = || (1..=5).map(|i| vec![json!(i)]).collect::<Vec<_>>();
        // Write mode (UPDATE ... RETURNING): capped, truncated stays false as in dbrun.py, rowcount is the server's.
        let w = shape_entry(&cols, rows(), 5, 2, false);
        assert_eq!(w["rows"], json!([[1], [2]]));
        assert_eq!((w["truncated"].clone(), w["rowcount"].clone()), (json!(false), json!(5)));
        // Read mode: capped and flagged.
        let r = shape_entry(&cols, rows(), 5, 2, true);
        assert_eq!((r["rows"].clone(), r["truncated"].clone()), (json!([[1], [2]]), json!(true)));
        // Exactly at the cap is not truncated.
        assert_eq!(shape_entry(&cols, rows()[..2].to_vec(), 2, 2, true)["truncated"], json!(false));
    }

    #[test]
    fn a_refused_connection_is_a_redacted_connect_failure() {
        // Port 1 on loopback: nothing listens. The user and password must not appear in the error.
        let req = json!({"engine": "postgresql", "mode": "test", "timeout": 2,
            "conn": {"host": "localhost", "port": 1, "user": "app_ro", "password": "s3cr3t!", "database": "shop"}});
        let out = execute(&req);
        assert_eq!((out["ok"].clone(), out["stage"].clone()), (json!(false), json!("connect")));
        assert!(!out.to_string().contains("s3cr3t"));
    }
}
