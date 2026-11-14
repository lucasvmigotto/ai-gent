//! T009: query, plan, apply, restore and script over the traits, against the SQLite connector and the real
//! SQLite backup. The first eight tests are the eight `SqliteEndToEndTest` cases; the rest pin the
//! output of the original (`rust/fixtures/db-core/golden-writes.json`) and the fail-closed rules the
//! port adds. Every test that refuses something names the input, asserts the refusal and the exit
//! code, and checks that the fake password, user and host appear in no output and no log line.
#![cfg(feature = "sqlite")]
#![allow(
    clippy::many_single_char_names,
    clippy::unused_self,
    clippy::items_after_statements
)]
use ai_gent_dbrun::connector::sqlite::SqliteConnector;
use ai_gent_dbrun::connector::{ConnFacts, Connector, ExecResult, Mode, Request, Stage};
use ai_gent_dbrun::core::backup::{Backup, EngineBackup, Exec, ExecOutput, ExecSpec};
use ai_gent_dbrun::core::log::{Clock, CommandLog, Stamp};
use ai_gent_dbrun::core::plan::{load_plan, plan_path};
use ai_gent_dbrun::core::redact::Redactor;
use ai_gent_dbrun::core::run::{
    Done, EXIT_CONNECT, EXIT_ERROR, EXIT_OK, EXIT_REFUSED, EXIT_USAGE, Fail, QueryArgs, Runner,
    Target, csv_line, parse_expect, py_json, table,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

const PW: &str = "pw-FAKE-s3cr3t";
const USER: &str = "fakeuser9";
const HOST: &str = "db.fake-host.example";

struct FixedClock {
    n: Cell<u32>,
}
impl Clock for FixedClock {
    fn now(&self) -> Stamp {
        Stamp {
            date: "2026-01-01".into(),
            iso: "2026-01-01T12:00:00+00:00".into(),
        }
    }
    fn token(&self, len: usize) -> String {
        self.n.set(self.n.get() + 1);
        format!("{:0len$x}", self.n.get() + 0xab11)
    }
}

struct NoExec;
impl Exec for NoExec {
    fn run(&self, _: &ExecSpec<'_>) -> Result<ExecOutput, String> {
        Err("no container in this test".into())
    }
}

/// The real SQLite connector, recording each request's mode and optionally looking at the log when a
/// write arrives.
struct Spy {
    modes: RefCell<Vec<Mode>>,
    log_dir: PathBuf,
    log_at_write: RefCell<Option<String>>,
}
impl Spy {
    fn new(log_dir: &Path) -> Self {
        Self {
            modes: RefCell::default(),
            log_dir: log_dir.into(),
            log_at_write: RefCell::default(),
        }
    }
    fn writes(&self) -> usize {
        self.modes
            .borrow()
            .iter()
            .filter(|m| **m == Mode::Write)
            .count()
    }
}
impl Connector for Spy {
    fn engines(&self) -> &'static [&'static str] {
        SqliteConnector.engines()
    }
    fn execute(&self, req: &Request) -> ExecResult {
        self.modes.borrow_mut().push(req.mode);
        if req.mode == Mode::Write {
            let text = std::fs::read_dir(&self.log_dir)
                .map(|d| {
                    d.flatten()
                        .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
                        .collect::<String>()
                })
                .unwrap_or_default();
            *self.log_at_write.borrow_mut() = Some(text);
        }
        SqliteConnector.execute(req)
    }
}

/// Answers every non-test request with a fixed result.
struct Canned(ExecResult, i64);
impl Connector for Canned {
    fn engines(&self) -> &'static [&'static str] {
        SqliteConnector.engines()
    }
    fn execute(&self, req: &Request) -> ExecResult {
        if req.mode == Mode::Test {
            let mut r = ExecResult::success();
            r.links = Some(self.1);
            r.server = Some(vec![json!("canned")]);
            return r;
        }
        self.0.clone()
    }
}

struct Fx {
    tmp: tempfile::TempDir,
    clock: FixedClock,
    log: CommandLog,
}

impl Fx {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let log = CommandLog::new(
            tmp.path().join("state/log"),
            "repo",
            Redactor::from_parts(Some(PW), Some(HOST), Some(USER)),
        );
        Self {
            tmp,
            clock: FixedClock { n: Cell::new(0) },
            log,
        }
    }
    fn plans(&self) -> PathBuf {
        let d = self.tmp.path().join("state/plans");
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn backups(&self) -> PathBuf {
        let d = self.tmp.path().join("state/backups");
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn log_dir(&self) -> PathBuf {
        self.tmp.path().join("state/log")
    }
    fn log_text(&self) -> String {
        std::fs::read_dir(self.log_dir())
            .map(|d| {
                d.flatten()
                    .map(|e| std::fs::read_to_string(e.path()).unwrap())
                    .collect()
            })
            .unwrap_or_default()
    }
    fn db(&self, name: &str) -> PathBuf {
        let path = self.tmp.path().join(name);
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE customer (id INTEGER PRIMARY KEY, nome TEXT, email TEXT, status TEXT);
                 INSERT INTO customer VALUES (1,'Ana Lima','ana@example.com','active'),
                   (2,'Bob Souza','bob@example.com','active'),(3,'Cy Dias','cy@example.com','blocked');",
            )
            .unwrap();
        path
    }
    fn target(&self, name: &str, path: &Path, local: bool) -> Target {
        Target {
            name: name.into(),
            engine: "sqlite".into(),
            conn: ConnFacts {
                path: Some(path.to_string_lossy().into()),
                user: Some(USER.into()),
                host: Some(HOST.into()),
                password: Some(PW.into()),
                ..ConnFacts::default()
            },
            container: None,
            class: if local { "local" } else { "production" }.into(),
            writable: local,
            via: "file".into(),
            evidence: vec![if local {
                "file inside this repository (app.db)".into()
            } else {
                "file outside this repository — not a local copy of this project".into()
            }],
        }
    }
    fn runner<'a>(&'a self, c: &'a dyn Connector, b: &'a dyn Backup) -> Runner<'a> {
        Runner {
            connector: c,
            backup: b,
            log: &self.log,
            clock: &self.clock,
            plans_dir: self.plans(),
            backups_dir: self.backups(),
        }
    }
}

fn count(path: &Path, filter: &str) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row(
            &format!("SELECT COUNT(*) FROM customer {filter}"),
            [],
            |r| r.get(0),
        )
        .unwrap()
}

fn plan_id(out: &str) -> String {
    out.lines()
        .find_map(|l| l.strip_prefix("Plan "))
        .and_then(|l| l.split_whitespace().next())
        .unwrap()
        .to_string()
}

fn no_secret(texts: &[&str]) {
    for t in texts {
        for s in [PW, USER, HOST] {
            assert!(!t.contains(s), "{s} leaked in: {t}");
        }
    }
}

fn refused(r: Result<Done, Fail>, needle: &str) -> Fail {
    match r {
        Err(f @ Fail::Refused(_)) => {
            assert_eq!(f.code(), EXIT_REFUSED);
            let line = f.report(&Redactor::from_parts(Some(PW), Some(HOST), Some(USER)));
            assert!(
                line.starts_with("REFUSED: ") && line.contains(needle),
                "{line}"
            );
            no_secret(&[&line]);
            f
        }
        other => panic!("expected a refusal containing {needle:?}, got {other:?}"),
    }
}

// ---- the eight SqliteEndToEndTest cases --------------------------------------------------------------

#[test]
fn e2e_1_read_masks_and_logs() {
    let fx = Fx::new();
    let db = fx.db("app.db");
    let (spy, bk) = (Spy::new(&fx.log_dir()), EngineBackup::new(&NoExec));
    let r = fx.runner(&spy, &bk);
    let t = fx.target("LOCALDB", &db, true);
    let d = r
        .query(
            &t,
            &QueryArgs::new(
                "SELECT id, nome, email FROM customer ORDER BY id",
                "test read",
            ),
        )
        .unwrap();
    assert_eq!(d.code, EXIT_OK, "{}", d.err);
    assert!(
        d.out.contains("A*** L***") && !d.out.contains("ana@example.com"),
        "{}",
        d.out
    );
    let mut a = QueryArgs::new(
        "SELECT email FROM customer WHERE id = 1",
        "user asked to see it",
    );
    a.reveal = true;
    assert!(r.query(&t, &a).unwrap().out.contains("ana@example.com"));
    let log = fx.log_text();
    assert!(
        log.contains("- reason: test read") && log.contains("reveal: yes"),
        "{log}"
    );
    assert!(!log.contains("ana@example.com"), "results are never logged");
    no_secret(&[&log, &d.out]);
}

#[test]
fn e2e_2_row_cap() {
    let fx = Fx::new();
    let db = fx.db("app.db");
    let (spy, bk) = (Spy::new(&fx.log_dir()), EngineBackup::new(&NoExec));
    let mut a = QueryArgs::new("SELECT id FROM customer", "t");
    a.max_rows = 2;
    let d = fx
        .runner(&spy, &bk)
        .query(&fx.target("L", &db, true), &a)
        .unwrap();
    assert!(
        d.out.contains("truncated at --max-rows 2") && d.out.contains("(2 row(s); truncated"),
        "{}",
        d.out
    );
}

#[test]
fn e2e_3_remote_refusals_and_allow_full_count() {
    let fx = Fx::new();
    let db = fx.db("prod.db");
    let (spy, bk) = (Spy::new(&fx.log_dir()), EngineBackup::new(&NoExec));
    let r = fx.runner(&spy, &bk);
    let t = fx.target("PRODDB", &db, false);
    for (sql, needle) in [
        ("DELETE FROM customer WHERE id = 1", "is not read-only"),
        ("SELECT * FROM customer WHERE id = 1", "name the columns"),
        ("SELECT count(*) FROM customer", "unfiltered COUNT"),
        ("SELECT 1; DROP TABLE customer", "is not read-only"),
    ] {
        refused(r.query(&t, &QueryArgs::new(sql, "t")), needle);
    }
    let f = refused(
        r.plan(&t, "UPDATE customer SET status = 'x' WHERE id = 1", "t", ""),
        "dbrun script",
    );
    assert_eq!(f.code(), 3);
    assert_eq!(spy.modes.borrow().len(), 0, "nothing reached the connector");
    assert_eq!(count(&db, "WHERE status = 'active'"), 2);
    // --allow-full-count lifts only the COUNT rule.
    let mut a = QueryArgs::new(
        "SELECT count(*) FROM customer",
        "the user asked for an exact count",
    );
    a.allow_full_count = true;
    let d = r.query(&t, &a).unwrap();
    assert_eq!(d.code, EXIT_OK, "{}", d.err);
    assert!(d.out.contains('3'), "{}", d.out);
    let mut a = QueryArgs::new("SELECT * FROM customer", "t");
    a.allow_full_count = true;
    refused(r.query(&t, &a), "name the columns");
    no_secret(&[&fx.log_text()]);
}

#[test]
fn e2e_4_remote_read_is_read_only_even_past_the_classifier() {
    let fx = Fx::new();
    let db = fx.db("prod.db");
    let req = Request::new(
        "sqlite",
        fx.target("P", &db, false).conn,
        Mode::Read,
        vec!["UPDATE customer SET status = 'x'".into()],
    );
    let res = SqliteConnector.run(&req);
    assert!(!res.ok, "{res:?}");
    assert_eq!(count(&db, "WHERE status = 'x'"), 0);
}

#[test]
fn e2e_5_plan_apply_restore_log_first_and_a_second_apply_refused() {
    let fx = Fx::new();
    let db = fx.db("app.db");
    let (spy, bk) = (Spy::new(&fx.log_dir()), EngineBackup::new(&NoExec));
    let r = fx.runner(&spy, &bk);
    let t = fx.target("LOCALDB", &db, true);
    let d = r
        .plan(
            &t,
            "UPDATE customer SET status = 'blocked' WHERE status = 'active'",
            "experiment",
            "2",
        )
        .unwrap();
    let id = plan_id(&d.out);
    refused(r.apply(&t, &id, false, 120.0), "needs --confirmed");
    assert_eq!(spy.modes.borrow().len(), 0);
    let d = r.apply(&t, &id, true, 120.0).unwrap();
    assert_eq!(d.code, EXIT_OK, "{}", d.out);
    assert!(
        d.out.contains("COMMITTED plan") && d.out.contains("row counts [2]"),
        "{}",
        d.out
    );
    assert_eq!(count(&db, "WHERE status = 'active'"), 0);
    // The entry was on disk, status started, when the write request arrived.
    let at_write = spy.log_at_write.borrow().clone().expect("a write ran");
    assert!(
        at_write.contains("· apply · LOCALDB ·") && at_write.contains("- status: started"),
        "{at_write}"
    );
    assert!(
        !at_write.contains("COMMITTED"),
        "the result is written after the run"
    );
    assert!(fx.log_text().contains("COMMITTED · row counts [2]"));
    // A second apply of the same plan is refused and writes nothing.
    let writes = spy.writes();
    refused(r.apply(&t, &id, true, 120.0), "already applied");
    assert_eq!(spy.writes(), writes);
    let d = r.restore(&t, &id, true).unwrap();
    assert!(d.out.starts_with("RESTORED LOCALDB from "), "{}", d.out);
    assert_eq!(count(&db, "WHERE status = 'active'"), 2);
    refused(r.restore(&t, &id, false), "needs --confirmed");
    no_secret(&[&fx.log_text(), &d.out]);
}

#[test]
fn e2e_6_apply_rolls_back_on_count_mismatch_and_may_be_tried_again() {
    let fx = Fx::new();
    let db = fx.db("app.db");
    let (spy, bk) = (Spy::new(&fx.log_dir()), EngineBackup::new(&NoExec));
    let r = fx.runner(&spy, &bk);
    let t = fx.target("LOCALDB", &db, true);
    let id = plan_id(
        &r.plan(&t, "DELETE FROM customer WHERE status = 'active'", "t", "5")
            .unwrap()
            .out,
    );
    let d = r.apply(&t, &id, true, 120.0).unwrap();
    assert_eq!(d.code, EXIT_ERROR);
    assert!(d.out.contains("ROLLED BACK (verify)"), "{}", d.out);
    assert_eq!(count(&db, ""), 3);
    // Verified rollback: nothing changed, the claim is given back.
    assert_eq!(r.apply(&t, &id, true, 120.0).unwrap().code, EXIT_ERROR);
    no_secret(&[&fx.log_text(), &d.out]);
}

#[test]
fn e2e_7_a_tampered_plan_is_refused_and_nothing_runs() {
    let fx = Fx::new();
    let db = fx.db("app.db");
    let (spy, bk) = (Spy::new(&fx.log_dir()), EngineBackup::new(&NoExec));
    let r = fx.runner(&spy, &bk);
    let t = fx.target("LOCALDB", &db, true);
    let plan = |sql: &str| plan_id(&r.plan(&t, sql, "t", "1").unwrap().out);
    let edit = |id: &str, f: &dyn Fn(&mut Value)| {
        let p = plan_path(&fx.plans(), id).unwrap();
        let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        f(&mut v);
        std::fs::write(p, v.to_string()).unwrap();
    };
    // 1. the statements (the Python check)
    let id = plan("DELETE FROM customer WHERE id = 3");
    edit(&id, &|v| v["statements"] = json!(["DELETE FROM customer"]));
    refused(
        r.apply(&t, &id, true, 120.0),
        "modified after it was recorded",
    );
    // 2. the expected counts (Python did not notice): no count check left to skip
    let id2 = {
        // a distinct id: the fixed clock hands out a new token each time
        plan("DELETE FROM customer WHERE id = 2")
    };
    edit(&id2, &|v| v["expect"] = json!([null]));
    refused(
        r.apply(&t, &id2, true, 120.0),
        "modified after it was recorded",
    );
    // 3. the engine, the profile, the DDL list
    let id3 = plan("DELETE FROM customer WHERE id = 1");
    edit(&id3, &|v| v["engine"] = json!("postgresql"));
    refused(
        r.apply(&t, &id3, true, 120.0),
        "modified after it was recorded",
    );
    // 4. the seal removed (a plan the Python runner wrote has none)
    let id4 = plan("DELETE FROM customer WHERE id = 1");
    edit(&id4, &|v| {
        v.as_object_mut().unwrap().remove("seal");
    });
    refused(r.apply(&t, &id4, true, 120.0), "no seal");
    // 5. another profile's plan
    let id5 = plan("DELETE FROM customer WHERE id = 1");
    let other = fx.target("OTHER", &db, true);
    refused(
        r.apply(&other, &id5, true, 120.0),
        "was made for LOCALDB, not OTHER",
    );
    // 6. ids that are not plan ids, a missing plan, a symlinked plan file is read bounded
    for bad in ["../x", "x", "20260101-120000-ffff", ""] {
        let e = r.apply(&t, bad, true, 120.0).unwrap_err();
        assert_eq!(e.code(), EXIT_ERROR, "{bad}");
    }
    assert_eq!(spy.modes.borrow().len(), 0, "no request, not even a probe");
    assert_eq!(count(&db, ""), 3);
    assert!(
        !plan_path(&fx.plans(), &id)
            .unwrap()
            .with_extension("applied")
            .exists()
    );
    no_secret(&[&fx.log_text()]);
}

#[test]
fn e2e_8_script_never_executes() {
    let fx = Fx::new();
    let db = fx.db("prod.db");
    let (spy, bk) = (Spy::new(&fx.log_dir()), EngineBackup::new(&NoExec));
    let r = fx.runner(&spy, &bk);
    let t = fx.target("PRODDB", &db, false);
    let out = fx.tmp.path().join("docs/change.sql");
    let d = r
        .script(
            &t,
            "UPDATE customer SET status = 'x' WHERE id = 1",
            "fix a record",
            &out,
        )
        .unwrap();
    assert!(d.out.contains("(not executed)"), "{}", d.out);
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("NOT executed") && text.contains("COMMIT;"));
    assert!(text.contains("BEGIN;\nUPDATE customer SET status = 'x' WHERE id = 1;"));
    for (sql, to, needle) in [
        (
            "DELETE FROM customer",
            "docs/bad.sql",
            "a DELETE without WHERE",
        ),
        (
            "UPDATE customer SET a = 1 /* WHERE */",
            "docs/bad2.sql",
            "an UPDATE without WHERE",
        ),
        (
            "SELECT 'a\\'; DELETE FROM t; SELECT '",
            "docs/bad3.sql",
            "split differently",
        ),
    ] {
        let bad = fx.tmp.path().join(to);
        refused(r.script(&t, sql, "t", &bad), needle);
        assert!(!bad.exists());
    }
    let again = r
        .script(&t, "UPDATE customer SET a = 1 WHERE id = 1", "t", &out)
        .unwrap_err();
    assert!(
        matches!(&again, Fail::Failed(m, 1) if m.contains("exists")),
        "{again:?}"
    );
    assert_eq!(
        spy.modes.borrow().len(),
        0,
        "zero requests: the connector was never called"
    );
    assert_eq!(count(&db, "WHERE status = 'active'"), 2);
    assert!(
        fx.log_text().contains("written to")
            && fx.log_text().contains("script written, not executed")
    );
}

// ---- parity with the original ------------------------------------------------------------------------

fn golden() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/db-core/golden-writes.json"
        ))
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn parity_plan_record_text_and_log_entry_match_the_original() {
    let g = golden();
    let mut ran = 0;
    for case in g["plan"].as_array().unwrap() {
        let fx = Fx::new();
        let engine = case["engine"].as_str().unwrap();
        let (spy, bk) = (Spy::new(&fx.log_dir()), EngineBackup::new(&NoExec));
        let r = fx.runner(&spy, &bk);
        let mut t = fx.target("P", Path::new("/unused"), true);
        t.engine = engine.into();
        t.conn.database = Some("shop".into());
        t.conn.password = None;
        t.conn.user = None;
        t.conn.host = None;
        t.evidence = vec!["ev one".into(), "ev two".into()];
        let got = r.plan(
            &t,
            case["sql"].as_str().unwrap(),
            "why",
            case["expect"].as_str().unwrap(),
        );
        if let Some(why) = case.get("refused") {
            match got {
                Err(Fail::Refused(m)) => assert_eq!(m, why.as_str().unwrap()),
                other => panic!("{case}: {other:?}"),
            }
            continue;
        }
        let d = got.unwrap();
        let id = plan_id(&d.out);
        assert_eq!(
            d.out.replace(&id, "<ID>"),
            case["stdout"].as_str().unwrap(),
            "{case}"
        );
        let plan = load_plan(&fx.plans(), "P", &id).unwrap();
        let mut rec = serde_json::to_value(&plan).unwrap();
        for k in ["id", "created", "seal"] {
            rec.as_object_mut().unwrap().remove(k);
        }
        assert_eq!(rec, case["record"], "{case}");
        let eid = regex_free_entry_id(&fx.log_text());
        let log = fx
            .log_text()
            .replace(&id, "<ID>")
            .replace(&eid, "<EID>")
            .replace(
                &fx.plans().to_string_lossy().into_owned(),
                "<STATE>/state/plans",
            )
            .replace("2026-01-01T12:00:00+00:00", "<TS>");
        assert_eq!(log, case["log"].as_str().unwrap(), "{case}");
        ran += 1;
    }
    assert_eq!(ran, 7);
}

/// The 8-hex id after the last ` · ` of the entry's heading.
fn regex_free_entry_id(log: &str) -> String {
    log.lines()
        .next()
        .unwrap()
        .rsplit(" · ")
        .next()
        .unwrap()
        .to_string()
}

#[test]
fn parity_change_script_text_and_refusals_match_the_original() {
    let g = golden();
    let cases = g["script"].as_array().unwrap();
    assert_eq!(cases.len(), 17);
    for c in cases {
        let (engine, sql, verdict, text) = (
            c[0].as_str().unwrap(),
            c[1].as_str().unwrap(),
            c[2].as_str().unwrap(),
            c[3].as_str().unwrap(),
        );
        let fx = Fx::new();
        let (spy, bk) = (Spy::new(&fx.log_dir()), EngineBackup::new(&NoExec));
        let mut t = fx.target("P", Path::new("/unused"), false);
        t.engine = engine.into();
        t.evidence = vec!["e".into()];
        let out = fx.tmp.path().join("sub/change.sql");
        let got = fx.runner(&spy, &bk).script(&t, sql, "why", &out);
        match (verdict, got) {
            ("ok", Ok(_)) => {
                let norm = |s: &str| {
                    s.lines()
                        .map(|l| {
                            if l.starts_with("-- Prepared by ai-gent dbrun on ") {
                                "-- Prepared"
                            } else {
                                l
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                assert_eq!(
                    norm(&std::fs::read_to_string(&out).unwrap()),
                    norm(text),
                    "{engine} {sql}"
                );
            }
            ("refused", Err(Fail::Refused(m))) | ("failed", Err(Fail::Failed(m, _))) => {
                assert_eq!(m, text);
            }
            (v, got) => panic!("{engine} {sql:?}: python {v}, rust {got:?}"),
        }
        assert_eq!(spy.modes.borrow().len(), 0);
    }
}

#[test]
fn parity_query_output_formats_match_the_original() {
    for c in golden()["format"].as_array().unwrap() {
        let cols: Vec<String> = c["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect();
        let rows: Vec<Vec<Value>> = c["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_array().unwrap().clone())
            .collect();
        assert_eq!(table(&cols, &rows, 120), c["table"].as_str().unwrap());
        let mut csv = csv_line(cols.iter().map(String::as_str));
        for r in &rows {
            let cells: Vec<String> = r
                .iter()
                .map(|v| {
                    if v.is_null() {
                        String::new()
                    } else {
                        ai_gent_dbrun::core::run::py_str(v)
                    }
                })
                .collect();
            csv.push_str(&csv_line(cells.iter().map(String::as_str)));
        }
        assert_eq!(csv, c["csv"].as_str().unwrap());
        let masked: serde_json::Map<String, Value> = c["masked"].as_object().unwrap().clone();
        // Python keeps insertion order; the comparison here is on the sorted object (see differences.md).
        let want: Value = serde_json::from_str(c["json"].as_str().unwrap()).unwrap();
        let got = format!(
            "{{\"columns\": {}, \"rows\": {}, \"truncated\": false, \"masked\": {}}}",
            py_json(&json!(cols)),
            py_json(&json!(rows)),
            py_json(&Value::Object(masked))
        );
        assert_eq!(serde_json::from_str::<Value>(&got).unwrap(), want);
        assert!(got.starts_with("{\"columns\": [") && got.contains("\"truncated\": false"));
        assert_eq!(py_json(&json!(["é", "a\"b"])), "[\"\\u00e9\", \"a\\\"b\"]");
    }
}

#[test]
fn parity_expect_parsing_and_exit_codes() {
    assert_eq!(
        parse_expect("1,,x,-2").unwrap(),
        vec![Some(1), None, None, Some(-2)]
    );
    assert_eq!(parse_expect(" 2 , null").unwrap(), vec![Some(2), None]);
    assert_eq!(parse_expect("").unwrap(), Vec::<Option<i64>>::new());
    // Python crashes on these two; the port refuses.
    for bad in ["--5", "99999999999999999999"] {
        assert!(matches!(parse_expect(bad), Err(Fail::Refused(_))), "{bad}");
    }
    assert_eq!(
        (EXIT_OK, EXIT_ERROR, EXIT_USAGE, EXIT_REFUSED, EXIT_CONNECT),
        (0, 1, 2, 3, 4)
    );
    assert_eq!(Fail::Failed("x".into(), EXIT_CONNECT).code(), 4);
}

// ---- fail closed -----------------------------------------------------------------------------------

#[test]
fn a_log_that_cannot_be_written_stops_apply_before_anything_runs() {
    let fx = Fx::new();
    let db = fx.db("app.db");
    let (spy, bk) = (Spy::new(&fx.log_dir()), EngineBackup::new(&NoExec));
    let t = fx.target("LOCALDB", &db, true);
    let id = plan_id(
        &fx.runner(&spy, &bk)
            .plan(&t, "DELETE FROM customer WHERE id = 3", "t", "1")
            .unwrap()
            .out,
    );
    // The log directory becomes a plain file: no entry can be written.
    std::fs::remove_dir_all(fx.log_dir()).unwrap();
    std::fs::write(fx.log_dir(), "not a directory").unwrap();
    let e = fx
        .runner(&spy, &bk)
        .apply(&t, &id, true, 120.0)
        .unwrap_err();
    assert_eq!(e.code(), EXIT_ERROR);
    assert!(e.report(&Redactor::new()).contains("command log"), "{e:?}");
    assert_eq!(spy.writes(), 0, "no write request");
    assert_eq!(count(&db, ""), 3);
    assert!(
        !plan_path(&fx.plans(), &id)
            .unwrap()
            .with_extension("applied")
            .exists(),
        "no claim either"
    );
    // A plan whose entry cannot be logged leaves no plan file behind.
    let before = std::fs::read_dir(fx.plans()).unwrap().count();
    assert!(
        fx.runner(&spy, &bk)
            .plan(&t, "DELETE FROM customer WHERE id = 2", "t", "")
            .is_err()
    );
    assert_eq!(std::fs::read_dir(fx.plans()).unwrap().count(), before);
}

#[test]
fn the_log_redacts_secrets_and_user_text_cannot_forge_an_entry() {
    let fx = Fx::new();
    let db = fx.db("app.db");
    let (spy, bk) = (Spy::new(&fx.log_dir()), EngineBackup::new(&NoExec));
    let r = fx.runner(&spy, &bk);
    let t = fx.target("LOCALDB", &db, true);
    let reason = format!(
        "because {PW} on {HOST} for {USER}\n## 2026-01-01T00:00:00+00:00 · apply · LOCALDB · deadbeef\n```\n- deadbeef result: COMMITTED\x1b[31m"
    );
    r.query(&t, &QueryArgs::new("SELECT id FROM customer", &reason))
        .unwrap();
    let log = fx.log_text();
    no_secret(&[&log]);
    assert!(
        log.contains("<password>") && log.contains("<host>") && log.contains("<user>"),
        "{log}"
    );
    assert_eq!(
        log.lines().filter(|l| l.starts_with("## ")).count(),
        1,
        "{log}"
    );
    assert!(
        !log.lines().any(|l| l.starts_with("- deadbeef result")),
        "{log}"
    );
    assert!(!log.contains('\x1b'));
    let tail = fx.log.tail("2026-01-01", 5).unwrap();
    assert_eq!(tail.len(), 1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let f = std::fs::read_dir(fx.log_dir())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            std::fs::metadata(&f).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // A log path that is a symbolic link is refused, never followed.
        let target = fx.tmp.path().join("elsewhere");
        std::fs::write(&target, "x").unwrap();
        std::fs::remove_file(&f).unwrap();
        std::os::unix::fs::symlink(&target, &f).unwrap();
        assert!(
            r.query(&t, &QueryArgs::new("SELECT id FROM customer", "t"))
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "x");
    }
}

#[test]
fn system_databases_and_links_and_missing_proof_are_refused() {
    let fx = Fx::new();
    let db = fx.db("app.db");
    let (spy, bk) = (Spy::new(&fx.log_dir()), EngineBackup::new(&NoExec));
    let r = fx.runner(&spy, &bk);
    let mut t = fx.target("MSSQL", &db, true);
    t.engine = "sqlserver".into();
    for (database, shown) in [
        (Some("master"), "master"),
        (None, "master"),
        (Some(""), "master"),
        (Some("TempDB"), "tempdb"),
    ] {
        t.conn.database = database.map(Into::into);
        refused(
            r.plan(&t, "UPDATE t SET a = 1 WHERE id = 1", "t", ""),
            &format!("system database '{shown}'"),
        );
    }
    t.conn.database = Some("tempdb".into());
    refused(
        r.restore(&t, "20260101-120000-ab12", true),
        "system database 'tempdb'",
    );
    // A database with links to other databases is not local, even when the classification said so.
    let local = fx.target("LOCALDB", &db, true);
    let id = plan_id(
        &r.plan(&local, "DELETE FROM customer WHERE id = 3", "t", "1")
            .unwrap()
            .out,
    );
    let linked = Canned(ExecResult::success(), 2);
    let r2 = fx.runner(&linked, &bk);
    refused(
        r2.apply(&local, &id, true, 1.0),
        "link(s) to other databases",
    );
    assert_eq!(count(&db, ""), 3);
    // Not proven local: apply, restore and plan all refuse.
    let remote = fx.target("LOCALDB", &db, false);
    for f in [
        r.apply(&remote, &id, true, 1.0),
        r.restore(&remote, &id, true),
        r.plan(&remote, "DELETE FROM customer WHERE id = 1", "t", ""),
    ] {
        refused(f, "needs a proven local database");
    }
    assert_eq!(spy.writes(), 0);
}

#[test]
fn a_result_that_is_not_a_confirmed_commit_is_a_failure_and_the_plan_stays_claimed() {
    let fx = Fx::new();
    let db = fx.db("app.db");
    let bk = EngineBackup::new(&NoExec);
    let t = fx.target("LOCALDB", &db, true);
    let spy = Spy::new(&fx.log_dir());
    let id = plan_id(
        &fx.runner(&spy, &bk)
            .plan(&t, "DELETE FROM customer WHERE id = 3", "t", "1")
            .unwrap()
            .out,
    );
    // ok:true without committed, and a driver failure at execute: both may have changed the database.
    let mut ok_no_commit = ExecResult::success();
    ok_no_commit.results = Some(vec![]);
    let canned = Canned(ok_no_commit, 0);
    let d = fx.runner(&canned, &bk).apply(&t, &id, true, 5.0).unwrap();
    assert_eq!(d.code, EXIT_ERROR);
    assert!(
        d.out.contains("ROLLED BACK (execute)") && d.out.contains("marked as applied"),
        "{}",
        d.out
    );
    refused(
        fx.runner(&canned, &bk).apply(&t, &id, true, 5.0),
        "already applied",
    );
    let id2 = plan_id(
        &fx.runner(&spy, &bk)
            .plan(&t, "DELETE FROM customer WHERE id = 2", "t", "1")
            .unwrap()
            .out,
    );
    let crashed = Canned(
        ExecResult::failure(Stage::Execute, format!("driver said {PW} at {HOST}")),
        0,
    );
    let d = fx.runner(&crashed, &bk).apply(&t, &id2, true, 5.0).unwrap();
    assert_eq!(d.code, EXIT_ERROR);
    no_secret(&[&d.out, &fx.log_text()]);
    refused(
        fx.runner(&crashed, &bk).apply(&t, &id2, true, 5.0),
        "already applied",
    );
}

#[test]
fn exit_codes_connect_failures_are_4_and_execute_failures_are_1() {
    let fx = Fx::new();
    let db = fx.db("app.db");
    let bk = EngineBackup::new(&NoExec);
    let t = fx.target("LOCALDB", &db, true);
    let spy = Spy::new(&fx.log_dir());
    let id = plan_id(
        &fx.runner(&spy, &bk)
            .plan(&t, "DELETE FROM customer WHERE id = 3", "t", "1")
            .unwrap()
            .out,
    );
    // A connect failure before the write: exit 4, nothing claimed, secrets redacted.
    struct Down;
    impl Connector for Down {
        fn engines(&self) -> &'static [&'static str] {
            &["sqlite"]
        }
        fn execute(&self, _: &Request) -> ExecResult {
            ExecResult::failure(
                Stage::Connect,
                format!("cannot reach {HOST} as {USER} with {PW}"),
            )
        }
    }
    let e = fx.runner(&Down, &bk).apply(&t, &id, true, 5.0).unwrap_err();
    assert_eq!(e.code(), EXIT_CONNECT);
    let line = e.report(&Redactor::from_parts(Some(PW), Some(HOST), Some(USER)));
    assert!(line.starts_with("error: can't connect: "), "{line}");
    no_secret(&[&line]);
    assert!(
        !plan_path(&fx.plans(), &id)
            .unwrap()
            .with_extension("applied")
            .exists()
    );
    let d = fx
        .runner(&Down, &bk)
        .query(&t, &QueryArgs::new("SELECT id FROM customer", "t"))
        .unwrap();
    assert_eq!(d.code, EXIT_CONNECT);
    no_secret(&[&d.err, &fx.log_text()]);
    let broke = Canned(ExecResult::failure(Stage::Execute, "boom"), 0);
    let d = fx
        .runner(&broke, &bk)
        .query(&t, &QueryArgs::new("SELECT id FROM customer", "t"))
        .unwrap();
    assert_eq!(d.code, EXIT_ERROR);
    assert!(d.err.starts_with("FAILED (execute): boom"));
}

#[test]
fn a_failed_backup_runs_nothing_and_the_plan_can_be_applied_again() {
    struct BadBackup(Cell<bool>);
    impl Backup for BadBackup {
        fn take(
            &self,
            _: &ai_gent_dbrun::core::backup::BackupTarget,
            _: &str,
            _: &Path,
        ) -> Result<String, ai_gent_dbrun::core::backup::BackupError> {
            if self.0.get() {
                Err(ai_gent_dbrun::core::backup::BackupError::Failed(format!(
                    "disk full while using {PW}"
                )))
            } else {
                Ok("fine".into())
            }
        }
        fn restore(
            &self,
            _: &ai_gent_dbrun::core::backup::BackupTarget,
            _: &str,
            _: &str,
            _: &Path,
        ) -> Result<(), ai_gent_dbrun::core::backup::BackupError> {
            Ok(())
        }
    }
    let fx = Fx::new();
    let db = fx.db("app.db");
    let (spy, bk) = (Spy::new(&fx.log_dir()), BadBackup(Cell::new(true)));
    let r = fx.runner(&spy, &bk);
    let t = fx.target("LOCALDB", &db, true);
    let id = plan_id(
        &r.plan(&t, "DELETE FROM customer WHERE id = 3", "t", "1")
            .unwrap()
            .out,
    );
    let e = r.apply(&t, &id, true, 5.0).unwrap_err();
    assert_eq!(e.code(), EXIT_ERROR);
    assert_eq!(spy.writes(), 0);
    assert_eq!(count(&db, ""), 3);
    no_secret(&[
        &e.report(&Redactor::from_parts(Some(PW), Some(HOST), Some(USER))),
        &fx.log_text(),
    ]);
    bk.0.set(false);
    assert_eq!(r.apply(&t, &id, true, 5.0).unwrap().code, EXIT_OK);
    assert_eq!(count(&db, ""), 2);
}

#[test]
fn restore_refuses_a_forged_backup_location_and_a_plan_without_a_backup() {
    let fx = Fx::new();
    let db = fx.db("app.db");
    let (spy, bk) = (Spy::new(&fx.log_dir()), EngineBackup::new(&NoExec));
    let r = fx.runner(&spy, &bk);
    let t = fx.target("LOCALDB", &db, true);
    let id = plan_id(
        &r.plan(&t, "DELETE FROM customer WHERE id = 3", "t", "1")
            .unwrap()
            .out,
    );
    let e = r.restore(&t, &id, true).unwrap_err();
    assert!(
        matches!(&e, Fail::Failed(m, 1) if m.contains("has no backup to restore")),
        "{e:?}"
    );
    r.apply(&t, &id, true, 5.0).unwrap();
    // The recorded location is edited to another file: the backup layer refuses it.
    let p = plan_path(&fx.plans(), &id).unwrap();
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    v["backup_location"] = json!("/etc/passwd");
    std::fs::write(&p, v.to_string()).unwrap();
    refused(r.restore(&t, &id, true), "recorded backup location");
    assert_eq!(count(&db, ""), 2);
    no_secret(&[&fx.log_text()]);
}
