//! Journeys 07 (plan tampering) and 08 (a change script that never executes) of
//! `SqliteEndToEndTest`, through the SQLite connector and temp dirs. No Python at test time.
#![cfg(feature = "sqlite")]
use ai_gent_dbrun::connector::sqlite::SqliteConnector;
use ai_gent_dbrun::connector::{ConnFacts, Connector, ExecResult, Request};
use ai_gent_dbrun::core::plan::{
    ApplyOptions, NewPlan, PlanError, ScriptSpec, apply_plan, create_plan, load_plan, write_script,
};
use rusqlite::Connection;
use std::cell::Cell;
use std::path::Path;

/// Counts the requests that reach the real connector.
struct Counting {
    calls: Cell<usize>,
    /// Timeout and row cap of the last request.
    seen: Cell<(f64, usize)>,
}
impl Connector for Counting {
    fn engines(&self) -> &'static [&'static str] {
        SqliteConnector.engines()
    }
    fn execute(&self, req: &Request) -> ExecResult {
        self.calls.set(self.calls.get() + 1);
        self.seen.set((req.timeout, req.max_rows));
        SqliteConnector.execute(req)
    }
}

const ID: &str = "20260101-120000-ab12";

fn seeded(dir: &Path) -> ConnFacts {
    let path = dir.join("app.db");
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TABLE customer (id INTEGER PRIMARY KEY, status TEXT);
             INSERT INTO customer VALUES (1,'active'),(2,'active'),(3,'active');",
        )
        .unwrap();
    ConnFacts {
        path: Some(path.to_string_lossy().into_owned()),
        ..ConnFacts::default()
    }
}

fn count(conn: &ConnFacts) -> i64 {
    Connection::open(conn.path.as_deref().unwrap())
        .unwrap()
        .query_row("SELECT COUNT(*) FROM customer", [], |r| r.get(0))
        .unwrap()
}

fn new_plan(sql: &str) -> NewPlan<'_> {
    NewPlan {
        id: ID,
        profile: "LOCALDB",
        engine: "sqlite",
        reason: "t",
        created: "2026-01-01T12:00:00+00:00",
        sql,
        expect: vec![Some(1)],
        backup_supported: true,
    }
}

#[test]
fn journey_07_a_tampered_plan_is_refused_and_nothing_runs() {
    let dir = tempfile::tempdir().unwrap();
    let plans = tempfile::tempdir().unwrap();
    let conn = seeded(dir.path());
    let plan = create_plan(plans.path(), &new_plan("DELETE FROM customer WHERE id = 3")).unwrap();
    assert_eq!(plan.statements, ["DELETE FROM customer WHERE id = 3"]);

    // Edit the file after planning, as the Python test does.
    let file = plans.path().join(format!("{ID}.json"));
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    json["statements"] = serde_json::json!(["DELETE FROM customer"]);
    std::fs::write(&file, json.to_string()).unwrap();

    let connector = Counting {
        calls: Cell::new(0),
        seen: Cell::new((0.0, 0)),
    };
    let backups = Cell::new(0);
    let err = apply_plan(
        &connector,
        plans.path(),
        &conn,
        "LOCALDB",
        ID,
        ApplyOptions::default(),
        |_| {
            backups.set(backups.get() + 1);
            Ok("never".into())
        },
    )
    .unwrap_err();
    assert!(
        matches!(&err, PlanError::Refused(m) if m.contains("modified after it was recorded")),
        "{err}"
    );
    assert_eq!(
        (connector.calls.get(), backups.get()),
        (0, 0),
        "nothing ran"
    );
    assert_eq!(count(&conn), 3);
}

#[test]
fn an_untouched_plan_applies_with_a_backup_and_binds_to_its_profile() {
    let dir = tempfile::tempdir().unwrap();
    let plans = tempfile::tempdir().unwrap();
    let conn = seeded(dir.path());
    create_plan(plans.path(), &new_plan("DELETE FROM customer WHERE id = 3")).unwrap();
    assert!(matches!(
        load_plan(plans.path(), "OTHER", ID),
        Err(PlanError::Refused(m)) if m.contains("was made for LOCALDB, not OTHER")
    ));
    assert!(matches!(
        load_plan(plans.path(), "LOCALDB", "../x"),
        Err(PlanError::Failed(_))
    ));
    assert!(matches!(
        load_plan(plans.path(), "LOCALDB", "20260101-120000-ffff"),
        Err(PlanError::Failed(_))
    ));
    // A plan id is never overwritten.
    assert!(create_plan(plans.path(), &new_plan("DELETE FROM customer WHERE id = 1")).is_err());

    let connector = Counting {
        calls: Cell::new(0),
        seen: Cell::new((0.0, 0)),
    };
    let opts = ApplyOptions {
        timeout: 300.0,
        ..ApplyOptions::default()
    };
    let (plan, result) = apply_plan(&connector, plans.path(), &conn, "LOCALDB", ID, opts, |p| {
        Ok(format!("/backups/{}.sqlite", p.id))
    })
    .unwrap();
    // The caller's timeout reaches the request (it is not the connector's 30 s default).
    assert_eq!(connector.seen.get(), (300.0, 1000));
    assert!(result.ok, "{result:?}");
    assert_eq!(result.committed, Some(true));
    assert_eq!(connector.calls.get(), 1);
    assert_eq!(count(&conn), 2);
    let stored = load_plan(plans.path(), "LOCALDB", ID).unwrap();
    assert_eq!(
        stored.backup_location.as_deref(),
        Some(format!("/backups/{ID}.sqlite").as_str())
    );
    assert_eq!(stored, plan);
}

#[test]
fn a_failed_backup_runs_nothing_and_ddl_without_backup_support_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let plans = tempfile::tempdir().unwrap();
    let conn = seeded(dir.path());
    create_plan(plans.path(), &new_plan("DELETE FROM customer WHERE id = 3")).unwrap();
    let connector = Counting {
        calls: Cell::new(0),
        seen: Cell::new((0.0, 0)),
    };
    let err = apply_plan(
        &connector,
        plans.path(),
        &conn,
        "LOCALDB",
        ID,
        ApplyOptions::default(),
        |_| Err("disk full".into()),
    )
    .unwrap_err();
    assert!(matches!(err, PlanError::Failed(m) if m.contains("disk full")));
    assert_eq!((connector.calls.get(), count(&conn)), (0, 3));

    let other = NewPlan {
        id: "20260101-120001-ab12",
        backup_supported: false,
        ..new_plan("DROP TABLE customer")
    };
    assert!(matches!(
        create_plan(plans.path(), &other),
        Err(PlanError::Refused(_))
    ));
}

#[test]
fn journey_08_a_change_script_is_written_and_never_executed() {
    let dir = tempfile::tempdir().unwrap();
    let conn = seeded(dir.path());
    // The only way to reach the database is a connector; write_script takes none, and the counter
    // proves no request was made while the file was written.
    let connector = Counting {
        calls: Cell::new(0),
        seen: Cell::new((0.0, 0)),
    };
    let out = dir.path().join("docs").join("change.sql");
    let spec = |sql| ScriptSpec {
        profile: "PRODDB",
        engine: "sqlite",
        class: "production",
        reason: "fix a record",
        now: "2026-01-01T12:00:00+00:00",
        sql,
    };
    write_script(&spec("UPDATE customer SET status = 'x' WHERE id = 1"), &out).unwrap();
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("NOT executed"));
    assert!(
        text.contains("BEGIN;\nUPDATE customer SET status = 'x' WHERE id = 1;"),
        "{text}"
    );
    assert!(text.contains("COMMIT;"));
    assert!(text.contains("-- Statements: 1 (1 write, 0 read)"));

    let bad = dir.path().join("docs").join("bad.sql");
    let err = write_script(&spec("DELETE FROM customer"), &bad).unwrap_err();
    assert!(
        matches!(&err, PlanError::Refused(m) if m.contains("a DELETE without WHERE")),
        "{err}"
    );
    assert!(!bad.exists());
    assert!(matches!(
        write_script(&spec("UPDATE customer SET status = 'x' WHERE id = 1"), &out),
        Err(PlanError::Failed(m)) if m.contains("exists")
    ));

    assert_eq!(connector.calls.get(), 0, "zero statements executed");
    let status: String = Connection::open(conn.path.as_deref().unwrap())
        .unwrap()
        .query_row("SELECT status FROM customer WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "active");
}

#[test]
fn apply_defaults_are_those_of_dbrun_py_apply() {
    let d = ApplyOptions::default();
    assert!(d.backup_supported);
    assert_eq!((d.timeout, d.max_rows), (120.0, 1000));
}
