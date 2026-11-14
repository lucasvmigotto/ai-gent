//! T020/T022 (library half): a SQLite profile from its file through the local proof to plan, apply and restore,
//! with the real connector, the real backup API and the real command log; and the refusals of a file
//! outside the repository, a symbolic link, and a file swapped for a link after it was proven. The CLI half
//! of T022 (`ai-gent db plan|apply|restore`) needs the `ai-gent db` command of T011.
//!
//! Every refusal names its input, checks the exit code and that no password, user or host appears in the
//! output, the log or the plan files, and that the database outside the repository is unchanged.
#![cfg(all(feature = "sqlite", unix))]
use ai_gent_core::env::Env;
use ai_gent_dbrun::connector::sqlite::SqliteConnector;
use ai_gent_dbrun::core::backup::{EngineBackup, Exec, ExecOutput, ExecSpec};
use ai_gent_dbrun::core::log::{CommandLog, SystemClock};
use ai_gent_dbrun::core::redact::Redactor;
use ai_gent_dbrun::core::run::{Done, EXIT_OK, EXIT_REFUSED, Fail, Runner, Target};
use ai_gent_dbrun::profiles::{Ctx, classify, user_profiles};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

const PW: &str = "pw-J0urney-S3cret";
const USER: &str = "journey_user_x";
const HOST: &str = "journey-host.example";

struct NoExec;
impl Exec for NoExec {
    fn run(&self, _: &ExecSpec<'_>) -> Result<ExecOutput, String> {
        Err("SQLite never uses the container exec".into())
    }
}

struct Sandbox {
    _tmp: tempfile::TempDir,
    repo: PathBuf,
    outside: PathBuf,
    config: PathBuf,
    state: PathBuf,
}

fn seed(path: &Path) {
    Connection::open(path)
        .unwrap()
        .execute_batch(
            "CREATE TABLE customer (id INTEGER PRIMARY KEY, status TEXT);
             INSERT INTO customer VALUES (1, 'active'), (2, 'active'), (3, 'blocked');",
        )
        .unwrap();
}

impl Sandbox {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().canonicalize().unwrap();
        let (repo, outside, config, state) = (
            base.join("repo"),
            base.join("elsewhere"),
            base.join("config"),
            base.join("state"),
        );
        for d in [&repo, &outside, &config, &state] {
            std::fs::create_dir_all(d).unwrap();
        }
        seed(&repo.join("app.db"));
        seed(&outside.join("prod.db"));
        Self {
            _tmp: tmp,
            repo,
            outside,
            config,
            state,
        }
    }

    /// One `.env` profile `NAME` for `path`, declared local, with a user and host to redact.
    fn ctx(&self, name: &str, path: &Path) -> Ctx {
        let file = self.config.join("connections.env");
        std::fs::write(
            &file,
            format!(
                "{name}_ENGINE=sqlite\n{name}_CLASS=local\n{name}_PATH={}\n{name}_USER={USER}\n{name}_HOST={HOST}\n",
                path.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(
            &file,
            <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o600),
        )
        .unwrap();
        // No PATH entry: nothing here may run a program (no pass, no container engine).
        let env = Env::from_pairs([
            ("AI_GENT_DB_CONFIG", self.config.to_str().unwrap()),
            ("AI_GENT_DB_CONNECTIONS", file.to_str().unwrap()),
            ("PATH", self.state.to_str().unwrap()),
            ("HOME", self.state.to_str().unwrap()),
        ]);
        Ctx::new(env, self.repo.clone(), None)
    }

    /// The target a command gets: loaded, classified now, with the password the runner would read.
    fn target(&self, name: &str, path: &Path) -> Target {
        let ctx = self.ctx(name, path);
        let loaded = user_profiles(&ctx).unwrap();
        let p = &loaded.profiles[name];
        let c = classify(&ctx, p).unwrap();
        let mut t = Target::from_profile(p, &c);
        t.conn.password = Some(PW.into());
        t
    }

    fn log(&self) -> CommandLog {
        CommandLog::new(self.state.join("log"), "repo", redactor())
    }

    fn dirs(&self) -> (PathBuf, PathBuf) {
        let (plans, backups) = (self.state.join("plans"), self.state.join("backups"));
        std::fs::create_dir_all(&plans).unwrap();
        std::fs::create_dir_all(&backups).unwrap();
        (plans, backups)
    }

    /// Everything the run left on disk: log, plans, backups' names.
    fn written(&self) -> String {
        let mut out = String::new();
        for d in ["log", "plans"] {
            if let Ok(entries) = std::fs::read_dir(self.state.join(d)) {
                for e in entries.flatten() {
                    out.push_str(&std::fs::read_to_string(e.path()).unwrap_or_default());
                }
            }
        }
        out
    }
}

fn redactor() -> Redactor {
    Redactor::from_parts(Some(PW), Some(HOST), Some(USER))
}

fn no_secret(texts: &[&str]) {
    for t in texts {
        for s in [PW, USER, HOST] {
            assert!(!t.contains(s), "{s} leaked in: {t}");
        }
    }
}

fn refused(r: Result<Done, Fail>, needle: &str) {
    match r {
        Err(f @ Fail::Refused(_)) => {
            assert_eq!(f.code(), EXIT_REFUSED);
            let line = f.report(&redactor());
            assert!(line.contains(needle), "{line}");
            no_secret(&[&line]);
        }
        other => panic!("expected a refusal containing {needle:?}, got {other:?}"),
    }
}

fn snapshot(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap()
}

fn statuses(path: &Path) -> Vec<String> {
    let c = Connection::open(path).unwrap();
    let mut s = c
        .prepare("SELECT status FROM customer ORDER BY id")
        .unwrap();
    s.query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn has_table(path: &Path, name: &str) -> bool {
    Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = ?1",
            [name],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        == 1
}

const SQL: &str =
    "CREATE TABLE extra (x); UPDATE customer SET status = 'blocked' WHERE status = 'active'";

#[test]
fn a_file_inside_the_repository_plans_applies_and_restores_with_the_real_clock() {
    let r = Sandbox::new();
    let db = r.repo.join("app.db");
    let t = r.target("LOCALDB", &db);
    assert!(t.writable && t.class == "local", "{:?}", t.evidence);
    let (log, clock, bk) = (r.log(), SystemClock, EngineBackup::new(&NoExec));
    let (plans, backups) = r.dirs();
    let run = Runner {
        connector: &SqliteConnector,
        backup: &bk,
        log: &log,
        clock: &clock,
        plans_dir: plans.clone(),
        backups_dir: backups.clone(),
    };
    let d = run.plan(&t, SQL, "journey", "0,2").unwrap();
    let id = d
        .out
        .lines()
        .find_map(|l| l.strip_prefix("Plan "))
        .and_then(|l| l.split_whitespace().next())
        .unwrap()
        .to_string();
    // Plan id from the UTC clock: YYYYMMDD-HHMMSS-xxxx, and the plan's time in UTC.
    let b = id.as_bytes();
    assert!(
        id.len() == 20
            && b[8] == b'-'
            && b[15] == b'-'
            && id[..8].bytes().all(|c| c.is_ascii_digit())
            && id[9..15].bytes().all(|c| c.is_ascii_digit())
            && id[16..].bytes().all(|c| c.is_ascii_hexdigit()),
        "{id}"
    );
    let plan_text = std::fs::read_to_string(plans.join(format!("{id}.json"))).unwrap();
    assert!(plan_text.contains("+00:00"), "{plan_text}");
    let d = run.apply(&t, &id, true, 30.0).unwrap();
    assert_eq!(d.code, EXIT_OK, "{}", d.out);
    assert!(d.out.contains("row counts [0, 2]"), "{}", d.out);
    assert!(has_table(&db, "extra"));
    assert_eq!(statuses(&db), ["blocked", "blocked", "blocked"]);
    let backup = backups.join(format!("{id}.sqlite"));
    assert_eq!(
        std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&backup).unwrap().permissions())
            & 0o777,
        0o600
    );
    let d = run.restore(&t, &id, true).unwrap();
    assert_eq!(d.code, EXIT_OK);
    assert!(
        !has_table(&db, "extra"),
        "the table the plan created is gone"
    );
    assert_eq!(statuses(&db), ["active", "active", "blocked"]);
    no_secret(&[&d.out, &r.written()]);
}

#[test]
fn a_file_outside_the_repository_or_behind_a_link_is_never_planned_applied_or_restored() {
    let r = Sandbox::new();
    let prod = r.outside.join("prod.db");
    let before = snapshot(&prod);
    let link_out = r.repo.join("link-out.db");
    let link_in = r.repo.join("link-in.db");
    std::os::unix::fs::symlink(&prod, &link_out).unwrap();
    std::os::unix::fs::symlink(r.repo.join("app.db"), &link_in).unwrap();
    let (log, clock, bk) = (r.log(), SystemClock, EngineBackup::new(&NoExec));
    let (plans, backups) = r.dirs();
    let run = Runner {
        connector: &SqliteConnector,
        backup: &bk,
        log: &log,
        clock: &clock,
        plans_dir: plans,
        backups_dir: backups,
    };
    for (name, path) in [
        ("OUTSIDE", &prod),
        ("LINKOUT", &link_out),
        ("LINKIN", &link_in),
    ] {
        let t = r.target(name, path);
        assert!(!t.writable, "{name}: {:?}", t.evidence);
        assert_eq!(t.class, "production", "{name}: declared local, not proven");
        refused(
            run.plan(&t, SQL, "journey", "0,2"),
            "needs a proven local database",
        );
        refused(
            run.apply(&t, "20260101-000000-abcd", true, 30.0),
            "needs a proven local database",
        );
        refused(
            run.restore(&t, "20260101-000000-abcd", true),
            "needs a proven local database",
        );
    }
    assert_eq!(snapshot(&prod), before, "the outside database is unchanged");
    no_secret(&[&r.written()]);
}

#[test]
fn a_file_swapped_for_a_link_after_its_plan_is_refused_by_the_proof_and_by_the_connector() {
    let r = Sandbox::new();
    let db = r.repo.join("app.db");
    let prod = r.outside.join("prod.db");
    let before = snapshot(&prod);
    let (log, clock, bk) = (r.log(), SystemClock, EngineBackup::new(&NoExec));
    let (plans, backups) = r.dirs();
    let run = Runner {
        connector: &SqliteConnector,
        backup: &bk,
        log: &log,
        clock: &clock,
        plans_dir: plans,
        backups_dir: backups,
    };
    let proven = r.target("LOCALDB", &db);
    let d = run.plan(&proven, SQL, "journey", "0,2").unwrap();
    let id = d
        .out
        .lines()
        .find_map(|l| l.strip_prefix("Plan "))
        .and_then(|l| l.split_whitespace().next())
        .unwrap()
        .to_string();
    // The repository file becomes a link to the database outside.
    std::fs::remove_file(&db).unwrap();
    std::os::unix::fs::symlink(&prod, &db).unwrap();
    // A command proves again: refused.
    let now = r.target("LOCALDB", &db);
    assert!(!now.writable);
    refused(
        run.apply(&now, &id, true, 30.0),
        "needs a proven local database",
    );
    // A stale proof reaches the connector anyway: the write is refused there, nothing is written outside.
    let d = run.apply(&proven, &id, true, 30.0).unwrap();
    assert_ne!(d.code, EXIT_OK, "{}", d.out);
    assert!(
        d.out.contains("symbolic link") || d.err.contains("symbolic link"),
        "{d:?}"
    );
    // And a restore through the link is refused by the backup layer too.
    assert!(run.restore(&proven, &id, true).is_err());
    assert_eq!(snapshot(&prod), before, "the outside database is unchanged");
    no_secret(&[&d.out, &d.err, &r.written()]);
}
