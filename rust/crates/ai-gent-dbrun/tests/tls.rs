//! TLS profile model and rustls builder (spec 043 T007, FR-015..FR-017).
//!
//! Every refusal test names the input it feeds, asserts the field the error names, and asserts that the
//! message does not echo the input (a path, a name, a key). The handshake tests run a real rustls server
//! on loopback with a certificate from a throwaway CA.
#![allow(
    clippy::many_single_char_names,
    clippy::unused_self,
    clippy::needless_pass_by_value,
    clippy::single_match_else,
    clippy::too_many_lines
)]
use ai_gent_dbrun::core::tls::{
    Authorized, ClassKind, TlsContext, TlsMode, TlsSettings, UNVERIFIED_WARNING, client_config,
    client_config_with, is_loopback_host, server_name, testca::TestCa,
};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

fn kind(s: &str) -> ClassKind {
    match s {
        "production" => ClassKind::Production,
        "staging" => ClassKind::Staging,
        "development" => ClassKind::Development,
        "local" => ClassKind::Local,
        other => panic!("unknown class {other}"),
    }
}

fn ctx<'a>(declared: &str, proven: &str, host: &'a str) -> TlsContext<'a> {
    TlsContext {
        declared: kind(declared),
        proven: kind(proven),
        host,
    }
}

fn unverified() -> TlsSettings {
    TlsSettings::from_fields(Some("unverified"), None, None, None).unwrap()
}

fn verified_with(ca: &Path) -> Authorized {
    TlsSettings::from_fields(None, Some(ca.to_str().unwrap()), None, None)
        .unwrap()
        .authorize(&ctx("production", "production", "db.internal"))
        .unwrap()
}

#[test]
fn tls_default_is_verify_with_no_ca_and_no_name() {
    let s = TlsSettings::from_fields(None, None, None, None).unwrap();
    assert_eq!(s.mode, TlsMode::Verify);
    assert!(s.ca_file.is_none() && s.server_name.is_none());
    // An empty value is "not set", never a weaker mode.
    let e = TlsSettings::from_fields(Some(" "), Some(""), Some(""), None).unwrap();
    assert_eq!(e, s);
}

#[test]
fn tls_mode_values_other_than_verify_or_unverified_are_refused() {
    // Libpq-style names and a secret-looking typo: none may fall back to a weaker mode.
    for bad in [
        "disable",
        "require",
        "verify-full",
        "prefer",
        "false",
        "s3cr3t-mode",
    ] {
        let e = TlsSettings::from_fields(Some(bad), None, None, None).unwrap_err();
        assert_eq!(e.field(), "tls_mode", "{bad}");
        assert!(!e.to_string().contains(bad), "the message echoed {bad}");
    }
}

#[test]
fn tls_ca_file_must_be_absolute_and_not_combined_with_unverified() {
    for bad in [
        "certs/ca.pem",
        "./ca.pem",
        "ca.pem",
        "../ca.pem",
        "C:ca.pem",
    ] {
        let e = TlsSettings::from_fields(None, Some(bad), None, None).unwrap_err();
        assert_eq!(e.field(), "tls_ca_file", "{bad}");
        assert!(!e.to_string().contains(bad));
    }
    let e = TlsSettings::from_fields(None, Some("/ca\0.pem"), None, None).unwrap_err();
    assert_eq!(e.field(), "tls_ca_file");
    let e = TlsSettings::from_fields(None, Some("~/ca.pem"), None, None).unwrap_err();
    assert_eq!(e.field(), "tls_ca_file");
    let ok = TlsSettings::from_fields(None, Some("~/ca.pem"), None, Some(Path::new("/h"))).unwrap();
    assert_eq!(ok.ca_file.unwrap(), Path::new("/h").join("ca.pem"));
    let e =
        TlsSettings::from_fields(Some("unverified"), Some("/etc/ca.pem"), None, None).unwrap_err();
    assert_eq!(e.field(), "tls_ca_file");
    assert!(!e.to_string().contains("/etc/ca.pem"));
}

#[test]
fn tls_server_name_refuses_what_is_not_a_host_name() {
    let long = "a".repeat(300);
    for bad in [
        "a b",
        "x;y",
        "(host)",
        "h=1",
        "-lead.example",
        "a..b",
        "q\"uote",
        &long,
    ] {
        let e = TlsSettings::from_fields(None, None, Some(bad), None).unwrap_err();
        assert_eq!(e.field(), "tls_server_name", "{bad}");
        assert!(!e.to_string().contains(bad));
    }
    for ok in [
        "db.internal",
        "localhost",
        "10.1.2.3",
        "::1",
        "db_1.example.com",
    ] {
        assert!(
            TlsSettings::from_fields(None, None, Some(ok), None).is_ok(),
            "{ok}"
        );
    }
}

#[test]
fn tls_loopback_hosts() {
    for h in [
        "127.0.0.1",
        "127.9.9.9",
        "::1",
        "[::1]",
        "LOCALHOST",
        "::ffff:127.0.0.1",
    ] {
        assert!(is_loopback_host(h), "{h}");
    }
    for h in [
        "",
        " ",
        "0.0.0.0",
        "10.0.0.1",
        "db.internal",
        "127.0.0.1.evil.com",
        "localhost.evil.com",
        "::",
        "128.0.0.1",
    ] {
        assert!(!is_loopback_host(h), "{h}");
    }
}

#[test]
fn tls_unverified_policy_cases_from_the_fixture() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/db-tls/cases.json");
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let cases = v["authorize"].as_array().unwrap();
    assert!(cases.len() >= 20);
    for c in cases {
        let name = c["name"].as_str().unwrap();
        let s = TlsSettings::from_fields(c["mode"].as_str(), None, None, None).unwrap();
        let r = s.authorize(&ctx(
            c["declared"].as_str().unwrap(),
            c["proven"].as_str().unwrap(),
            c["host"].as_str().unwrap(),
        ));
        match c["expect"].as_str().unwrap() {
            "ok" => assert!(r.is_ok(), "{name}: {r:?}"),
            _ => {
                let e = r.expect_err(name);
                assert_eq!(e.field(), c["field"].as_str().unwrap(), "{name}");
                assert!(
                    e.to_string().contains(c["says"].as_str().unwrap()),
                    "{name}: {e}"
                );
                assert!(
                    e.to_string().starts_with("tls_mode:"),
                    "{name}: names the field first"
                );
            }
        }
    }
}

#[test]
fn tls_production_refusal_names_the_field_and_the_way_out() {
    let e = unverified()
        .authorize(&ctx("production", "production", "127.0.0.1"))
        .unwrap_err();
    assert_eq!(e.field(), "tls_mode");
    let m = e.to_string();
    assert!(m.contains("production") && m.contains("tls_ca_file"), "{m}");
}

// ---- handshake tests -------------------------------------------------------------------------

fn server(ca: &TestCa, names: &[&str], connections: usize) -> u16 {
    let cert = ca.issue_server(names);
    let chain = vec![CertificateDer::from_pem_slice(cert.cert_pem.as_bytes()).unwrap()];
    let key = PrivateKeyDer::from_pem_slice(cert.key_pem.as_bytes()).unwrap();
    let cfg = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(chain, key)
    .unwrap();
    let cfg = Arc::new(cfg);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for _ in 0..connections {
            let Ok((tcp, _)) = listener.accept() else {
                return;
            };
            let _ = tcp.set_read_timeout(Some(Duration::from_secs(5)));
            let _ = tcp.set_write_timeout(Some(Duration::from_secs(5)));
            let conn = rustls::ServerConnection::new(Arc::clone(&cfg)).unwrap();
            let mut tls = rustls::StreamOwned::new(conn, tcp);
            let mut b = [0u8; 1];
            if tls.read_exact(&mut b).is_ok() {
                let _ = tls.write_all(&b);
            }
        }
    });
    port
}

/// One connection through `config`; Ok when the handshake and one echo succeed.
fn connect(config: rustls::ClientConfig, port: u16, name: &str) -> Result<(), String> {
    let tcp = TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    tcp.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    let sn = server_name(&TlsSettings::default(), name).map_err(|e| e.to_string())?;
    connect_as(config, tcp, sn)
}

fn connect_as(
    config: rustls::ClientConfig,
    tcp: TcpStream,
    sn: rustls::pki_types::ServerName<'static>,
) -> Result<(), String> {
    let conn = rustls::ClientConnection::new(Arc::new(config), sn).map_err(|e| e.to_string())?;
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    tls.write_all(b"x").map_err(|e| e.to_string())?;
    let mut b = [0u8; 1];
    tls.read_exact(&mut b).map_err(|e| e.to_string())?;
    Ok(())
}

#[test]
fn tls_verified_with_the_ca_file_connects() {
    let dir = tempfile::tempdir().unwrap();
    let ca = TestCa::new("ok");
    let port = server(&ca, &["localhost"], 1);
    let cfg = client_config(&verified_with(&ca.write_ca(dir.path()))).unwrap();
    assert_eq!(connect(cfg, port, "localhost"), Ok(()));
}

#[test]
fn tls_rejected_without_the_ca_file() {
    // Default settings: system roots (or, on a machine with none, a refusal naming tls_ca_file). Either
    // way the throwaway CA is not trusted and the connection never carries data.
    let ca = TestCa::new("not-in-system-roots");
    let port = server(&ca, &["localhost"], 1);
    let auth = TlsSettings::default()
        .authorize(&ctx("production", "production", "localhost"))
        .unwrap();
    match client_config(&auth) {
        Ok(cfg) => assert!(connect(cfg, port, "localhost").is_err()),
        Err(e) => assert_eq!(e.field(), "tls_ca_file"),
    }
}

#[test]
fn tls_rejected_when_another_ca_signed_the_certificate() {
    let dir = tempfile::tempdir().unwrap();
    let trusted = TestCa::new("trusted");
    let port = server(&TestCa::new("rogue"), &["localhost"], 1);
    let cfg = client_config(&verified_with(&trusted.write_ca(dir.path()))).unwrap();
    assert!(connect(cfg, port, "localhost").is_err());
}

#[test]
fn tls_rejected_when_the_name_does_not_match_and_accepted_with_the_override() {
    let dir = tempfile::tempdir().unwrap();
    let ca = TestCa::new("names");
    let port = server(&ca, &["db.example.test"], 2);
    let path = ca.write_ca(dir.path());
    // host 127.0.0.1 is not in the certificate: refused.
    let cfg = client_config(&verified_with(&path)).unwrap();
    assert!(connect(cfg, port, "127.0.0.1").is_err());
    // tls_server_name names what the certificate holds: accepted.
    let s = TlsSettings::from_fields(
        None,
        Some(path.to_str().unwrap()),
        Some("db.example.test"),
        None,
    )
    .unwrap();
    let auth = s
        .authorize(&ctx("production", "production", "127.0.0.1"))
        .unwrap();
    let sn = server_name(auth.settings(), "127.0.0.1").unwrap();
    let tcp = TcpStream::connect(("127.0.0.1", port)).unwrap();
    tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    assert_eq!(connect_as(client_config(&auth).unwrap(), tcp, sn), Ok(()));
}

#[test]
fn tls_unverified_connects_and_warns_on_every_connection() {
    let ca = TestCa::new("unverified");
    let port = server(&ca, &["localhost"], 2);
    let auth = unverified()
        .authorize(&ctx("local", "local", "127.0.0.1"))
        .unwrap();
    let warned = Arc::new(AtomicUsize::new(0));
    let w = Arc::clone(&warned);
    let warn: ai_gent_dbrun::core::tls::WarnFn = Arc::new(move |m: &str| {
        assert_eq!(m, UNVERIFIED_WARNING);
        assert!(
            !m.contains("127.0.0.1") && !m.contains("localhost"),
            "no host in the warning"
        );
        w.fetch_add(1, Ordering::SeqCst);
    });
    // The same config reused for two connections: one warning each.
    let cfg = client_config_with(&auth, warn).unwrap();
    assert_eq!(connect(cfg.clone(), port, "localhost"), Ok(()));
    assert_eq!(connect(cfg, port, "localhost"), Ok(()));
    assert_eq!(warned.load(Ordering::SeqCst), 2);
}

#[test]
fn tls_unverified_cannot_be_built_without_authorization_on_production() {
    // The builder takes only an Authorized, and authorize refuses: there is no way to the verifier.
    let r = unverified().authorize(&ctx("production", "production", "127.0.0.1"));
    assert!(r.is_err());
}

#[test]
fn tls_ca_file_refusals_name_the_field_and_echo_no_content() {
    let dir = tempfile::tempdir().unwrap();
    let ca = TestCa::new("files");
    let key = ca.issue_server(&["localhost"]).key_pem;
    let write = |name: &str, bytes: &[u8]| {
        let p = dir.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    };
    let cases: Vec<(&str, std::path::PathBuf)> = vec![
        ("missing", dir.path().join("nope.pem")),
        ("empty", write("empty.pem", b"")),
        ("garbage", write("garbage.pem", b"s3cr3t-garbage not pem")),
        (
            "private key inside",
            write("key.pem", format!("{}{}", ca.ca_pem(), key).as_bytes()),
        ),
        ("a directory", dir.path().to_path_buf()),
        (
            "over the size cap",
            write("big.pem", &vec![b'A'; 1024 * 1024 + 1]),
        ),
        (
            "broken pem body",
            write(
                "broken.pem",
                b"-----BEGIN CERTIFICATE-----\n!!!notbase64!!!\n-----END CERTIFICATE-----\n",
            ),
        ),
    ];
    for (what, path) in cases {
        let s = TlsSettings::from_fields(None, Some(path.to_str().unwrap()), None, None).unwrap();
        let auth = s.authorize(&ctx("production", "production", "h")).unwrap();
        let e = client_config(&auth).expect_err(what);
        assert_eq!(e.field(), "tls_ca_file", "{what}");
        let m = e.to_string();
        assert!(!m.contains("s3cr3t-garbage"), "{what}: echoed the content");
        assert!(
            !m.contains("PRIVATE KEY") || m.contains("holds a private key"),
            "{what}"
        );
        assert!(!m.contains(&key[30..60]), "{what}: echoed key material");
    }
}

#[test]
fn tls_ca_file_with_several_certificates_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (TestCa::new("a"), TestCa::new("b"));
    let p = dir.path().join("two.pem");
    std::fs::write(&p, format!("{}\n{}", a.ca_pem(), b.ca_pem())).unwrap();
    let port = server(&b, &["localhost"], 1);
    assert_eq!(
        connect(
            client_config(&verified_with(&p)).unwrap(),
            port,
            "localhost"
        ),
        Ok(())
    );
}

#[test]
fn tls_test_ca_issues_distinct_authorities() {
    let (a, b) = (TestCa::new("x"), TestCa::new("x"));
    assert_ne!(a.ca_pem(), b.ca_pem());
    assert!(a.ca_pem().starts_with("-----BEGIN CERTIFICATE-----"));
}

#[test]
fn tls_authorized_carries_the_host_it_was_checked_against() {
    let auth = unverified()
        .authorize(&ctx("local", "local", "127.0.0.1"))
        .unwrap();
    assert_eq!(auth.host(), "127.0.0.1");
    let auth = TlsSettings::default()
        .authorize(&ctx("production", "production", "db.internal"))
        .unwrap();
    assert_eq!(auth.host(), "db.internal");
}

#[test]
fn tls_authorize_refuses_unverified_with_a_ca_file_even_when_built_by_hand() {
    // The fields are public, so from_fields' check can be bypassed; authorize repeats it.
    let s = TlsSettings {
        mode: TlsMode::Unverified,
        ca_file: Some("/etc/ca.pem".into()),
        server_name: None,
    };
    let e = s
        .authorize(&ctx("local", "local", "127.0.0.1"))
        .unwrap_err();
    assert_eq!(e.field(), "tls_ca_file");
    assert!(!e.to_string().contains("/etc/ca.pem"));
}

#[test]
fn tls_ca_file_accepts_a_windows_style_home_prefix() {
    let ok =
        TlsSettings::from_fields(None, Some("~\\ca.pem"), None, Some(Path::new("/h"))).unwrap();
    assert_eq!(ok.ca_file.unwrap(), Path::new("/h").join("ca.pem"));
}

#[test]
#[cfg(unix)]
fn tls_a_fifo_as_the_ca_file_is_refused_without_blocking() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("ca.pem");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let auth = verified_with(&fifo);
    let e = client_config(&auth).unwrap_err();
    assert_eq!(e.field(), "tls_ca_file");
    assert!(e.to_string().contains("not a regular file"), "{e}");
}
