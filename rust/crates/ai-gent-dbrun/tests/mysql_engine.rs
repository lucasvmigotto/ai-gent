//! The MySQL and MariaDB connector against real servers (spec 043 T040-T042, FR-017).
//!
//! `#[ignore]`: needs Podman (or Docker, `AI_GENT_TEST_RUNNER=docker`) and the images
//! `docker.io/library/mysql:8.4` and `docker.io/library/mariadb:11.4` (pulled on first use). Run:
//!
//! ```text
//! cargo test -p ai-gent-dbrun --features mysql --test mysql_engine -- --ignored --test-threads=1
//! ```
//!
//! One container per engine and test, published on a random loopback port, with a memory cap and the
//! project's ownership label (`shared/containers.md` 9a); it is removed by its ID when the test ends (a
//! guard), never by a prune. Images are pinned by tag; TODO pin them by digest as well. The server certificate
//! is signed by a throwaway CA made in the test, so every connection here is TLS and verified.
//! Passwords are throwaway values of these containers only.
#![cfg(feature = "mysql")]
#![allow(clippy::too_many_lines)]
use ai_gent_dbrun::connector::contract::{Profile, run_exec_cases};
use ai_gent_dbrun::connector::mysql::{MysqlConnector, TlsPolicy};
use ai_gent_dbrun::connector::{ConnFacts, Connector, Mode, Request};
use ai_gent_dbrun::core::tls::{ClassKind, TlsSettings, testca::TestCa};
use mysql::prelude::Queryable;
use mysql::{Conn, OptsBuilder, SslOpts};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const PASSWORD: &str = "Dbrun_Test_1";
const USER: &str = "root";

fn runner() -> String {
    std::env::var("AI_GENT_TEST_RUNNER").unwrap_or_else(|_| "podman".into())
}

struct Container {
    id: String,
    port: u16,
    _certs: tempfile::TempDir,
    ca: PathBuf,
    family: &'static str,
}

impl Drop for Container {
    fn drop(&mut self) {
        // By ID, with its anonymous volumes; never a prune.
        let _ = Command::new(runner())
            .args(["rm", "-f", "-v", &self.id])
            .output();
    }
}

fn start(family: &'static str) -> Container {
    let (image, env) = match family {
        "mysql" => ("docker.io/library/mysql:8.4", "MYSQL_ROOT_PASSWORD"),
        _ => ("docker.io/library/mariadb:11.4", "MARIADB_ROOT_PASSWORD"),
    };
    let ca = TestCa::new(family);
    let certs = tempfile::tempdir().unwrap();
    let server = ca.issue_server(&["127.0.0.1", "localhost"]);
    let ca_path = ca.write_ca(certs.path());
    std::fs::write(certs.path().join("server.pem"), server.cert_pem).unwrap();
    std::fs::write(certs.path().join("server-key.pem"), server.key_pem).unwrap();
    // The container's server user must read these throwaway files.
    for f in ["ca.pem", "server.pem", "server-key.pem"] {
        set_mode(&certs.path().join(f));
    }
    set_dir_mode(certs.path());
    let out = Command::new(runner())
        .args([
            "run",
            "-d",
            "--memory",
            "1g",
            "--label",
            "me.lucasvmigotto.ai-gent.project=ai-gent",
        ])
        .args([
            "-p",
            "127.0.0.1::3306",
            "-e",
            &format!("{env}={PASSWORD}"),
            "-v",
        ])
        .arg(format!(
            "{}:/certs:{}",
            certs.path().display(),
            mount_options()
        ))
        .arg(image)
        .args([
            "--ssl-ca=/certs/ca.pem",
            "--ssl-cert=/certs/server.pem",
            "--ssl-key=/certs/server-key.pem",
            "--require-secure-transport=ON",
        ])
        .output()
        .expect("the container runner");
    assert!(
        out.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    // From here the guard owns the container.
    let mut c = Container {
        id,
        port: 0,
        _certs: certs,
        ca: ca_path,
        family,
    };
    let p = Command::new(runner())
        .args(["port", &c.id, "3306/tcp"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&p.stdout);
    c.port = text
        .lines()
        .find_map(|l| l.rsplit(':').next()?.trim().parse().ok())
        .expect("published port");
    wait_ready(&c);
    c
}

/// `ro`, and `Z` (a private `SELinux` label) only where the runner is Podman on a host with `SELinux`
/// enforcing; elsewhere (other Linux hosts, macOS, Docker Desktop) the option would be refused or wrong.
fn mount_options() -> &'static str {
    let selinux = std::fs::read_to_string("/sys/fs/selinux/enforce").is_ok_and(|v| v.trim() == "1");
    if selinux && runner().contains("podman") {
        "ro,Z"
    } else {
        "ro"
    }
}

#[cfg(unix)]
fn set_mode(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o644)).unwrap();
}
#[cfg(unix)]
fn set_dir_mode(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
}
#[cfg(not(unix))]
fn set_mode(_: &Path) {}
#[cfg(not(unix))]
fn set_dir_mode(_: &Path) {}

fn raw(c: &Container, database: Option<&str>) -> mysql::Result<Conn> {
    Conn::new(
        OptsBuilder::new()
            .ip_or_hostname(Some("127.0.0.1"))
            .tcp_port(c.port)
            .user(Some(USER))
            .pass(Some(PASSWORD))
            .db_name(database)
            .prefer_socket(false)
            .tcp_connect_timeout(Some(Duration::from_secs(5)))
            .read_timeout(Some(Duration::from_secs(60)))
            .ssl_opts(Some(
                SslOpts::default()
                    .with_danger_accept_invalid_certs(true)
                    .with_danger_skip_domain_validation(true),
            )),
    )
}

/// Waits (at most 180 s) until the server answers a query, not only a port: the entrypoint restarts
/// the server once while it initialises.
fn wait_ready(c: &Container) {
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut stable = 0;
    while Instant::now() < deadline {
        if raw(c, None)
            .and_then(|mut x| x.query_drop("SELECT 1"))
            .is_ok()
        {
            stable += 1;
            if stable >= 3 {
                return;
            }
        } else {
            stable = 0;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    panic!("the {} server did not become ready in 180 s", c.family);
}

fn facts(c: &Container, database: Option<&str>) -> ConnFacts {
    ConnFacts {
        host: Some("127.0.0.1".into()),
        port: Some(c.port.to_string()),
        database: database.map(String::from),
        user: Some(USER.into()),
        password: Some(PASSWORD.into()),
        ..ConnFacts::default()
    }
}

fn verified(c: &Container) -> MysqlConnector {
    MysqlConnector::new(TlsPolicy {
        settings: TlsSettings::from_fields(None, c.ca.to_str(), None, None).unwrap(),
        declared: ClassKind::Production,
        proven: ClassKind::Production,
    })
}

fn run(c: &MysqlConnector, req: &Request) -> (Value, String) {
    let line = c.run(req).to_line();
    (serde_json::from_str(&line).unwrap(), line)
}

fn no_secret(line: &str) {
    assert!(!line.contains(PASSWORD), "password in {line}");
}

fn request(c: &Container, mode: Mode, db: Option<&str>, sql: &[&str], timeout: f64) -> Request {
    let mut r = Request::new(
        c.family,
        facts(c, db),
        mode,
        sql.iter().map(|s| (*s).to_string()).collect(),
    );
    r.timeout = timeout;
    r
}

fn tls_checks(c: &Container) {
    let good = verified(c);
    // Verified against the throwaway CA: connects, and the session really is encrypted.
    let (out, line) = run(
        &good,
        &request(
            c,
            Mode::Read,
            None,
            &["SHOW SESSION STATUS LIKE 'Ssl_cipher'"],
            10.0,
        ),
    );
    assert_eq!(out["ok"], true, "{line}");
    let cipher = out["results"][0]["rows"][0][1].as_str().unwrap_or("");
    assert!(
        cipher.starts_with("TLS") || cipher.contains("AES") || cipher.contains("CHACHA"),
        "{line}"
    );

    // No CA file: the throwaway CA is not a root, so verification fails, names the field, leaks nothing.
    let (out, line) = run(
        &MysqlConnector::default(),
        &request(c, Mode::Read, None, &["SELECT 1"], 10.0),
    );
    assert_eq!(
        (out["ok"].clone(), out["stage"].clone()),
        (false.into(), "connect".into()),
        "{line}"
    );
    assert!(
        out["error"].as_str().unwrap().contains("tls_ca_file"),
        "{line}"
    );
    no_secret(&line);

    // A CA that did not sign the server certificate.
    let dir = tempfile::tempdir().unwrap();
    let rogue = TestCa::new("rogue").write_ca(dir.path());
    let other = MysqlConnector::new(TlsPolicy {
        settings: TlsSettings::from_fields(None, rogue.to_str(), None, None).unwrap(),
        ..TlsPolicy::default()
    });
    let (out, line) = run(&other, &request(c, Mode::Read, None, &["SELECT 1"], 10.0));
    assert_eq!(out["ok"], false, "{line}");
    assert_eq!(out["stage"], "connect", "{line}");
    no_secret(&line);

    // Unverified: refused on production, accepted on a proven-local loopback profile.
    let unverified = |declared, proven| {
        MysqlConnector::new(TlsPolicy {
            settings: TlsSettings::from_fields(Some("unverified"), None, None, None).unwrap(),
            declared,
            proven,
        })
    };
    let (out, line) = run(
        &unverified(ClassKind::Production, ClassKind::Production),
        &request(c, Mode::Test, None, &[], 10.0),
    );
    assert_eq!(out["ok"], false, "{line}");
    assert!(
        out["error"].as_str().unwrap().starts_with("tls_mode:"),
        "{line}"
    );
    let (out, line) = run(
        &unverified(ClassKind::Local, ClassKind::Local),
        &request(c, Mode::Test, None, &[], 10.0),
    );
    assert_eq!(out["ok"], true, "{line}");
    assert!(out["server"][0].as_str().unwrap().len() > 3, "{line}");
}

fn wrong_password(c: &Container) {
    let mut req = request(c, Mode::Read, None, &["SELECT 1"], 10.0);
    req.conn.password = Some("Not_The_Password_77".into());
    let (out, line) = run(&verified(c), &req);
    assert_eq!(out["stage"], "connect", "{line}");
    assert!(line.contains("1045"), "{line}");
    assert!(
        !line.contains("Not_The_Password_77") && !line.contains("'root'"),
        "{line}"
    );
    // An account the server does not have: refused as a connect failure that names neither the user
    // nor the password (MySQL answers with an authentication plugin the crate lacks; see Q-MY-10).
    req.conn.user = Some("someone_else".into());
    let (out, line) = run(&verified(c), &req);
    assert_eq!(
        (out["ok"].clone(), out["stage"].clone()),
        (false.into(), "connect".into()),
        "{line}"
    );
    assert!(
        !line.contains("Not_The_Password_77") && !line.contains("someone_else"),
        "{line}"
    );
}

fn fixtures(c: &Container) -> usize {
    let mut n = 0;
    let connector = verified(c);
    run_exec_cases(&connector, c.family, Profile::Port, |_case, setup| {
        n += 1;
        let db = format!("fx{n}");
        let mut conn = raw(c, None).unwrap();
        conn.query_drop(format!("CREATE DATABASE {db}")).unwrap();
        conn.query_drop(format!("USE {db}")).unwrap();
        for sql in setup {
            conn.query_drop(sql)
                .unwrap_or_else(|e| panic!("setup {sql}: {e}"));
        }
        facts(c, Some(&db))
    })
}

/// A write that waits on a row lock held by another session stops at `lock_wait_timeout` (5 s), well
/// before the socket timeout (`timeout + 5` s), and leaves the table as it was.
fn write_waits_on_a_lock(c: &Container) {
    let mut holder = raw(c, None).unwrap();
    holder.query_drop("CREATE DATABASE lockdb").unwrap();
    holder.query_drop("USE lockdb").unwrap();
    holder
        .query_drop("CREATE TABLE l (id INT PRIMARY KEY, n INT)")
        .unwrap();
    holder.query_drop("INSERT INTO l VALUES (1, 1)").unwrap();
    holder.query_drop("SET autocommit = 0").unwrap();
    holder
        .query_drop("UPDATE l SET n = 2 WHERE id = 1")
        .unwrap(); // holds the lock
    let started = Instant::now();
    let (out, line) = run(
        &verified(c),
        &request(
            c,
            Mode::Write,
            Some("lockdb"),
            &["UPDATE l SET n = 3 WHERE id = 1"],
            30.0,
        ),
    );
    let took = started.elapsed();
    assert_eq!(out["ok"], false, "{line}");
    assert_eq!(out["stage"], "execute", "{line}");
    assert!(out["error"].as_str().unwrap().contains("1205"), "{line}");
    assert!(
        took >= Duration::from_secs(4) && took < Duration::from_secs(15),
        "{took:?}"
    );
    holder.query_drop("ROLLBACK").unwrap();
    let n: Option<i64> = holder.query_first("SELECT n FROM l WHERE id = 1").unwrap();
    assert_eq!(n, Some(1));
    no_secret(&line);
}

/// A slow write is bounded: on MariaDB `max_statement_time` stops it (error 1969) after the request
/// timeout; on MySQL `MAX_EXECUTION_TIME` covers SELECT only, so the socket timeout (`timeout + 5` s)
/// ends the wait. Either way the call returns inside that bound and nothing is committed.
fn slow_write_is_bounded(c: &Container) {
    let mut s = raw(c, None).unwrap();
    s.query_drop("CREATE DATABASE slowdb").unwrap();
    s.query_drop("USE slowdb").unwrap();
    s.query_drop("CREATE TABLE s (id INT PRIMARY KEY, n INT)")
        .unwrap();
    s.query_drop("INSERT INTO s VALUES (1, 1)").unwrap();
    let started = Instant::now();
    let (out, line) = run(
        &verified(c),
        &request(
            c,
            Mode::Write,
            Some("slowdb"),
            &["UPDATE s SET n = 2 WHERE SLEEP(9) = 0"],
            1.0,
        ),
    );
    let took = started.elapsed();
    assert_eq!(out["ok"], false, "{line}");
    assert_eq!(out["stage"], "execute", "{line}");
    assert!(
        took < Duration::from_secs(8),
        "returned after {took:?}: {line}"
    );
    let error = out["error"].as_str().unwrap();
    if c.family == "mariadb" {
        assert!(error.contains("1969"), "{line}");
    } else {
        assert!(
            error.contains("did not answer within the request timeout"),
            "{line}"
        );
        assert!(took >= Duration::from_secs(5), "{took:?}");
    }
    // Let the server finish or abort the orphaned statement, then check nothing was committed.
    std::thread::sleep(Duration::from_secs(5));
    let n: Option<i64> = s.query_first("SELECT n FROM s WHERE id = 1").unwrap();
    assert_eq!(n, Some(1), "the slow write committed");
    no_secret(&line);
}

/// A read past `max_execution_time` / `max_statement_time` fails at the server in both families.
fn slow_read_is_stopped_by_the_server(c: &Container) {
    let started = Instant::now();
    let (out, line) = run(
        &verified(c),
        &request(c, Mode::Read, None, &["SELECT SLEEP(8)"], 1.0),
    );
    assert!(started.elapsed() < Duration::from_secs(6), "{line}");
    // MySQL: SLEEP is interrupted and returns 1 (ok); MariaDB raises 1969. Neither may run the 8 s out.
    if c.family == "mariadb" {
        assert_eq!(out["ok"], false, "{line}");
        assert!(out["error"].as_str().unwrap().contains("1969"), "{line}");
    }
}

fn the_timeout_variable_is_set_after_the_version(c: &Container) {
    let var = if c.family == "mariadb" {
        "SELECT @@max_statement_time"
    } else {
        "SELECT @@max_execution_time"
    };
    let (out, line) = run(&verified(c), &request(c, Mode::Read, None, &[var], 2.0));
    assert_eq!(out["ok"], true, "{line}");
    let v = &out["results"][0]["rows"][0][0];
    let got = v
        .as_f64()
        .or_else(|| v.as_str().and_then(|t| t.parse().ok()))
        .unwrap_or(-1.0);
    let want = if c.family == "mariadb" { 2.0 } else { 2000.0 };
    assert!((got - want).abs() < f64::EPSILON, "{line}");
}

fn guards(c: &Container) {
    // Multi-statement tricks, LOCAL INFILE and the server's own comment rules: none reaches the server,
    // and the table is untouched.
    let mut s = raw(c, None).unwrap();
    s.query_drop("CREATE DATABASE guarddb").unwrap();
    s.query_drop("USE guarddb").unwrap();
    s.query_drop("CREATE TABLE g (n INT)").unwrap();
    s.query_drop("INSERT INTO g VALUES (1)").unwrap();
    for sql in [
        "SELECT 1; DELETE FROM g",
        "SELECT 1--1; SET SESSION TRANSACTION READ WRITE; COMMIT; DELETE FROM g; COMMIT",
        "SELECT 1 #'\n; SET SESSION transaction_read_only = 0; COMMIT; DELETE FROM g; COMMIT; #'",
        "SELECT 1 /*!50000 FOR UPDATE */",
        "SELECT 1\\G",
        "LOAD DATA LOCAL INFILE '/etc/hostname' INTO TABLE g",
    ] {
        for mode in [Mode::Read, Mode::Write] {
            let (out, line) = run(
                &verified(c),
                &request(c, mode, Some("guarddb"), &["SELECT 0", sql], 10.0),
            );
            assert_eq!(out["ok"], false, "{sql}: {line}");
            assert_eq!(out["stage"], "execute", "{sql}: {line}");
            assert!(
                out.get("server").is_none(),
                "{sql}: reached the server: {line}"
            );
            no_secret(&line);
        }
    }
    let n: Option<i64> = s.query_first("SELECT COUNT(*) FROM g").unwrap();
    assert_eq!(n, Some(1));
}

/// In read mode the connector allows only reads, so a write statement is refused before it connects
/// (and so is anything that would turn the session read-write: the recheck after each statement cannot
/// be reached). What the allowlist still lets through is a read that calls a stored function that
/// writes; the server's read-only transaction stops that one (error 1792). The table stays as it was.
fn read_only_is_checked_after_every_statement(c: &Container) {
    let mut s = raw(c, None).unwrap();
    s.query_drop("CREATE DATABASE rodb").unwrap();
    s.query_drop("USE rodb").unwrap();
    s.query_drop("CREATE TABLE r (n INT)").unwrap();
    s.query_drop("INSERT INTO r VALUES (1)").unwrap();
    // The throwaway server has binary logging on, which refuses a function that writes unless trusted.
    s.query_drop("SET GLOBAL log_bin_trust_function_creators = 1")
        .unwrap();
    s.query_drop(
        "CREATE FUNCTION wr() RETURNS INT MODIFIES SQL DATA BEGIN DELETE FROM r; RETURN 1; END",
    )
    .unwrap();
    for sql in [
        "SET SESSION TRANSACTION READ WRITE",
        "DELETE FROM r",
        "CALL nothing()",
        "EXPLAIN ANALYZE SELECT * FROM r",
    ] {
        let (out, line) = run(
            &verified(c),
            &request(c, Mode::Read, Some("rodb"), &[sql], 10.0),
        );
        assert_eq!(out["ok"], false, "{sql}: {line}");
        assert_eq!(out["stage"], "execute", "{sql}: {line}");
        assert!(
            out.get("server").is_none(),
            "{sql}: reached the server: {line}"
        );
    }
    let (out, line) = run(
        &verified(c),
        &request(c, Mode::Read, Some("rodb"), &["SELECT wr()"], 10.0),
    );
    assert_eq!(out["ok"], false, "{line}");
    assert_eq!(out["stage"], "execute", "{line}");
    assert!(out["error"].as_str().unwrap().contains("1792"), "{line}");
    let n: Option<i64> = s.query_first("SELECT COUNT(*) FROM r").unwrap();
    assert_eq!(n, Some(1), "the read deleted a row");
}

/// The session reads text the way the lexer assumes: `"abc"` is a string (no `ANSI_QUOTES`), a
/// backslash escapes a quote (no `NO_BACKSLASH_ESCAPES`), and the connection character set is utf8mb4.
fn the_session_reads_text_as_the_lexer_assumes(c: &Container) {
    let (out, line) = run(
        &verified(c),
        &request(
            c,
            Mode::Read,
            None,
            &["SELECT \"abc\" AS a, 'x\\'y' AS b, CHARSET('z') AS c"],
            10.0,
        ),
    );
    assert_eq!(out["ok"], true, "{line}");
    assert_eq!(
        out["results"][0]["rows"][0],
        serde_json::json!(["abc", "x'y", "utf8mb4"]),
        "{line}"
    );
}

fn whole_family(family: &'static str) {
    let c = start(family);
    tls_checks(&c);
    wrong_password(&c);
    assert_eq!(fixtures(&c), 2, "the value-types and the JSON fixture ran");
    the_timeout_variable_is_set_after_the_version(&c);
    slow_read_is_stopped_by_the_server(&c);
    write_waits_on_a_lock(&c);
    slow_write_is_bounded(&c);
    guards(&c);
    read_only_is_checked_after_every_statement(&c);
    the_session_reads_text_as_the_lexer_assumes(&c);
}

#[test]
#[ignore = "needs Podman and the mysql:8.4 image"]
fn mysql_84_through_a_verified_tls_connection() {
    whole_family("mysql");
}

#[test]
#[ignore = "needs Podman and the mariadb:11.4 image"]
fn mariadb_114_through_a_verified_tls_connection() {
    whole_family("mariadb");
}
