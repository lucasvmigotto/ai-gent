//! Execution-only adapter: one Python JSON request on stdin, one result line.
//! With `--features pg`, PostgreSQL runs natively; every other engine goes through the Python `_exec`.
use serde_json::{json, Value};
use std::io;

fn main() {
    let result = match serde_json::from_reader::<_, Value>(io::stdin().lock()) {
        Ok(request) => match request["engine"].as_str() {
            #[cfg(feature = "pg")]
            Some("postgresql") => dbcore::drivers::pg::execute(&request),
            _ => dbcore::drivers::process::execute(&request),
        },
        Err(_) => {
            json!({"ok": false, "stage": "connect", "error": "invalid execution request JSON"})
        }
    };
    println!("{result}");
}
