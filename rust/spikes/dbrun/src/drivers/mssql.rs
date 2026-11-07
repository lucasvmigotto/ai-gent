//! E5: SQL Server through `tiberius-ng` 0.13.1 (pure Rust TDS client, rustls), no ODBC, no FreeTDS.
//!
//! Python still owns policy, plans, backups and restore; this executes one `_exec` request.
//!
//! SQL Server has NO server-side read-only mode: nothing here makes the server refuse a write in read
//! mode. Read mode is the Python classifier (outside this adapter) plus a transaction that is always rolled
//! back, exactly as `dbrun.py`. A write that reached this adapter in read mode would run and then be undone.
//! Differences from `dbrun.py` are listed in `specs/038-db-runner-spike/results.md` (E5).
use crate::core::redact;
use futures_util::TryStreamExt;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tiberius::{error::Error as TdsError, AuthMethod, Client, ColumnData, Config, Row};
use tokio::net::TcpStream;
use tokio::time::timeout as within;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

const LOCK_TIMEOUT_MS: u64 = 5000; // as dbrun.py LOCK_TIMEOUT_MS
const MAX_TIMEOUT_SECS: f64 = 86_400.0;
const IDENTITY: &str =
    "SELECT @@VERSION, CAST(SERVERPROPERTY('MachineName') AS nvarchar(256)), DB_NAME()";
const LINKS: &str = "SELECT COUNT(*) FROM sys.servers WHERE is_linked = 1";

type Tds = Client<Compat<TcpStream>>;

/// The request's timeout in seconds, as in `mysql.rs`: absent is 30, a finite value of zero or more is
/// accepted (capped at one day), anything else is an invalid request and never reaches a panicking path.
fn timeout_secs(req: &Value) -> Result<f64, String> {
    match &req["timeout"] {
        Value::Null => Ok(30.0),
        v => match v.as_f64() {
            Some(t) if t.is_finite() && t >= 0.0 => Ok(t.min(MAX_TIMEOUT_SECS)),
            _ => Err("ValueError: invalid timeout".to_string()),
        },
    }
}

/// A server error as `MssqlError <number> (state, class): message`; other failures as their text.
fn describe(e: &TdsError) -> String {
    match e {
        TdsError::Server(t) => format!(
            "MssqlError {} (state {}, class {}): {}",
            t.code(),
            t.state(),
            t.class(),
            t.message()
        ),
        other => other.to_string(),
    }
}

fn text(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::to_owned)
        .or_else(|| v.as_u64().map(|n| n.to_string()))
}

async fn connect(req: &Value) -> Result<Tds, String> {
    let c = &req["conn"];
    let host = text(&c["host"]).unwrap_or_default();
    let port: u16 = text(&c["port"])
        .and_then(|p| p.parse().ok())
        .unwrap_or(1433);
    let mut config = Config::new();
    config.host(&host);
    config.port(port);
    config.database(
        text(&c["database"])
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| "master".into()),
    );
    config.authentication(AuthMethod::sql_server(
        text(&c["user"]).unwrap_or_default(),
        text(&c["password"]).unwrap_or_default(),
    ));
    config.application_name("ai-gent-dbrun");
    // TLS is rustls with the default `Required` encryption. The test image's certificate is self-signed,
    // so the certificate is not verified: acceptable for a loopback throwaway container only. A
    // production build needs a profile field (trust a CA file, or verify against the system roots).
    config.trust_cert();
    let login = Duration::from_secs(10); // pymssql login_timeout=10
    let tcp = within(login, TcpStream::connect((host.as_str(), port)))
        .await
        .map_err(|_| "connection timed out".to_string())?
        .map_err(|e| format!("{e}"))?;
    tcp.set_nodelay(true).map_err(|e| e.to_string())?;
    let mut client = within(login, Client::connect(config, tcp.compat_write()))
        .await
        .map_err(|_| "login timed out".to_string())?
        .map_err(|e| describe(&e))?;
    // As connect(): lock timeout and isolation level. No NOCOUNT: apply checks the row counts.
    batch(
        &mut client,
        &format!(
            "SET LOCK_TIMEOUT {LOCK_TIMEOUT_MS}; SET TRANSACTION ISOLATION LEVEL READ COMMITTED"
        ),
    )
    .await
    .map_err(|e| describe(&e))?;
    Ok(client)
}

async fn batch(client: &mut Tds, sql: &str) -> Result<Vec<Vec<Row>>, TdsError> {
    client.simple_query(sql).await?.into_results().await
}

/// Days since 0001-01-01 to (year, month, day), proleptic Gregorian (Howard Hinnant's `civil_from_days`).
fn civil(days_since_year_1: i64) -> (i64, u32, u32) {
    let z = days_since_year_1 - 719_162 + 719_468; // days from 0000-03-01
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn date_text(days_since_year_1: i64) -> String {
    let (y, m, d) = civil(days_since_year_1);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `HH:MM:SS` and, as Python's `isoformat()`, `.ffffff` only when the microseconds are not zero.
fn clock_text(seconds: u64, micros: u64) -> String {
    let base = format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    );
    if micros == 0 {
        base
    } else {
        format!("{base}.{micros:06}")
    }
}

/// The value of a TIME(n) as (seconds, microseconds): the increments are units of 10^-scale seconds and
/// anything finer than a microsecond is truncated, as a Python `datetime.time` cannot hold it.
fn time_parts(increments: u64, scale: u8) -> (u64, u64) {
    let scale = u32::from(scale.min(7));
    let micros_total = if scale <= 6 {
        increments * 10u64.pow(6 - scale)
    } else {
        increments / 10
    };
    (micros_total / 1_000_000, micros_total % 1_000_000)
}

/// The legacy DATETIME: days since 1900-01-01 and 1/300 s ticks. Milliseconds the way SQL Server itself
/// shows them (`(ticks * 10 + 1) / 3`: .000, .003, .007, .010), which is also what FreeTDS hands to pymssql.
fn datetime_text(days_since_1900: i64, ticks: u64) -> String {
    let seconds = ticks / 300;
    let ms = legacy_ms(ticks % 300);
    format!(
        "{}T{}",
        date_text(days_since_1900 + 693_595),
        clock_text(seconds, ms * 1000)
    )
}

fn legacy_ms(tick: u64) -> u64 {
    (tick * 10 + 1) / 3
}

/// TDS sends a DATETIMEOFFSET as UTC plus its offset; pymssql returns the wall time the value was stored
/// with (`14:59:58+05:30`), so the offset is added back, carrying into the day.
fn shift_minutes(days: i64, seconds: u64, offset: i16) -> (i64, u64) {
    let total = seconds as i64 + i64::from(offset) * 60;
    (
        days + total.div_euclid(86_400),
        total.rem_euclid(86_400) as u64,
    )
}

fn offset_text(minutes: i16) -> String {
    let sign = if minutes < 0 { '-' } else { '+' };
    let m = minutes.unsigned_abs();
    format!("{sign}{:02}:{:02}", m / 60, m % 60)
}

/// `str(decimal.Decimal)` for an exact scaled integer: the digits keep their scale (`1.50`, `0.05`, `-3`).
fn decimal_text(value: i128, scale: u8) -> String {
    let neg = value < 0;
    let digits = value.unsigned_abs().to_string();
    let scale = scale as usize;
    let body = if scale == 0 {
        digits
    } else {
        let padded = format!("{digits:0>width$}", width = scale + 1);
        let (int, frac) = padded.split_at(padded.len() - scale);
        format!("{int}.{frac}")
    };
    if neg {
        format!("-{body}")
    } else {
        body
    }
}

/// One cell as dbrun.py's `to_jsonable` renders what pymssql returns: ints, floats and bools as JSON,
/// Decimal and everything unknown as `str()`, dates and times as `isoformat()`, binary as `<N bytes>`.
fn to_json(data: &ColumnData<'_>, money: bool) -> Value {
    match data {
        ColumnData::U8(v) => v.map_or(Value::Null, |n| json!(n)),
        ColumnData::I16(v) => v.map_or(Value::Null, |n| json!(n)),
        ColumnData::I32(v) => v.map_or(Value::Null, |n| json!(n)),
        ColumnData::I64(v) => v.map_or(Value::Null, |n| json!(n)),
        ColumnData::F32(v) => v.map_or(Value::Null, |n| json!(f64::from(n))),
        // MONEY and SMALLMONEY arrive as a float; pymssql returns a Decimal with four decimals.
        ColumnData::F64(Some(n)) if money => json!(decimal_text((n * 10_000.0).round() as i128, 4)),
        ColumnData::F64(v) => v.map_or(Value::Null, |n| {
            if n.is_finite() {
                json!(n)
            } else {
                json!(n.to_string())
            }
        }),
        ColumnData::Bit(v) => v.map_or(Value::Null, |b| json!(b)),
        ColumnData::String(v) => v.as_ref().map_or(Value::Null, |s| json!(s.as_ref())),
        ColumnData::Guid(v) => v.map_or(Value::Null, |g| json!(g.to_string())),
        ColumnData::Binary(v) => v
            .as_ref()
            .map_or(Value::Null, |b| json!(format!("<{} bytes>", b.len()))),
        ColumnData::Numeric(v) => {
            v.map_or(Value::Null, |n| json!(decimal_text(n.value(), n.scale())))
        }
        ColumnData::Xml(v) => v.as_ref().map_or(Value::Null, |x| json!(x.to_string())),
        ColumnData::DateTime(v) => v.map_or(Value::Null, |d| {
            json!(datetime_text(
                i64::from(d.days()),
                u64::from(d.seconds_fragments())
            ))
        }),
        ColumnData::SmallDateTime(v) => v.map_or(Value::Null, |d| {
            json!(format!(
                "{}T{}",
                date_text(i64::from(d.days()) + 693_595),
                clock_text(u64::from(d.seconds_fragments()) * 60, 0)
            ))
        }),
        ColumnData::Time(v) => v.map_or(Value::Null, |t| {
            let (s, us) = time_parts(t.increments(), t.scale());
            json!(clock_text(s, us))
        }),
        ColumnData::Date(v) => v.map_or(Value::Null, |d| json!(date_text(i64::from(d.days())))),
        ColumnData::DateTime2(v) => v.map_or(Value::Null, |d| {
            let (s, us) = time_parts(d.time().increments(), d.time().scale());
            json!(format!(
                "{}T{}",
                date_text(i64::from(d.date().days())),
                clock_text(s, us)
            ))
        }),
        ColumnData::DateTimeOffset(v) => v.map_or(Value::Null, |d| {
            let dt = d.datetime2();
            let (s, us) = time_parts(dt.time().increments(), dt.time().scale());
            let (days, s) = shift_minutes(i64::from(dt.date().days()), s, d.offset());
            json!(format!(
                "{}T{}{}",
                date_text(days),
                clock_text(s, us),
                offset_text(d.offset())
            ))
        }),
    }
}

/// One result set: its column names and the converted rows (at most `cap + 1` are kept).
#[derive(Default)]
struct Set {
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
}

/// What a statement produced: its result sets and the server's own row count for it.
#[derive(Default)]
struct Outcome {
    sets: Vec<Set>,
    count: i64,
}

/// Runs one statement as a plain batch (what pymssql sends when there are no parameters), then asks the
/// same session for `ROWCOUNT_BIG()`, the server's row count for that statement. The client crate does not
/// expose the DONE counts of a streamed batch, and its RPC path drops the column names of an empty set.
async fn run_statement(
    client: &mut Tds,
    sql: &str,
    cap: usize,
    count: bool,
) -> Result<Outcome, TdsError> {
    let mut stream = client.simple_query(sql).await?;
    let mut out = Outcome::default();
    let mut money: Vec<bool> = vec![];
    while let Some(item) = stream.try_next().await? {
        if let Some(m) = item.as_metadata() {
            money = m
                .columns()
                .iter()
                .map(|c| {
                    matches!(
                        c.column_type(),
                        tiberius::ColumnType::Money | tiberius::ColumnType::Money4
                    )
                })
                .collect();
            out.sets.push(Set {
                columns: m.columns().iter().map(|c| c.name().to_owned()).collect(),
                rows: vec![],
            });
        } else if let Some(row) = item.into_row() {
            let Some(set) = out.sets.last_mut() else {
                continue;
            };
            // One past the cap is kept so read mode can tell it truncated; the rest are not converted.
            if set.rows.len() <= cap {
                set.rows.push(
                    row.cells()
                        .enumerate()
                        .map(|(i, (_, d))| to_json(d, money.get(i).copied().unwrap_or(false)))
                        .collect(),
                );
            }
        }
    }
    drop(stream);
    if count {
        let rows = batch(client, "SELECT ROWCOUNT_BIG()").await?;
        out.count = rows
            .first()
            .and_then(|r| r.first())
            .and_then(|r| r.get::<i64, _>(0))
            .unwrap_or(0);
    }
    Ok(out)
}

/// The result entry from what the server reported. `rowcount` is the server's count and is never derived
/// from the statement text, so comments, CTEs and OUTPUT clauses cannot change it. `max_rows` caps the
/// returned rows in both modes; only read mode sets `truncated`, as `dbrun.py`.
fn shape_entry(out: Outcome, cap: usize, read: bool, explain: bool) -> Value {
    // SHOWPLAN_TEXT: the first result set echoes the statement, the plan is the second.
    let pick = if explain {
        1.min(out.sets.len().saturating_sub(1))
    } else {
        0
    };
    let Some(set) = out.sets.into_iter().nth(pick) else {
        return json!({"rowcount": out.count});
    };
    let truncated = read && set.rows.len() > cap;
    let mut rows = set.rows;
    rows.truncate(cap);
    json!({"rowcount": out.count, "columns": set.columns, "rows": rows, "truncated": truncated})
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
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => return json!({"ok": false, "stage": "connect", "error": format!("runtime: {e}")}),
    };
    let mut result = rt.block_on(session(req, timeout));
    result["seconds"] = json!(started.elapsed().as_secs_f64());
    result
}

async fn session(req: &Value, timeout: f64) -> Value {
    let mut client = match connect(req).await {
        Ok(c) => c,
        Err(e) => return json!({"ok": false, "stage": "connect", "error": e}),
    };
    let mut result = json!({"ok": true, "results": []});
    let read = req["mode"] != "write";
    let explain = req["explain"] == true;
    // pymssql's `timeout` bounds each statement; zero means none there too.
    let limit = (timeout > 0.0).then(|| Duration::from_secs_f64(timeout));
    let cap = req["max_rows"].as_u64().unwrap_or(200) as usize;
    let outcome: Result<(), String> = async {
        let e = |e: TdsError| describe(&e);
        // pymssql (autocommit off) works inside a transaction it opens itself; so does this.
        batch(&mut client, "BEGIN TRANSACTION").await.map_err(e)?;
        let ident = batch(&mut client, IDENTITY).await.map_err(e)?;
        let first: Vec<Value> = ident
            .first()
            .and_then(|r| r.first())
            .map(|row| row.cells().map(|(_, d)| to_json(d, false)).collect())
            .unwrap_or_default();
        result["server"] = json!(first);
        let links = batch(&mut client, LINKS).await.map_err(e)?;
        result["links"] = links
            .first()
            .and_then(|r| r.first())
            .and_then(|row| row.get::<i32, _>(0))
            .map_or(json!(0), |n| json!(n));
        batch(&mut client, "ROLLBACK TRANSACTION")
            .await
            .map_err(e)?;
        if req["mode"] == "test" {
            return Ok(());
        }
        batch(&mut client, "BEGIN TRANSACTION").await.map_err(e)?;
        if explain {
            batch(&mut client, "SET SHOWPLAN_TEXT ON")
                .await
                .map_err(e)?;
        }
        for stmt in req["statements"].as_array().into_iter().flatten() {
            let sql = stmt.as_str().unwrap_or_default();
            let run = run_statement(&mut client, sql, cap, !explain);
            let out = match limit {
                Some(l) => within(l, run)
                    .await
                    .map_err(|_| format!("TimeoutError: statement exceeded {timeout} s"))?
                    .map_err(e)?,
                None => run.await.map_err(e)?,
            };
            result["results"]
                .as_array_mut()
                .unwrap()
                .push(shape_entry(out, cap, read, explain));
        }
        if explain {
            batch(&mut client, "SET SHOWPLAN_TEXT OFF")
                .await
                .map_err(e)?;
        }
        if read {
            batch(&mut client, "ROLLBACK TRANSACTION")
                .await
                .map_err(e)?;
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
            batch(&mut client, "COMMIT TRANSACTION").await.map_err(e)?;
            result["committed"] = json!(true);
        } else {
            batch(&mut client, "ROLLBACK TRANSACTION")
                .await
                .map_err(e)?;
            result["ok"] = json!(false);
            result["stage"] = json!("verify");
            result["error"] = json!(format!(
                "row counts differ from the plan: {}",
                mismatches.join(", ")
            ));
        }
        Ok(())
    }
    .await;
    if let Err(error) = outcome {
        // After a timeout the response is abandoned mid-read and the client would wait for the server to
        // finish the statement before it could send anything. So no rollback is sent: the connection is
        // dropped and the server rolls the open transaction back when it sees it closed.
        if !error.starts_with("TimeoutError:") {
            let _ = within(
                Duration::from_secs(5),
                batch(&mut client, "IF @@TRANCOUNT > 0 ROLLBACK TRANSACTION"),
            )
            .await;
        }
        result["ok"] = json!(false);
        result["stage"] = json!("execute");
        result["error"] = json!(error);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_is_validated_and_clamped() {
        assert_eq!(timeout_secs(&json!({})), Ok(30.0));
        assert_eq!(timeout_secs(&json!({"timeout": 7})), Ok(7.0));
        assert_eq!(timeout_secs(&json!({"timeout": 0})), Ok(0.0));
        assert_eq!(
            timeout_secs(&json!({"timeout": 1e300})),
            Ok(MAX_TIMEOUT_SECS)
        );
        for bad in [
            json!(-6),
            json!(-1e-9),
            json!("NaN"),
            json!("5"),
            json!(true),
        ] {
            assert_eq!(
                timeout_secs(&json!({"timeout": bad})),
                Err("ValueError: invalid timeout".to_string())
            );
        }
    }

    #[test]
    fn invalid_timeouts_fail_as_json_and_never_panic() {
        for bad in [json!(-6), json!("NaN"), json!(-0.5)] {
            let out = execute(
                &json!({"engine": "sqlserver", "mode": "test", "timeout": bad,
                "conn": {"host": "127.0.0.1", "port": 1, "user": "sa", "password": "p"}}),
            );
            assert_eq!(
                (out["ok"].clone(), out["stage"].clone()),
                (json!(false), json!("connect"))
            );
        }
    }

    #[test]
    fn a_refused_connection_is_a_redacted_connect_failure() {
        let req = json!({"engine": "sqlserver", "mode": "test", "timeout": 2,
            "conn": {"host": "localhost", "port": 1, "user": "app_ro", "password": "s3cr3t!", "database": "shop"}});
        let out = execute(&req);
        assert_eq!(
            (out["ok"].clone(), out["stage"].clone()),
            (json!(false), json!("connect"))
        );
        assert!(!out.to_string().contains("s3cr3t"));
    }

    #[test]
    fn rowcount_is_the_servers_count_whatever_the_statement_text() {
        let counted = |count: i64| Outcome {
            sets: vec![],
            count,
        };
        assert_eq!(
            shape_entry(counted(1), 200, false, false),
            json!({"rowcount": 1})
        );
        assert_eq!(
            shape_entry(counted(0), 200, false, false),
            json!({"rowcount": 0})
        );
    }

    #[test]
    fn max_rows_caps_returned_rows_in_both_modes() {
        let set = |kept: usize| Set {
            columns: vec!["id".into()],
            rows: (1..=kept).map(|i| vec![json!(i)]).collect(),
        };
        let out = |n: usize, kept| Outcome {
            sets: vec![set(kept)],
            count: n as i64,
        };
        // Write mode (UPDATE ... OUTPUT): capped, truncated stays false as in dbrun.py.
        let w = shape_entry(out(5, 3), 2, false, false);
        assert_eq!(w["rows"], json!([[1], [2]]));
        assert_eq!(
            (w["truncated"].clone(), w["rowcount"].clone()),
            (json!(false), json!(5))
        );
        // Read mode: capped and flagged.
        let r = shape_entry(out(5, 3), 2, true, false);
        assert_eq!(
            (r["rows"].clone(), r["truncated"].clone()),
            (json!([[1], [2]]), json!(true))
        );
        assert_eq!(
            shape_entry(out(2, 2), 2, true, false)["truncated"],
            json!(false)
        );
    }

    // Expected strings below were produced by Python (`datetime`, `decimal`) for the same values.
    #[test]
    fn dates_match_python_isoformat() {
        // date(1,1,1), date(1900,1,1), date(2026,11,7), date(9999,12,31), date(2000,2,29), date(1969,12,31)
        for (days, want) in [
            (0, "0001-01-01"),
            (693_595, "1900-01-01"),
            (739_926, "2026-11-07"),
            (3_652_058, "9999-12-31"),
            (730_178, "2000-02-29"),
            (719_161, "1969-12-31"),
        ] {
            assert_eq!(date_text(days), want, "{days}");
        }
    }

    #[test]
    fn clock_matches_python_isoformat() {
        assert_eq!(clock_text(0, 0), "00:00:00");
        assert_eq!(clock_text(3723, 0), "01:02:03");
        assert_eq!(clock_text(3723, 120_000), "01:02:03.120000");
        assert_eq!(clock_text(86_399, 999_999), "23:59:59.999999");
        assert_eq!(clock_text(5, 1), "00:00:05.000001");
    }

    #[test]
    fn time_scales_truncate_to_microseconds() {
        assert_eq!(time_parts(12, 1), (1, 200_000));
        assert_eq!(time_parts(1_234_567, 7), (0, 123_456)); // 0.1234567 s
        assert_eq!(time_parts(863_999_999_999, 7), (86_399, 999_999));
        assert_eq!(time_parts(5, 0), (5, 0));
    }

    #[test]
    fn legacy_datetime_milliseconds() {
        // Ticks 0..4 of 1/300 s render .000 .003 .007 .010 .013 in SQL Server and pymssql.
        let ms: Vec<u64> = (0..5).map(legacy_ms).collect();
        assert_eq!(ms, vec![0, 3, 7, 10, 13]);
        assert_eq!(legacy_ms(299), 997);
        assert_eq!(
            datetime_text(46_015, 3 * 300 + 1),
            "2025-12-26T00:00:03.003000"
        );
    }

    #[test]
    fn decimals_match_python_str() {
        // str(Decimal('1.50')), Decimal('0.05'), Decimal('-3'), Decimal('-0.5'), Decimal('12345.6789')
        assert_eq!(decimal_text(150, 2), "1.50");
        assert_eq!(decimal_text(5, 2), "0.05");
        assert_eq!(decimal_text(-3, 0), "-3");
        assert_eq!(decimal_text(-5, 1), "-0.5");
        assert_eq!(decimal_text(123_456_789, 4), "12345.6789");
        assert_eq!(decimal_text(0, 2), "0.00");
    }

    #[test]
    fn datetimeoffset_is_shown_as_the_stored_wall_time() {
        // pymssql: CAST('2026-11-07 14:59:58 +05:30' AS datetimeoffset(0)) -> 2026-11-07T14:59:58+05:30,
        // while TDS carries 09:29:58 (UTC); -03:00 carries 17:59:58.
        assert_eq!(
            shift_minutes(739_926, 9 * 3600 + 29 * 60 + 58, 330),
            (739_926, 14 * 3600 + 59 * 60 + 58)
        );
        assert_eq!(
            shift_minutes(739_926, 17 * 3600 + 59 * 60 + 58, -180),
            (739_926, 14 * 3600 + 59 * 60 + 58)
        );
        // The wall time can fall on the neighbouring day.
        assert_eq!(
            shift_minutes(739_926, 22 * 3600, 330),
            (739_927, 3 * 3600 + 30 * 60)
        );
        assert_eq!(shift_minutes(739_926, 60, -180), (739_925, 21 * 3600 + 60));
    }

    #[test]
    fn offsets_have_python_shape() {
        assert_eq!(offset_text(0), "+00:00");
        assert_eq!(offset_text(-180), "-03:00");
        assert_eq!(offset_text(330), "+05:30");
    }
}
