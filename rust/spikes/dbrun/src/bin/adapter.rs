//! Execution-only adapter: one Python JSON request on stdin, one result line.
//! PostgreSQL runs natively when built with the `pg` feature, MySQL and MariaDB with the `mysql`
//! feature; every other engine (or a build without the feature) goes through the Python `_exec`
//! bridge.
use serde_json::{json, Value};
use std::io;

fn execute(request: &Value) -> Value {
    #[cfg(feature = "pg")]
    if request["engine"].as_str() == Some("postgresql") {
        return dbcore::drivers::pg::execute(request);
    }
    #[cfg(feature = "mysql")]
    if matches!(request["engine"].as_str(), Some("mysql" | "mariadb")) {
        return dbcore::drivers::mysql::execute(request);
    }
    dbcore::drivers::process::execute(request)
}

fn main() {
    let result = match serde_json::from_reader::<_, Value>(io::stdin().lock()) {
        Ok(request) => execute(&request),
        Err(_) => {
            json!({"ok": false, "stage": "connect", "error": "invalid execution request JSON"})
        }
    };
    println!("{result}");
}
