#![cfg(feature = "sqlite")]

#[test]
fn sqlite_original_eight_end_to_end_cases() {
    let status = std::process::Command::new("python3")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/test_sqlite.py"))
        .env("DBRUN_SQLITE_EXEC", env!("CARGO_BIN_EXE_sqlite-exec"))
        .status()
        .expect("run Python's eight SQLite journeys with the native executor");
    assert!(status.success());
}
