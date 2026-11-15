//! T008: the Backup trait and its engine implementation, with a fake exec (what ran, with which
//! arguments and stdin) and, on SQLite, the real backup API. Every refusal test also checks that the
//! fake password appears in no error text and in no argument the tool received.
use ai_gent_dbrun::core::backup::{
    Backup, BackupError, BackupTarget, EngineBackup, Exec, ExecOutput, ExecSpec, backup_supported,
};
use std::cell::RefCell;
use std::path::Path;

const PW: &str = "pw-FAKE-s3cr3t";
const ID: &str = "20260101-120000-ab12";

#[derive(Debug, Clone)]
struct Seen {
    container: String,
    script: String,
    user: String,
    database: String,
    stdin: Vec<u8>,
    stdin_file: Option<String>,
    stdout_file: Option<String>,
}

/// Records every spec; answers with `code`, writing `dump` to the stdout file when asked.
struct Fake {
    seen: RefCell<Vec<Seen>>,
    code: i32,
    stderr: String,
    dump: &'static [u8],
}

impl Fake {
    fn ok() -> Self {
        Self {
            seen: RefCell::default(),
            code: 0,
            stderr: String::new(),
            dump: b"DUMP",
        }
    }
}

impl Exec for Fake {
    fn run(&self, s: &ExecSpec<'_>) -> Result<ExecOutput, String> {
        self.seen.borrow_mut().push(Seen {
            container: s.container.into(),
            script: s.script.into(),
            user: s.user.into(),
            database: s.database.into(),
            stdin: s.stdin.to_vec(),
            stdin_file: s.stdin_file.map(|p| p.to_string_lossy().into()),
            stdout_file: s.stdout_file.map(|p| p.to_string_lossy().into()),
        });
        if let Some(p) = s.stdout_file {
            std::fs::write(p, self.dump).unwrap();
            if self.code != 0 {
                std::fs::remove_file(p).unwrap();
            }
        }
        Ok(ExecOutput {
            code: self.code,
            stderr_tail: self.stderr.clone(),
        })
    }
}

fn target(engine: &str, db: Option<&str>) -> BackupTarget {
    BackupTarget {
        engine: engine.into(),
        database: db.map(Into::into),
        user: Some("app".into()),
        password: Some(PW.into()),
        host: Some("db.internal.example".into()),
        path: None,
        container: Some("shop-db-1".into()),
    }
}

fn no_secret(s: &Seen) {
    assert!(!s.script.contains(PW) && !s.user.contains(PW) && !s.database.contains(PW));
    assert!(!s.container.contains(PW));
}

fn assert_refused(r: Result<impl std::fmt::Debug, BackupError>, needle: &str) {
    match r {
        Err(BackupError::Refused(m)) => {
            assert!(m.contains(needle), "{m}");
            assert!(!m.contains(PW), "{m}");
        }
        other => panic!("expected a refusal containing {needle:?}, got {other:?}"),
    }
}

#[test]
fn postgres_backup_drives_pg_dump_with_the_password_on_stdin_only() {
    let dir = tempfile::tempdir().unwrap();
    let fake = Fake::ok();
    let b = EngineBackup::new(&fake);
    let loc = b
        .take(&target("postgresql", Some("shop")), ID, dir.path())
        .unwrap();
    assert_eq!(
        loc,
        dir.path().join(format!("{ID}.pgdump")).to_string_lossy()
    );
    let seen = fake.seen.borrow();
    let s = &seen[0];
    assert!(
        s.script.contains("exec pg_dump -Fc -h 127.0.0.1"),
        "{}",
        s.script
    );
    assert_eq!(
        (s.container.as_str(), s.user.as_str(), s.database.as_str()),
        ("shop-db-1", "app", "shop")
    );
    assert_eq!(s.stdin, format!("{PW}\n").into_bytes());
    no_secret(s);
    assert_eq!(s.stdout_file.as_deref(), Some(loc.as_str()));
    assert_eq!(std::fs::read(&loc).unwrap(), b"DUMP");
}

#[test]
fn mysql_and_mariadb_use_their_dump_tool_and_need_a_database() {
    let dir = tempfile::tempdir().unwrap();
    let fake = Fake::ok();
    let b = EngineBackup::new(&fake);
    b.take(&target("mariadb", Some("shop")), ID, dir.path())
        .unwrap();
    let id2 = "20260101-120001-ab12";
    b.take(&target("mysql", Some("shop")), id2, dir.path())
        .unwrap();
    let seen = fake.seen.borrow();
    assert!(seen[0].script.contains("command -v mariadb-dump"));
    assert!(seen[1].script.contains("command -v mysqldump ||"));
    for s in seen.iter() {
        assert!(
            s.script.contains(
                "--single-transaction --routines --triggers --events --add-drop-database"
            )
        );
        assert!(s.script.contains("--databases \"$2\""));
        no_secret(s);
    }
    drop(seen);
    let err = b
        .take(&target("mysql", None), "20260101-120002-ab12", dir.path())
        .unwrap_err();
    assert!(
        matches!(&err, BackupError::Failed(m) if m.contains("needs the profile's database")),
        "{err:?}"
    );
    assert_eq!(fake.seen.borrow().len(), 2, "no exec without a database");
}

#[test]
fn a_system_database_is_refused_for_backup_and_restore_and_nothing_runs() {
    let dir = tempfile::tempdir().unwrap();
    let fake = Fake::ok();
    let b = EngineBackup::new(&fake);
    for (engine, db) in [
        ("sqlserver", Some("master")),
        ("sqlserver", None),
        ("sqlserver", Some("")),
        ("sqlserver", Some("TempDB")),
        ("mysql", Some("mysql")),
        ("mariadb", Some("sys")),
        ("postgresql", Some("template1")),
    ] {
        let t = target(engine, db);
        assert_refused(b.take(&t, ID, dir.path()), "system database");
        assert_refused(b.restore(&t, ID, "anything", dir.path()), "system database");
    }
    assert!(fake.seen.borrow().is_empty(), "no exec ran");
}

#[test]
fn engines_without_a_method_are_refused_not_guessed() {
    let dir = tempfile::tempdir().unwrap();
    let fake = Fake::ok();
    let b = EngineBackup::new(&fake);
    assert_refused(
        b.take(&target("oracle", Some("x")), ID, dir.path()),
        "no backup method for oracle",
    );
    assert_refused(
        b.take(&target("sqlserver", Some("shop")), ID, dir.path()),
        "autocommit path",
    );
    assert_refused(
        b.restore(&target("oracle", Some("x")), ID, "x", dir.path()),
        "no restore method",
    );
    assert!(
        !backup_supported("oracle") && backup_supported("sqlserver") && !backup_supported("nope")
    );
    assert!(fake.seen.borrow().is_empty());
}

#[test]
fn unsafe_inputs_are_refused_before_any_exec() {
    let dir = tempfile::tempdir().unwrap();
    let fake = Fake::ok();
    let b = EngineBackup::new(&fake);
    // A database name pg_dump would read as a connection URI, and names read as options.
    for db in [
        "postgresql://u:p@evil.example/x",
        "-x",
        "a b",
        "a;b",
        "a\nb",
        "$(id)",
    ] {
        assert_refused(
            b.take(&target("postgresql", Some(db)), ID, dir.path()),
            "database",
        );
    }
    let mut t = target("postgresql", Some("shop"));
    t.user = Some("--help".into());
    assert_refused(b.take(&t, ID, dir.path()), "user");
    let mut t = target("postgresql", Some("shop"));
    t.container = Some("-v".into());
    assert_refused(b.take(&t, ID, dir.path()), "container");
    t.container = None;
    assert_refused(b.take(&t, ID, dir.path()), "container");
    let mut t = target("postgresql", Some("shop"));
    t.password = Some(format!("{PW}\nPGHOST=evil.example"));
    let err = b.take(&t, ID, dir.path()).unwrap_err();
    assert!(
        matches!(&err, BackupError::Refused(m) if m.contains("line break") && !m.contains(PW)),
        "{err:?}"
    );
    assert!(fake.seen.borrow().is_empty(), "no exec ran");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn a_failed_tool_is_an_error_without_secrets_and_leaves_no_backup_file() {
    let dir = tempfile::tempdir().unwrap();
    let fake = Fake {
        code: 1,
        stderr: format!(
            "pg_dump: error: connection to server at \"db.internal.example\" failed: password {PW} rejected for user \"app\"\n"
        ),
        ..Fake::ok()
    };
    let b = EngineBackup::new(&fake);
    let err = b
        .take(&target("postgresql", Some("shop")), ID, dir.path())
        .unwrap_err();
    let BackupError::Failed(m) = err else {
        panic!("expected a failure")
    };
    assert!(m.starts_with("backup/restore command failed: "), "{m}");
    for secret in [PW, "db.internal.example"] {
        assert!(!m.contains(secret), "{m}");
    }
    assert!(m.contains("<password>"), "{m}");
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        0,
        "no partial file"
    );
}

#[test]
fn restore_streams_the_recorded_file_and_refuses_any_other_location() {
    let dir = tempfile::tempdir().unwrap();
    let fake = Fake::ok();
    let b = EngineBackup::new(&fake);
    let t = target("postgresql", Some("shop"));
    let loc = b.take(&t, ID, dir.path()).unwrap();
    b.restore(&t, ID, &loc, dir.path()).unwrap();
    {
        let seen = fake.seen.borrow();
        let r = &seen[1];
        assert!(r.script.contains(
            "pg_restore --clean --create --if-exists --no-owner -h 127.0.0.1 -U \"$1\" -d template1"
        ));
        assert_eq!(r.stdin, format!("{PW}\n").into_bytes());
        assert_eq!(r.stdin_file.as_deref(), Some(loc.as_str()));
        no_secret(r);
    }
    let n = fake.seen.borrow().len();
    let other = dir.path().join("20260101-120009-ffff.pgdump");
    for bad in [
        "/etc/passwd".to_string(),
        "../x".to_string(),
        other.to_string_lossy().into_owned(),
        format!("{loc}/../{ID}.pgdump"),
        format!("container:/var/opt/mssql/data/ai-gent-{ID}.bak"),
        String::new(),
    ] {
        assert_refused(
            b.restore(&t, ID, &bad, dir.path()),
            "recorded backup location",
        );
    }
    assert_eq!(fake.seen.borrow().len(), n, "a bad location runs nothing");
    // The MySQL restore feeds the mysql client.
    let my = target("mysql", Some("shop"));
    let loc = b.take(&my, "20260101-120005-ab12", dir.path()).unwrap();
    b.restore(&my, "20260101-120005-ab12", &loc, dir.path())
        .unwrap();
    assert!(
        fake.seen
            .borrow()
            .last()
            .unwrap()
            .script
            .contains("command -v mariadb || command -v mysql")
    );
}

#[test]
fn restore_refuses_a_missing_directory_or_empty_backup_and_never_runs_the_tool() {
    let dir = tempfile::tempdir().unwrap();
    let fake = Fake::ok();
    let b = EngineBackup::new(&fake);
    for engine in ["mysql", "mariadb", "postgresql"] {
        let t = target(engine, Some("shop"));
        let ext = if engine == "postgresql" {
            "pgdump"
        } else {
            "sql"
        };
        let path = dir.path().join(format!("{ID}.{ext}"));
        let loc = path.to_string_lossy().into_owned();
        // missing
        let e = b.restore(&t, ID, &loc, dir.path()).unwrap_err();
        assert!(
            matches!(&e, BackupError::Failed(m) if m.contains("cannot read the backup")),
            "{e:?}"
        );
        // a directory
        std::fs::create_dir(&path).unwrap();
        assert_refused(
            b.restore(&t, ID, &loc, dir.path()),
            "non-empty regular file",
        );
        std::fs::remove_dir(&path).unwrap();
        // empty
        std::fs::write(&path, b"").unwrap();
        assert_refused(
            b.restore(&t, ID, &loc, dir.path()),
            "non-empty regular file",
        );
        std::fs::remove_file(&path).unwrap();
    }
    assert!(fake.seen.borrow().is_empty(), "the tool never ran");
}

#[cfg(feature = "sqlite")]
mod sqlite {
    use super::*;
    use rusqlite::Connection;

    fn db(dir: &Path) -> BackupTarget {
        let path = dir.join("app.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE c (id INTEGER); INSERT INTO c VALUES (1),(2),(3);")
            .unwrap();
        BackupTarget {
            engine: "sqlite".into(),
            path: Some(path.to_string_lossy().into()),
            ..BackupTarget::default()
        }
    }

    fn rows(t: &BackupTarget) -> i64 {
        Connection::open(t.path.as_deref().unwrap())
            .unwrap()
            .query_row("SELECT COUNT(*) FROM c", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn sqlite_roundtrip_through_the_backup_api() {
        let dir = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        let t = db(dir.path());
        let fake = Fake::ok();
        let b = EngineBackup::new(&fake);
        let loc = b.take(&t, ID, backups.path()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&loc).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        Connection::open(t.path.as_deref().unwrap())
            .unwrap()
            .execute_batch("DELETE FROM c WHERE id > 1")
            .unwrap();
        assert_eq!(rows(&t), 1);
        b.restore(&t, ID, &loc, backups.path()).unwrap();
        assert_eq!(rows(&t), 3);
        assert!(
            fake.seen.borrow().is_empty(),
            "SQLite never uses the container exec"
        );
        // A backup of the same plan is never overwritten.
        assert!(b.take(&t, ID, backups.path()).is_err());
    }

    #[test]
    fn a_file_that_is_not_sqlite_is_never_restored_over_the_database() {
        let dir = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        let t = db(dir.path());
        let fake = Fake::ok();
        let b = EngineBackup::new(&fake);
        let loc = b.take(&t, ID, backups.path()).unwrap();
        std::fs::write(&loc, format!("not sqlite {PW}")).unwrap();
        assert_refused(
            b.restore(&t, ID, &loc, backups.path()),
            "not a SQLite database",
        );
        assert_eq!(rows(&t), 3);
        std::fs::remove_file(&loc).unwrap();
        let err = b.restore(&t, ID, &loc, backups.path()).unwrap_err();
        assert!(matches!(err, BackupError::Failed(_)), "{err:?}");
        assert_eq!(rows(&t), 3);
    }

    const USER: &str = "backup_user_q";
    const HOST: &str = "backup-host.example";

    fn failed(r: Result<impl std::fmt::Debug, BackupError>, needle: &str) -> String {
        match r {
            Err(BackupError::Failed(m)) => {
                assert!(m.contains(needle), "{m}");
                for secret in [PW, USER, HOST, "app.db", "pipe.db", "/"] {
                    assert!(!m.contains(secret), "{secret} in: {m}");
                }
                m
            }
            other => panic!("expected a failure containing {needle:?}, got {other:?}"),
        }
    }

    fn secret(mut t: BackupTarget) -> BackupTarget {
        t.password = Some(PW.into());
        t.user = Some(USER.into());
        t.host = Some(HOST.into());
        t
    }

    fn leftovers(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("restoring"))
            .collect()
    }

    fn wipe(t: &BackupTarget) {
        Connection::open(t.path.as_deref().unwrap())
            .unwrap()
            .execute_batch("DELETE FROM c")
            .unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_sqlite_database_or_backup_behind_a_link_is_neither_backed_up_nor_restored() {
        let dir = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        let good = secret(db(dir.path()));
        let fake = Fake::ok();
        let b = EngineBackup::new(&fake);
        let loc = b.take(&good, ID, backups.path()).unwrap();
        // The database path is a link (to the real database): refused both ways, nothing written.
        let link = dir.path().join("link.db");
        std::os::unix::fs::symlink(good.path.as_deref().unwrap(), &link).unwrap();
        let mut t = good.clone();
        t.path = Some(link.to_string_lossy().into());
        failed(
            b.take(&t, "20260101-120000-cd34", backups.path()),
            "symbolic link",
        );
        failed(b.restore(&t, ID, &loc, backups.path()), "symbolic link");
        // The backup file is swapped for a link to another valid database: refused, the database untouched.
        let other = dir.path().join("other.db");
        Connection::open(&other)
            .unwrap()
            .execute_batch("CREATE TABLE c (id INTEGER)")
            .unwrap();
        std::fs::remove_file(&loc).unwrap();
        std::os::unix::fs::symlink(&other, &loc).unwrap();
        failed(b.restore(&good, ID, &loc, backups.path()), "symbolic link");
        assert_eq!(rows(&good), 3);
        assert!(leftovers(backups.path()).is_empty());
    }

    #[test]
    fn a_sqlite_restore_copies_from_a_private_snapshot_and_leaves_none_behind() {
        let dir = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        let t = secret(db(dir.path()));
        let fake = Fake::ok();
        let mut b = EngineBackup::new(&fake);
        let loc = b.take(&t, ID, backups.path()).unwrap();
        wipe(&t);
        b.restore(&t, ID, &loc, backups.path()).unwrap();
        assert_eq!(rows(&t), 3);
        assert!(
            leftovers(backups.path()).is_empty(),
            "snapshot removed after success"
        );
        // A failing restore (cap below the backup) removes its snapshot too, and writes nothing.
        wipe(&t);
        b.max_bytes = 100;
        failed(b.restore(&t, ID, &loc, backups.path()), "size cap");
        assert_eq!(rows(&t), 0);
        assert!(
            leftovers(backups.path()).is_empty(),
            "snapshot removed after failure"
        );
    }

    fn files_in(dir: &Path) -> usize {
        std::fs::read_dir(dir).unwrap().count()
    }

    #[test]
    fn a_sqlite_backup_of_a_locked_database_gives_up_at_the_deadline_and_leaves_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        let t = secret(db(dir.path()));
        // Another process's EXCLUSIVE lock keeps every reader out (rollback journal).
        let holder = Connection::open(t.path.as_deref().unwrap()).unwrap();
        holder.execute_batch("BEGIN EXCLUSIVE").unwrap();
        let fake = Fake::ok();
        let mut b = EngineBackup::new(&fake);
        b.timeout = std::time::Duration::from_millis(300);
        let started = std::time::Instant::now();
        failed(b.take(&t, ID, backups.path()), "is the database in use");
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
        assert_eq!(files_in(backups.path()), 0, "no partial backup is left");
        holder.execute_batch("ROLLBACK").unwrap();
        assert!(
            b.take(&t, ID, backups.path()).is_ok(),
            "free again: the backup works"
        );
    }

    #[test]
    fn a_sqlite_restore_into_a_database_in_use_gives_up_and_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        let t = secret(db(dir.path()));
        let fake = Fake::ok();
        let mut b = EngineBackup::new(&fake);
        let loc = b.take(&t, ID, backups.path()).unwrap();
        Connection::open(t.path.as_deref().unwrap())
            .unwrap()
            .execute_batch("DELETE FROM c WHERE id > 1")
            .unwrap();
        let holder = Connection::open(t.path.as_deref().unwrap()).unwrap();
        holder.execute_batch("BEGIN EXCLUSIVE").unwrap();
        b.timeout = std::time::Duration::from_millis(300);
        let started = std::time::Instant::now();
        failed(
            b.restore(&t, ID, &loc, backups.path()),
            "is the database in use",
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
        holder.execute_batch("ROLLBACK").unwrap();
        assert_eq!(rows(&t), 1, "a restore that did not finish wrote nothing");
        b.restore(&t, ID, &loc, backups.path()).unwrap();
        assert_eq!(rows(&t), 3);
    }

    #[test]
    fn a_sqlite_database_over_the_size_cap_is_not_backed_up() {
        let dir = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        let t = secret(db(dir.path()));
        let fake = Fake::ok();
        let mut b = EngineBackup::new(&fake);
        b.max_bytes = 1024;
        failed(b.take(&t, ID, backups.path()), "size cap");
        assert_eq!(files_in(backups.path()), 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_sqlite_path_that_is_a_fifo_or_a_directory_is_neither_backed_up_nor_restored_into() {
        let dir = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        let good = secret(db(dir.path()));
        let fake = Fake::ok();
        let b = EngineBackup::new(&fake);
        let loc = b.take(&good, ID, backups.path()).unwrap();
        let fifo = dir.path().join("pipe.db");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        let started = std::time::Instant::now();
        for path in [fifo.clone(), dir.path().to_path_buf()] {
            let mut t = good.clone();
            t.path = Some(path.to_string_lossy().into());
            failed(
                b.take(&t, "20260101-120000-cd34", backups.path()),
                "not a regular file",
            );
            failed(
                b.restore(&t, ID, &loc, backups.path()),
                "not a regular file",
            );
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "nothing blocked"
        );
        assert_eq!(files_in(backups.path()), 1, "only the good backup exists");
    }
}

/// The real exec against a stand-in "container engine" script: the deadline kills a hang, the size cap
/// stops a flood, and a failure leaves no dump file. Unix only (the stand-in is a shell script).
#[cfg(unix)]
mod real_exec {
    use super::*;
    use ai_gent_core::env::Env;
    use ai_gent_dbrun::core::backup::ContainerExec;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    fn engine(dir: &Path, body: &str) -> ContainerExec {
        let p = dir.join("fake-engine");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        ContainerExec::new(p, Env::real())
    }

    fn spec<'a>(out: &'a Path, stdin: &'a [u8], timeout: Duration, cap: u64) -> ExecSpec<'a> {
        ExecSpec {
            container: "c1",
            script: "x",
            user: "u",
            database: "d",
            stdin,
            stdin_file: None,
            stdout_file: Some(out),
            timeout,
            max_stdout: cap,
        }
    }

    #[test]
    fn arguments_carry_no_password_and_stdin_reaches_the_tool() {
        let d = tempfile::tempdir().unwrap();
        let argfile = d.path().join("args");
        let e = engine(
            d.path(),
            &format!(
                "echo \"$@\" > {}; read -r line; echo \"got:$line\"",
                argfile.display()
            ),
        );
        let out = d.path().join("dump");
        let r = e
            .run(&spec(
                &out,
                format!("{PW}\n").as_bytes(),
                Duration::from_secs(10),
                1 << 20,
            ))
            .unwrap();
        assert_eq!(r.code, 0);
        let argv = std::fs::read_to_string(&argfile).unwrap();
        assert_eq!(argv.trim(), "exec -i c1 sh -c x sh u d");
        assert!(!argv.contains(PW));
        assert_eq!(
            std::fs::read_to_string(&out).unwrap().trim(),
            format!("got:{PW}")
        );
        assert_eq!(
            std::fs::metadata(&out).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn a_hang_is_killed_at_the_deadline_and_leaves_no_file() {
        let d = tempfile::tempdir().unwrap();
        let e = engine(d.path(), "sleep 30");
        let out = d.path().join("dump");
        let t0 = Instant::now();
        let r = e
            .run(&spec(&out, b"x\n", Duration::from_millis(300), 1 << 20))
            .unwrap();
        assert!(t0.elapsed() < Duration::from_secs(10));
        assert_eq!(r.code, 124);
        assert!(!out.exists());
    }

    #[test]
    fn a_flood_past_the_cap_is_stopped_and_the_partial_file_removed() {
        let d = tempfile::tempdir().unwrap();
        let e = engine(d.path(), "yes AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
        let out = d.path().join("dump");
        let t0 = Instant::now();
        let r = e
            .run(&spec(&out, b"x\n", Duration::from_secs(30), 100_000))
            .unwrap();
        assert!(t0.elapsed() < Duration::from_secs(20));
        assert_eq!(r.code, 125);
        assert!(!out.exists());
    }

    #[test]
    fn a_stdin_file_that_cannot_be_opened_fails_the_exec_before_the_engine_runs() {
        let d = tempfile::tempdir().unwrap();
        let ran = d.path().join("ran");
        // The stand-in would exit 0 on empty input, as a SQL client does.
        let e = engine(
            d.path(),
            &format!("touch {}; cat > /dev/null", ran.display()),
        );
        let unused = d.path().join("unused");
        let mut s = spec(&unused, b"x\n", Duration::from_secs(5), 10);
        s.stdout_file = None;
        let missing = d.path().join("missing.sql");
        s.stdin_file = Some(&missing);
        let err = e.run(&s).unwrap_err();
        assert!(err.contains("cannot open the backup file"), "{err}");
        assert!(!ran.exists(), "the engine was not started");
    }

    #[test]
    fn a_missing_engine_program_is_a_fixed_failure_and_creates_no_file() {
        let d = tempfile::tempdir().unwrap();
        let e = ContainerExec::new(d.path().join("nope"), Env::real());
        let out = d.path().join("dump");
        let err = e
            .run(&spec(&out, b"x\n", Duration::from_secs(5), 10))
            .unwrap_err();
        assert!(!err.contains(d.path().to_str().unwrap()), "{err}");
        assert!(!out.exists());
    }
}
