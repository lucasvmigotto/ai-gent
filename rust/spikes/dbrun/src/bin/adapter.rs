//! Execution-only adapter: one Python JSON request on stdin, one result line.
use serde_json::{json, Value};
use std::io;

fn main() {
    let result = match serde_json::from_reader::<_, Value>(io::stdin().lock()) {
        Ok(request) => dbcore::drivers::process::execute(&request),
        Err(_) => {
            json!({"ok": false, "stage": "connect", "error": "invalid execution request JSON"})
        }
    };
    println!("{result}");
}
