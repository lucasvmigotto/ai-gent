//! The connector contract cases (`rust/fixtures/db-exec/`) against the SQLite connector. A later
//! connector adds one test like `sqlite_passes_the_contract`, with its own seeding.
#![cfg(feature = "sqlite")]
use ai_gent_dbrun::connector::contract;
use ai_gent_dbrun::connector::sqlite::SqliteConnector;
use ai_gent_dbrun::connector::{ConnFacts, Connector};
use rusqlite::Connection;

#[test]
fn connector_sqlite_passes_the_contract() {
    let mut dirs = Vec::new(); // keeps each case's database alive until the end
    let connector: &dyn Connector = &SqliteConnector;
    let ran = contract::run_exec_cases(
        connector,
        "sqlite",
        contract::Profile::Port,
        |case, setup| {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(format!("{}.db", case.file));
            Connection::open(&path)
                .unwrap()
                .execute_batch(&setup.join(";\n"))
                .unwrap();
            let facts = ConnFacts {
                path: Some(path.to_string_lossy().into_owned()),
                ..ConnFacts::default()
            };
            dirs.push(dir);
            facts
        },
    );
    // 5 of the 8 SqliteEndToEndTest cases act at the connector (01, 02, 04, 05, 06), 6 more come from the
    // reviews (09-14) and 8 from the T002 contract freeze and its review round (15-22); 03 is policy, 07 and 08 are pending.
    assert_eq!(ran, 19, "exec cases run");
}

#[test]
fn connector_policy_cases_hold_without_a_connector() {
    assert_eq!(contract::run_policy_cases(), 5);
}

#[test]
fn connector_plan_layer_cases_are_listed_not_hidden() {
    let names: Vec<String> = contract::pending_cases()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert_eq!(names, ["tampered-plan-refused", "script-never-executes"]);
}
