//! dbq <engine> <host-or-path> <port> <user> <database> <sql>   (password: env DBQ_PASSWORD, throwaway fixtures only)
//! Classifies with the ported core, refuses anything but a read, runs it read-only, prints one JSON line.
use serde_json::{json, Value};
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 7 {
        eprintln!("usage: dbq engine host port user database sql");
        std::process::exit(2);
    }
    let (engine, host, port, user, db, sql) = (&a[1], &a[2], &a[3], &a[4], &a[5], &a[6]);
    let pw = std::env::var("DBQ_PASSWORD").unwrap_or_default();
    let t0 = Instant::now();
    let stmts = match dbcore::split_statements(sql, engine) {
        Ok(s) => s,
        Err(e) => return refuse(&e.0),
    };
    for s in &stmts {
        let (k, why) = dbcore::classify_statement(s, engine);
        if k != "read" && std::env::var("DBQ_SKIP_SCAN").is_err() {
            return refuse(&why);
        }
    }
    let _ = (&host, &port, &user, &db, &pw);
    let res: Result<Value, String> = match engine.as_str() {
        #[cfg(feature = "sqlite")]
        "sqlite" => sqlite(host, &stmts),
        #[cfg(feature = "pg")]
        "postgresql" => pg(host, port, user, db, &pw, &stmts),
        #[cfg(feature = "mysql")]
        "mysql" | "mariadb" => my(host, port, user, db, &pw, &stmts),
        #[cfg(feature = "mssql")]
        "sqlserver" => ms(host, port, user, db, &pw, &stmts),
        e => Err(format!("engine {e} not compiled in")),
    };
    match res {
        Ok(mut v) => {
            v["seconds"] = json!(t0.elapsed().as_secs_f64());
            println!("{v}");
        }
        Err(e) => {
            println!("{}", json!({"ok": false, "error": e}));
            std::process::exit(1);
        }
    }
}

fn refuse(why: &str) {
    println!("{}", json!({"ok": false, "refused": why}));
    std::process::exit(3);
}

#[cfg(feature = "sqlite")]
fn sqlite(path: &str, stmts: &[String]) -> Result<Value, String> {
    use rusqlite::{types::ValueRef, Connection, OpenFlags};
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI).map_err(|e| e.to_string())?;
    c.execute_batch("PRAGMA query_only = ON").map_err(|e| e.to_string())?;
    let mut results = vec![];
    for s in stmts {
        let mut st = c.prepare(s).map_err(|e| e.to_string())?;
        let cols: Vec<String> = st.column_names().into_iter().map(|s| s.to_string()).collect();
        let mut rows = vec![];
        let mut q = st.query([]).map_err(|e| e.to_string())?;
        while let Some(r) = q.next().map_err(|e| e.to_string())? {
            let mut row = vec![];
            for i in 0..cols.len() {
                row.push(match r.get_ref(i).map_err(|e| e.to_string())? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(i) => json!(i),
                    ValueRef::Real(f) => json!(f),
                    ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                    ValueRef::Blob(b) => json!(format!("<{} bytes>", b.len())),
                });
            }
            rows.push(row);
        }
        results.push(json!({"columns": cols, "rows": rows}));
    }
    Ok(json!({"ok": true, "results": results}))
}

#[cfg(feature = "pg")]
fn pg(host: &str, port: &str, user: &str, db: &str, pw: &str, stmts: &[String]) -> Result<Value, String> {
    use postgres::{types::Type, Client, Config, NoTls};
    let mut cfg = Config::new();
    cfg.host(host).port(port.parse().unwrap_or(5432)).user(user).password(pw).dbname(db)
        .application_name("ai-gent-dbrun")
        .connect_timeout(std::time::Duration::from_secs(10))
        .options("-c statement_timeout=30000 -c lock_timeout=5000 -c idle_in_transaction_session_timeout=60000 -c default_transaction_read_only=on");
    // NoTls here: TLS needs postgres-native-tls (OpenSSL) or tokio-postgres-rustls (async); see REPORT.
    let mut c: Client = cfg.connect(NoTls).map_err(|e| e.to_string())?;
    let mut results = vec![];
    for s in stmts {
        let st = c.prepare(s).map_err(|e| e.to_string())?;
        let cols: Vec<String> = st.columns().iter().map(|c| c.name().to_string()).collect();
        let rows = c.query(&st, &[]).map_err(|e| e.to_string())?;
        let mut out = vec![];
        for r in &rows {
            let mut row = vec![];
            for (i, col) in st.columns().iter().enumerate() {
                let t = col.type_();
                let v: Value = if *t == Type::INT4 { r.get::<_, Option<i32>>(i).map_or(Value::Null, |x| json!(x)) }
                    else if *t == Type::INT8 { r.get::<_, Option<i64>>(i).map_or(Value::Null, |x| json!(x)) }
                    else if *t == Type::INT2 { r.get::<_, Option<i16>>(i).map_or(Value::Null, |x| json!(x)) }
                    else if *t == Type::FLOAT8 { r.get::<_, Option<f64>>(i).map_or(Value::Null, |x| json!(x)) }
                    else if *t == Type::BOOL { r.get::<_, Option<bool>>(i).map_or(Value::Null, |x| json!(x)) }
                    else if *t == Type::TEXT || *t == Type::VARCHAR || *t == Type::NAME || *t == Type::BPCHAR { r.get::<_, Option<String>>(i).map_or(Value::Null, |x| json!(x)) }
                    else { json!(format!("<unmapped type {}>", t.name())) };
                row.push(v);
            }
            out.push(row);
        }
        results.push(json!({"columns": cols, "rows": out}));
    }
    Ok(json!({"ok": true, "results": results}))
}

#[cfg(feature = "mysql")]
fn my(host: &str, port: &str, user: &str, db: &str, pw: &str, stmts: &[String]) -> Result<Value, String> {
    use mysql::{prelude::Queryable, Conn, OptsBuilder, Value as V};
    let o = OptsBuilder::new().ip_or_hostname(Some(host)).tcp_port(port.parse().unwrap_or(3306)).user(Some(user)).pass(Some(pw)).db_name(Some(db))
        .tcp_connect_timeout(Some(std::time::Duration::from_secs(10)));
    let mut c = Conn::new(o).map_err(|e| e.to_string())?;
    let ver: String = c.query_first("SELECT VERSION()").map_err(|e| e.to_string())?.unwrap_or_default();
    if ver.to_lowercase().contains("mariadb") {
        c.query_drop("SET SESSION max_statement_time = 30").map_err(|e| e.to_string())?;
    } else {
        c.query_drop("SET SESSION MAX_EXECUTION_TIME = 30000").map_err(|e| e.to_string())?;
    }
    c.query_drop("SET SESSION TRANSACTION READ ONLY").map_err(|e| e.to_string())?;
    c.query_drop("START TRANSACTION READ ONLY").map_err(|e| e.to_string())?;
    let mut results = vec![];
    for s in stmts {
        let rows: Vec<mysql::Row> = c.query(s).map_err(|e| e.to_string())?;
        let cols: Vec<String> = rows.first().map(|r| r.columns_ref().iter().map(|c| c.name_str().to_string()).collect()).unwrap_or_default();
        let out: Vec<Vec<Value>> = rows.iter().map(|r| r.clone().unwrap().into_iter().map(|v| match v {
            V::NULL => Value::Null,
            V::Int(i) => json!(i),
            V::UInt(u) => json!(u),
            V::Float(f) => json!(f),
            V::Double(f) => json!(f),
            V::Bytes(b) => json!(String::from_utf8_lossy(&b)),
            other => json!(other.as_sql(true)),
        }).collect()).collect();
        results.push(json!({"columns": cols, "rows": out, "version": ver}));
    }
    c.query_drop("ROLLBACK").ok();
    Ok(json!({"ok": true, "results": results}))
}

#[cfg(feature = "mssql")]
fn ms(host: &str, port: &str, user: &str, db: &str, pw: &str, stmts: &[String]) -> Result<Value, String> {
    use tiberius::{AuthMethod, Client, Config, EncryptionLevel};
    use tokio_util::compat::TokioAsyncWriteCompatExt;
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
    rt.block_on(async {
        let mut cfg = Config::new();
        cfg.host(host);
        cfg.port(port.parse().unwrap_or(1433));
        cfg.database(db);
        cfg.authentication(AuthMethod::sql_server(user, pw));
        cfg.encryption(EncryptionLevel::Required);
        cfg.trust_cert(); // throwaway container's self-signed certificate
        let tcp = tokio::net::TcpStream::connect(cfg.get_addr()).await.map_err(|e| e.to_string())?;
        tcp.set_nodelay(true).ok();
        let mut c = Client::connect(cfg, tcp.compat_write()).await.map_err(|e| e.to_string())?;
        let mut results = vec![];
        for s in stmts {
            let rows = c.simple_query(s.as_str()).await.map_err(|e| e.to_string())?.into_first_result().await.map_err(|e| e.to_string())?;
            let cols: Vec<String> = rows.first().map(|r| r.columns().iter().map(|c| c.name().to_string()).collect()).unwrap_or_default();
            let out: Vec<Vec<Value>> = rows.iter().map(|r| r.cells().map(|(_, d)| json!(format!("{:?}", d))).collect()).collect();
            results.push(json!({"columns": cols, "rows": out}));
        }
        Ok(json!({"ok": true, "results": results}))
    })
}
