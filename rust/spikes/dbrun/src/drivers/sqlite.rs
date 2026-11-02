//! E4: bundled SQLite execution; Python still owns policy, plans and backups.
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

fn connect(req: &Value) -> rusqlite::Result<Connection> {
    let path = req["conn"]["path"].as_str().unwrap();
    // Encode reserved URI characters so a literal filename cannot add parameters.
    let encoded: String = path
        .bytes()
        .map(|b| match b {
            b'%' | b'?' | b'#' => format!("%{b:02X}"),
            _ if !b.is_ascii() => format!("%{b:02X}"),
            _ => (b as char).to_string(),
        })
        .collect();
    let read = req["mode"] != "write";
    let flags = if read {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    };
    let uri = format!("file:{encoded}{}", if read { "?mode=ro" } else { "" });
    let mut conn = Connection::open_with_flags(uri, flags | OpenFlags::SQLITE_OPEN_URI)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    let deadline =
        Instant::now() + Duration::from_secs_f64(req["timeout"].as_f64().unwrap_or(30.0));
    conn.progress_handler(10000, Some(move || Instant::now() > deadline))?;
    if read {
        conn.execute_batch("PRAGMA query_only = ON")?;
    }
    Ok(conn)
}

pub fn execute(req: &Value) -> Value {
    let started = Instant::now();
    let conn = match connect(req) {
        Ok(c) => c,
        Err(e) => return json!({"ok": false, "stage": "connect", "error": e.to_string()}),
    };
    let mut result = json!({"ok": true, "results": []});
    let run = (|| -> rusqlite::Result<()> {
        let version: String = conn.query_row("SELECT sqlite_version()", [], |r| r.get(0))?;
        result["server"] = json!([format!("SQLite {version}"), "file", "main"]);
        result["links"] = json!(conn.query_row(
            "SELECT COUNT(*) - 1 FROM pragma_database_list",
            [],
            |r| r.get::<_, i64>(0)
        )?);
        if req["mode"] == "test" {
            return Ok(());
        }
        let read = req["mode"] != "write";
        let cap = req["max_rows"].as_u64().unwrap_or(200) as usize;
        if !read {
            conn.execute_batch("BEGIN")?;
        }
        for sql in req["statements"].as_array().unwrap() {
            let sql = sql.as_str().unwrap();
            let sql = if req["explain"] == true {
                crate::explain_sql("sqlite", sql)
            } else {
                sql.to_owned()
            };
            let mut statement = conn.prepare(&sql)?;
            let mut entry = if statement.column_count() == 0 {
                json!({"rowcount": statement.execute([])?})
            } else {
                let columns: Vec<String> =
                    statement.column_names().map(|c| c.to_string()).collect();
                let mut query = statement.query([])?;
                let mut rows = vec![];
                let mut truncated = false;
                while let Some(row) = query.next()? {
                    if read && rows.len() == cap {
                        truncated = true;
                        break;
                    }
                    let mut values = vec![];
                    for i in 0..columns.len() {
                        values.push(match row.get_ref(i)? {
                            ValueRef::Null => Value::Null,
                            ValueRef::Integer(v) => json!(v),
                            ValueRef::Real(v) => json!(v),
                            ValueRef::Text(v) => json!(String::from_utf8_lossy(v)),
                            ValueRef::Blob(v) => json!(format!("<{} bytes>", v.len())),
                        });
                    }
                    rows.push(values);
                }
                json!({"rowcount": -1, "columns": columns, "rows": rows.into_iter().take(cap).collect::<Vec<_>>(), "truncated": truncated})
            };
            result["results"].as_array_mut().unwrap().push(entry.take());
        }
        if !read {
            let mut mismatches = vec![];
            if let Some(expect) = req["expect"].as_array() {
                for (i, expected) in expect.iter().enumerate() {
                    if !expected.is_null()
                        && i < result["results"].as_array().unwrap().len()
                        && *expected != result["results"][i]["rowcount"]
                    {
                        mismatches.push(format!(
                            "statement {}: expected {}, got {}",
                            i + 1,
                            expected,
                            result["results"][i]["rowcount"]
                        ));
                    }
                }
            }
            if mismatches.is_empty() {
                conn.execute_batch("COMMIT")?;
                result["committed"] = json!(true);
            } else {
                conn.execute_batch("ROLLBACK")?;
                result["ok"] = json!(false);
                result["stage"] = json!("verify");
                result["error"] = json!(format!(
                    "row counts differ from the plan: {}",
                    mismatches.join(", ")
                ));
            }
        }
        Ok(())
    })();
    if let Err(e) = run {
        if !conn.is_autocommit() {
            let _ = conn.execute_batch("ROLLBACK");
        }
        result["ok"] = json!(false);
        result["stage"] = json!("execute");
        result["error"] = json!(e.to_string());
    }
    result["seconds"] = json!(started.elapsed().as_secs_f64());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_read_only_and_query_only_each_refuse_writes() {
        let dir = std::path::PathBuf::from(std::env::var_os("TMPDIR").expect("TMPDIR required"));
        let path = dir.join(format!(
            "dbrun-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let setup = Connection::open(&path).unwrap();
        setup
            .execute_batch("CREATE TABLE t (n); INSERT INTO t VALUES (1)")
            .unwrap();
        drop(setup);
        let req = json!({"conn": {"path": path}, "mode": "read", "timeout": 5});
        let conn = connect(&req).unwrap();
        let readonly = |error: rusqlite::Error| {
            assert_eq!(
                error.sqlite_error_code(),
                Some(rusqlite::ErrorCode::ReadOnly)
            )
        };
        readonly(conn.execute("UPDATE t SET n=2", []).unwrap_err());
        // TEMP is normally writable even with mode=ro; query_only forbids it.
        readonly(conn.execute("CREATE TEMP TABLE tmp (n)", []).unwrap_err());
        conn.execute_batch("PRAGMA query_only=OFF").unwrap();
        conn.execute("CREATE TEMP TABLE tmp (n)", []).unwrap();
        // Even disabling query_only cannot defeat the underlying read-only open.
        readonly(conn.execute("UPDATE t SET n=2", []).unwrap_err());
        assert_eq!(
            conn.query_row("SELECT n FROM t", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        drop(conn);
        std::fs::remove_file(path).unwrap();
    }
}
