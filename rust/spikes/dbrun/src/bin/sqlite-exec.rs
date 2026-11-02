//! Test-only JSON bridge for the native SQLite experiment; no process connector.
use std::io::{self, Read};
fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let request = serde_json::from_str(&input).unwrap();
    println!("{}", dbcore::drivers::sqlite::execute(&request));
}
