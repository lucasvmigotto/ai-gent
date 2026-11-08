//! The PostgreSQL connector's connection path (spec 043 T030, T032) against a fake server that speaks
//! just enough of the wire protocol: the SSL request, a rustls handshake with a certificate from a
//! throwaway CA (`core::tls::testca`), the startup message, and an error response. No real engine and no
//! credential is involved; the passwords here are made up.
//!
//! What needs a real server (the `25006` refusal of a write in read mode, the multi-statement refusal,
//! values, counts) is in `real_server_*` at the bottom: ignored unless `AI_GENT_TEST_PG_HOST` is set.
#![cfg(feature = "postgres")]
use ai_gent_dbrun::connector::postgres::PostgresConnector;
use ai_gent_dbrun::connector::{ConnFacts, Connector, ExecResult, Mode, Request, Stage};
use ai_gent_dbrun::core::tls::testca::{ServerCert, TestCa};
use ai_gent_dbrun::core::tls::{ClassKind, TlsContext, TlsSettings};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PASSWORD: &str = "pw-Sup3rS3cret";
const USER: &str = "app_ro_user";
const MARKER: &str = "fake server reached";

enum Behaviour {
    /// Answers the SSL request with `S`, handshakes with this certificate, reads the startup message and
    /// answers an error response quoting the credentials, as a server's authentication error can.
    Tls(ServerCert),
    /// Answers the SSL request with `N` (a server without TLS) and records whether anything follows.
    NoTls,
    /// Accepts and never answers.
    Silent,
}

#[derive(Default)]
struct Stats {
    accepted: AtomicUsize,
    handshake_failed: AtomicUsize,
    startups: Mutex<Vec<Vec<u8>>>,
    bytes_after_refusal: AtomicUsize,
}

struct Fake {
    port: u16,
    stats: Arc<Stats>,
}

fn server_config(cert: &ServerCert) -> Arc<ServerConfig> {
    let certs: Vec<CertificateDer<'static>> =
        CertificateDer::pem_slice_iter(cert.cert_pem.as_bytes())
            .collect::<Result<_, _>>()
            .unwrap();
    let key = PrivateKeyDer::from_pem_slice(cert.key_pem.as_bytes()).unwrap();
    Arc::new(
        ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .unwrap(),
    )
}

fn error_response() -> Vec<u8> {
    let mut body = Vec::new();
    for (code, text) in [
        (b'S', "FATAL".to_string()),
        (b'C', "28000".to_string()),
        (
            b'M',
            format!(
                "{MARKER}: password authentication failed for user \"{USER}\" (password {PASSWORD})"
            ),
        ),
    ] {
        body.push(code);
        body.extend_from_slice(text.as_bytes());
        body.push(0);
    }
    body.push(0);
    let mut out = vec![b'E'];
    out.extend_from_slice(&u32::try_from(body.len() + 4).unwrap().to_be_bytes());
    out.extend_from_slice(&body);
    out
}

fn read_startup(stream: &mut impl Read) -> std::io::Result<Vec<u8>> {
    let mut len = [0u8; 4];
    stream.read_exact(&mut len)?;
    let n = usize::try_from(u32::from_be_bytes(len))
        .unwrap()
        .saturating_sub(4);
    let mut body = vec![0u8; n];
    stream.read_exact(&mut body)?;
    Ok(body)
}

fn fake(behaviour: &Behaviour) -> Fake {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let stats = Arc::new(Stats::default());
    let shared = Arc::clone(&stats);
    let config = match behaviour {
        Behaviour::Tls(c) => Some(server_config(c)),
        _ => None,
    };
    let silent = matches!(behaviour, Behaviour::Silent);
    let no_tls = matches!(behaviour, Behaviour::NoTls);
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut tcp) = conn else { return };
            shared.accepted.fetch_add(1, Ordering::SeqCst);
            let stats = Arc::clone(&shared);
            let config = config.clone();
            std::thread::spawn(move || {
                if silent {
                    std::thread::sleep(Duration::from_secs(30));
                    return;
                }
                let mut ssl_request = [0u8; 8];
                if tcp.read_exact(&mut ssl_request).is_err()
                    || ssl_request[4..] != 80_877_103u32.to_be_bytes()
                {
                    return;
                }
                if no_tls {
                    let _ = tcp.write_all(b"N");
                    tcp.set_read_timeout(Some(Duration::from_millis(400)))
                        .unwrap();
                    let mut rest = [0u8; 64];
                    if let Ok(n) = tcp.read(&mut rest) {
                        stats.bytes_after_refusal.fetch_add(n, Ordering::SeqCst);
                    }
                    return;
                }
                tcp.write_all(b"S").unwrap();
                let conn = ServerConnection::new(config.unwrap()).unwrap();
                let mut tls = StreamOwned::new(conn, tcp);
                match read_startup(&mut tls) {
                    Ok(body) => {
                        stats.startups.lock().unwrap().push(body);
                        let _ = tls.write_all(&error_response());
                        let _ = tls.flush();
                        tls.conn.send_close_notify();
                        let _ = tls.flush();
                    }
                    Err(_) => {
                        stats.handshake_failed.fetch_add(1, Ordering::SeqCst);
                    }
                }
            });
        }
    });
    Fake { port, stats }
}

fn ctx(declared: ClassKind, proven: ClassKind, host: &str) -> TlsContext<'_> {
    TlsContext {
        declared,
        proven,
        host,
    }
}

fn production(host: &str) -> TlsContext<'_> {
    ctx(ClassKind::Production, ClassKind::Production, host)
}

fn request(host: &str, port: u16) -> Request {
    let conn = ConnFacts {
        host: Some(host.into()),
        port: Some(port.to_string()),
        database: Some("shop".into()),
        user: Some(USER.into()),
        password: Some(PASSWORD.into()),
        ..ConnFacts::default()
    };
    let mut req = Request::new("postgresql", conn, Mode::Read, vec!["SELECT 1".into()]);
    req.timeout = 5.0;
    req
}

fn connector(ca: Option<&PathBuf>, server_name: Option<&str>, host: &str) -> PostgresConnector {
    let settings =
        TlsSettings::from_fields(None, ca.and_then(|p| p.to_str()), server_name, None).unwrap();
    PostgresConnector::authorized(&settings, &production(host)).unwrap()
}

/// No credential, key or host of the request may be in the result line.
fn assert_clean(out: &ExecResult, extra: &[&str]) {
    let line = out.to_line();
    for secret in [PASSWORD, "Sup3rS3cret", "PRIVATE KEY"].iter().chain(extra) {
        assert!(!line.contains(secret), "{secret} leaked: {line}");
    }
}

fn error(out: &ExecResult) -> &str {
    out.error.as_deref().unwrap_or_default()
}

fn startup_text(fake: &Fake) -> String {
    let all = fake.stats.startups.lock().unwrap();
    assert_eq!(all.len(), 1, "one startup message reached the server");
    String::from_utf8_lossy(&all[0]).replace('\0', "|")
}

#[test]
fn a_verified_connection_with_the_ca_reaches_the_server_and_sends_the_session_options() {
    let ca = TestCa::new("ok");
    let dir = tempfile::tempdir().unwrap();
    let ca_file = ca.write_ca(dir.path());
    let server = fake(&Behaviour::Tls(ca.issue_server(&["127.0.0.1"])));
    let out = connector(Some(&ca_file), None, "127.0.0.1").run(&request("127.0.0.1", server.port));
    // The handshake completed: the fake's authentication error came back through the TLS stream, redacted.
    assert_eq!(out.stage, Some(Stage::Connect), "{out:?}");
    assert!(error(&out).contains(MARKER), "{out:?}");
    assert!(error(&out).contains("28000"), "{out:?}");
    assert!(
        error(&out).contains("<password>") && error(&out).contains("<user>"),
        "{out:?}"
    );
    assert_clean(&out, &[USER]);
    let startup = startup_text(&server);
    for want in [
        "application_name|ai-gent-dbrun|",
        "-c statement_timeout=5000",
        "-c lock_timeout=5000",
        "-c idle_in_transaction_session_timeout=60000",
        "-c default_transaction_read_only=on",
        "database|shop|",
    ] {
        assert!(startup.contains(want), "{want} missing from {startup}");
    }
}

#[test]
fn write_mode_does_not_ask_for_a_read_only_session() {
    let ca = TestCa::new("write");
    let dir = tempfile::tempdir().unwrap();
    let ca_file = ca.write_ca(dir.path());
    let server = fake(&Behaviour::Tls(ca.issue_server(&["127.0.0.1"])));
    let mut req = request("127.0.0.1", server.port);
    req.mode = Mode::Write;
    let _ = connector(Some(&ca_file), None, "127.0.0.1").run(&req);
    let startup = startup_text(&server);
    assert!(
        !startup.contains("default_transaction_read_only"),
        "{startup}"
    );
    assert!(startup.contains("-c statement_timeout=5000"), "{startup}");
}

#[test]
fn a_server_certificate_the_client_does_not_trust_is_refused_and_no_startup_is_sent() {
    // The server's CA is not the one in tls_ca_file (a CA that signs a different name is the same case).
    let trusted = TestCa::new("trusted");
    let dir = tempfile::tempdir().unwrap();
    let ca_file = trusted.write_ca(dir.path());
    let server = fake(&Behaviour::Tls(TestCa::rogue_server(&["127.0.0.1"])));
    let out = connector(Some(&ca_file), None, "127.0.0.1").run(&request("127.0.0.1", server.port));
    assert_eq!(
        (out.ok, out.stage),
        (false, Some(Stage::Connect)),
        "{out:?}"
    );
    assert!(!error(&out).contains(MARKER), "{out:?}");
    assert!(
        error(&out).contains("tls_ca_file"),
        "names the field to set: {out:?}"
    );
    assert_clean(&out, &[USER]);
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        server.stats.startups.lock().unwrap().is_empty(),
        "no startup, so no credentials sent"
    );
}

#[test]
fn without_the_ca_file_the_system_roots_do_not_trust_a_test_ca() {
    let ca = TestCa::new("unknown");
    let server = fake(&Behaviour::Tls(ca.issue_server(&["127.0.0.1"])));
    let settings = TlsSettings::from_fields(None, None, None, None).unwrap();
    let c = PostgresConnector::authorized(&settings, &production("127.0.0.1")).unwrap();
    let out = c.run(&request("127.0.0.1", server.port));
    assert_eq!(
        (out.ok, out.stage),
        (false, Some(Stage::Connect)),
        "{out:?}"
    );
    assert!(!error(&out).contains(MARKER), "{out:?}");
    assert!(error(&out).contains("tls_ca_file"), "{out:?}");
    assert_clean(&out, &[USER]);
    assert!(server.stats.startups.lock().unwrap().is_empty());
}

#[test]
fn a_certificate_for_another_name_is_refused_until_tls_server_name_says_otherwise() {
    let ca = TestCa::new("names");
    let dir = tempfile::tempdir().unwrap();
    let ca_file = ca.write_ca(dir.path());
    let server = fake(&Behaviour::Tls(ca.issue_server(&["db.example.test"])));
    let refused =
        connector(Some(&ca_file), None, "127.0.0.1").run(&request("127.0.0.1", server.port));
    assert_eq!(refused.stage, Some(Stage::Connect), "{refused:?}");
    assert!(!error(&refused).contains(MARKER), "{refused:?}");
    assert_clean(&refused, &[USER]);
    let accepted = connector(Some(&ca_file), Some("db.example.test"), "127.0.0.1")
        .run(&request("127.0.0.1", server.port));
    assert!(error(&accepted).contains(MARKER), "{accepted:?}");
}

#[test]
fn a_server_that_refuses_tls_is_a_failure_never_a_plain_text_session() {
    let ca = TestCa::new("notls");
    let dir = tempfile::tempdir().unwrap();
    let ca_file = ca.write_ca(dir.path());
    let server = fake(&Behaviour::NoTls);
    let out = connector(Some(&ca_file), None, "127.0.0.1").run(&request("127.0.0.1", server.port));
    assert_eq!(
        (out.ok, out.stage),
        (false, Some(Stage::Connect)),
        "{out:?}"
    );
    assert!(error(&out).contains("never falls back"), "{out:?}");
    assert_clean(&out, &[USER]);
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(
        server.stats.bytes_after_refusal.load(Ordering::SeqCst),
        0,
        "no startup message (no user, no database) was sent in the clear"
    );
}

#[test]
fn unverified_works_on_a_proven_local_loopback_profile_only() {
    let rogue = TestCa::rogue_server(&["127.0.0.1"]);
    let server = fake(&Behaviour::Tls(rogue));
    let unverified = TlsSettings::from_fields(Some("unverified"), None, None, None).unwrap();
    let local = ctx(ClassKind::Local, ClassKind::Local, "127.0.0.1");
    let out = PostgresConnector::authorized(&unverified, &local)
        .unwrap()
        .run(&request("127.0.0.1", server.port));
    assert!(
        error(&out).contains(MARKER),
        "an untrusted certificate is accepted when explicit: {out:?}"
    );
    // Refused, and no connection is even attempted: production, remote host, not proven, with a CA file.
    for (name, context) in [
        ("production", production("127.0.0.1")),
        (
            "declared production, proven local",
            ctx(ClassKind::Production, ClassKind::Local, "127.0.0.1"),
        ),
        (
            "remote host",
            ctx(ClassKind::Local, ClassKind::Local, "db.internal"),
        ),
        (
            "staging",
            ctx(ClassKind::Staging, ClassKind::Staging, "127.0.0.1"),
        ),
    ] {
        let err = PostgresConnector::authorized(&unverified, &context).unwrap_err();
        assert_eq!(err.field(), "tls_mode", "{name}");
    }
    assert_eq!(
        server.stats.accepted.load(Ordering::SeqCst),
        1,
        "only the allowed case connected"
    );
}

#[test]
fn a_bad_tls_mode_is_refused_naming_the_field() {
    for bad in [
        "disable",
        "prefer",
        "require",
        "allow",
        "verify-full",
        "off",
        "UNVERIFIED ; DROP",
    ] {
        let err = TlsSettings::from_fields(Some(bad), None, None, None).unwrap_err();
        assert_eq!(err.field(), "tls_mode", "{bad}");
        assert!(!err.to_string().contains("DROP"), "no value echoed: {err}");
    }
}

#[test]
fn a_ca_file_that_is_a_private_key_is_refused_before_any_connection() {
    let ca = TestCa::new("key");
    let cert = ca.issue_server(&["127.0.0.1"]);
    let dir = tempfile::tempdir().unwrap();
    let key_file = dir.path().join("key.pem");
    std::fs::write(&key_file, &cert.key_pem).unwrap();
    let server = fake(&Behaviour::Tls(cert));
    let out = connector(Some(&key_file), None, "127.0.0.1").run(&request("127.0.0.1", server.port));
    assert_eq!(
        (out.ok, out.stage),
        (false, Some(Stage::Connect)),
        "{out:?}"
    );
    assert!(
        error(&out).contains("tls_ca_file") && error(&out).contains("private key"),
        "{out:?}"
    );
    assert_clean(&out, &[USER]);
    assert!(
        !out.to_line().contains(key_file.to_str().unwrap_or("?")) || cfg!(windows),
        "{out:?}"
    );
    assert_eq!(
        server.stats.accepted.load(Ordering::SeqCst),
        0,
        "nothing connected"
    );
}

#[test]
fn a_request_for_another_host_than_the_authorized_one_is_refused() {
    let ca = TestCa::new("host");
    let dir = tempfile::tempdir().unwrap();
    let ca_file = ca.write_ca(dir.path());
    let server = fake(&Behaviour::Tls(ca.issue_server(&["127.0.0.1"])));
    // Authorized for db.internal, the request names 127.0.0.1: the check must not be bypassable.
    let c = connector(Some(&ca_file), None, "db.internal");
    let out = c.run(&request("127.0.0.1", server.port));
    assert_eq!(
        (out.ok, out.stage),
        (false, Some(Stage::Connect)),
        "{out:?}"
    );
    assert!(error(&out).contains("another host"), "{out:?}");
    assert_clean(&out, &["db.internal"]);
    assert_eq!(server.stats.accepted.load(Ordering::SeqCst), 0);
}

#[test]
fn bad_connection_facts_are_refused_without_echoing_them() {
    let ca = TestCa::new("facts");
    let dir = tempfile::tempdir().unwrap();
    let ca_file = ca.write_ca(dir.path());
    let server = fake(&Behaviour::Tls(ca.issue_server(&["127.0.0.1"])));
    let c = connector(Some(&ca_file), None, "127.0.0.1");
    for port in ["abc", "0", "70000", "-5", "5432; DROP", "1.5"] {
        let mut req = request("127.0.0.1", server.port);
        req.conn.port = Some(port.into());
        let out = c.run(&req);
        assert_eq!(
            (out.ok, out.stage),
            (false, Some(Stage::Connect)),
            "{port}: {out:?}"
        );
        assert!(error(&out).contains("invalid port"), "{port}: {out:?}");
        assert!(!out.to_line().contains("DROP"), "{port}: {out:?}");
        assert_clean(&out, &[]);
    }
    let mut no_host = request("127.0.0.1", server.port);
    no_host.conn.host = None;
    let out = c.run(&no_host);
    assert!(error(&out).contains("missing conn.host"), "{out:?}");
    assert_eq!(server.stats.accepted.load(Ordering::SeqCst), 0);
}

#[test]
fn a_server_that_accepts_and_goes_silent_is_given_up_on_at_the_deadline() {
    let ca = TestCa::new("silent");
    let dir = tempfile::tempdir().unwrap();
    let ca_file = ca.write_ca(dir.path());
    let server = fake(&Behaviour::Silent);
    let c = connector(Some(&ca_file), None, "127.0.0.1")
        .with_limits(Duration::from_millis(200), Duration::from_millis(200));
    let mut req = request("127.0.0.1", server.port);
    req.timeout = 0.1;
    let started = Instant::now();
    let out = c.run(&req);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "bounded: {:?}",
        started.elapsed()
    );
    assert_eq!(
        (out.ok, out.stage),
        (false, Some(Stage::Execute)),
        "{out:?}"
    );
    assert!(error(&out).contains("deadline"), "{out:?}");
    assert_clean(&out, &[USER]);
}

#[test]
fn a_refused_connection_is_a_redacted_connect_failure() {
    // Port 1 on loopback: nothing listens. The user and password must not appear in the error.
    let ca = TestCa::new("refused");
    let dir = tempfile::tempdir().unwrap();
    let ca_file = ca.write_ca(dir.path());
    let mut req = request("127.0.0.1", 1);
    req.mode = Mode::Test;
    req.timeout = 2.0;
    let out = connector(Some(&ca_file), None, "127.0.0.1").run(&req);
    assert_eq!(
        (out.ok, out.stage),
        (false, Some(Stage::Connect)),
        "{out:?}"
    );
    assert_clean(&out, &[USER]);
}

#[test]
fn the_engine_name_is_postgresql_only() {
    let ca = TestCa::new("engine");
    let dir = tempfile::tempdir().unwrap();
    let ca_file = ca.write_ca(dir.path());
    let c = connector(Some(&ca_file), None, "127.0.0.1");
    assert_eq!(c.engines(), &["postgresql"]);
    let mut req = request("127.0.0.1", 1);
    req.engine = "mysql".into();
    let out = c.run(&req);
    assert_eq!(out.error.as_deref(), Some("unsupported engine mysql"));
}

// ---- A real server -------------------------------------------------------------------------------
//
// AI_GENT_TEST_PG_HOST, _PORT, _USER, _PASSWORD, _DATABASE, _CA_FILE (and optionally _SERVER_NAME)
// name a PostgreSQL server that offers TLS with a certificate the CA file verifies. Run with
// `cargo test -p ai-gent-dbrun --features postgres -- --ignored real_server`.

fn real(mode: Mode, statements: &[&str]) -> (Request, PostgresConnector) {
    let env = |k: &str| std::env::var(format!("AI_GENT_TEST_PG_{k}")).ok();
    let host = env("HOST").expect("AI_GENT_TEST_PG_HOST");
    let conn = ConnFacts {
        host: Some(host.clone()),
        port: env("PORT"),
        database: env("DATABASE"),
        user: env("USER"),
        password: env("PASSWORD"),
        ..ConnFacts::default()
    };
    let req = Request::new(
        "postgresql",
        conn,
        mode,
        statements.iter().map(|s| (*s).to_string()).collect(),
    );
    let settings = TlsSettings::from_fields(
        None,
        env("CA_FILE").as_deref(),
        env("SERVER_NAME").as_deref(),
        None,
    )
    .unwrap();
    let c = PostgresConnector::authorized(&settings, &production(&host)).unwrap();
    (req, c)
}

#[test]
#[ignore = "needs AI_GENT_TEST_PG_* (a real PostgreSQL server with TLS)"]
fn real_server_read_mode_refuses_a_write_with_25006_and_a_second_statement() {
    let (req, c) = real(Mode::Read, &["CREATE TABLE ai_gent_ro_probe (a int)"]);
    let out = c.run(&req);
    assert_eq!(
        (out.ok, out.stage),
        (false, Some(Stage::Execute)),
        "{out:?}"
    );
    assert!(error(&out).contains("25006"), "{out:?}");
    let (req, c) = real(Mode::Read, &["SELECT 1; SELECT 2"]);
    let out = c.run(&req);
    assert_eq!(out.stage, Some(Stage::Execute), "{out:?}");
    assert!(error(&out).contains("42601"), "{out:?}");
    let (req, c) = real(
        Mode::Read,
        &[
            "SET default_transaction_read_only = off",
            "CREATE TABLE ai_gent_ro_probe (a int)",
        ],
    );
    let out = c.run(&req);
    assert!(
        error(&out).contains("25006"),
        "the transaction stays read only: {out:?}"
    );
}

#[test]
#[ignore = "needs AI_GENT_TEST_PG_* (a real PostgreSQL server with TLS)"]
fn real_server_counts_come_from_the_server() {
    let (mut req, c) = real(
        Mode::Write,
        &[
            "CREATE TEMP TABLE t (id int)",
            "/* c */ INSERT INTO t VALUES (1), (2), (3)",
            "WITH d AS (DELETE FROM t WHERE id < 3 RETURNING id) SELECT id FROM d",
            "UPDATE t SET id = id RETURNING id",
            "SHOW TimeZone",
        ],
    );
    req.expect = vec![Some(0), Some(3), Some(2), Some(1), Some(0)];
    req.max_rows = 1;
    let out = c.run(&req);
    assert!(out.ok, "{out:?}");
    let r = out.results.unwrap();
    assert_eq!(
        r.iter().map(|e| e.rowcount).collect::<Vec<_>>(),
        vec![0, 3, 2, 1, 0]
    );
    assert_eq!(
        r[2].rows.as_ref().map(Vec::len),
        Some(1),
        "max_rows applies in write mode"
    );
    assert_eq!(r[2].truncated, Some(false), "truncated is read mode only");
}
