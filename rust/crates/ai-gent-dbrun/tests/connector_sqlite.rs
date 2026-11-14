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
    // reviews (09-14) and 8 from the T002 contract freeze and its review round (15-22); 03 is policy, 07 and 08 are
    // journeys of the plan layer (see the test below).
    assert_eq!(ran, 20, "exec cases run");
}

#[test]
fn connector_policy_cases_hold_without_a_connector() {
    assert_eq!(contract::run_policy_cases(), 5);
}

#[test]
fn connector_no_case_is_pending_and_every_journey_names_a_test_that_exists() {
    assert!(
        contract::pending_cases().is_empty(),
        "T020: none of the 8 SqliteEndToEndTest journeys is pending"
    );
    let journeys = contract::journey_cases();
    let names: Vec<&str> = journeys.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["tampered-plan-refused", "script-never-executes"]);
    for (name, tests) in &journeys {
        assert!(!tests.is_empty(), "{name}: names no test");
        for t in tests {
            let (file, function) = t.split_once("::").expect("tests/<file>.rs::<function>");
            let source = std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(file),
            )
            .unwrap_or_else(|e| panic!("{name}: {file}: {e}"));
            let at = source
                .find(&format!("fn {function}()"))
                .unwrap_or_else(|| panic!("{name}: {t} does not exist"));
            assert!(
                source[..at].trim_end().ends_with("#[test]"),
                "{name}: {t} is not a #[test]"
            );
        }
    }
}
