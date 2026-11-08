//! Profiles, classification and secrets (spec 043 T010), one test per behavior of `dbrun.py` it replaces
//! (`ProfilesTest`, `PassTest`, `TomlTest`, `MigrateAllTest` of `plugins/db/scripts/test_dbrun.py`) plus the
//! refusals the port adds. Values are fake; `pass` is a shell stand-in (Unix only), the container engine a
//! fake. Every refusal test feeds a named input, asserts the refusal, and asserts that no secret and no
//! host appears in the message.
#![allow(
    clippy::many_single_char_names,
    clippy::unused_self,
    clippy::needless_pass_by_value,
    clippy::single_match_else,
    clippy::too_many_lines
)]
use ai_gent_core::env::Env;
use ai_gent_dbrun::profiles::container::{
    ContainerEngine, EngineOut, Route, Via, classify as classify_with, detect_route,
    engine_endpoint_local, image_engine, valid_container_name,
};
use ai_gent_dbrun::profiles::{
    Class, Ctx, SecretSource, all_profiles, classify, export_env, files, get_profile, oracle_dsn,
    rows, rows_json, secrets, user_profiles,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

type Handler = Box<dyn Fn(&[&str]) -> EngineOut + Send + Sync>;

struct FakeEngine {
    program: &'static str,
    handler: Handler,
    calls: Mutex<Vec<Vec<String>>>,
}

impl ContainerEngine for FakeEngine {
    fn program(&self) -> String {
        self.program.to_string()
    }
    fn run(&self, args: &[&str]) -> EngineOut {
        self.calls
            .lock()
            .unwrap()
            .push(args.iter().map(|s| (*s).to_string()).collect());
        (self.handler)(args)
    }
}

fn out(stdout: &str) -> EngineOut {
    EngineOut {
        code: 0,
        stdout: stdout.to_string(),
    }
}

fn fail() -> EngineOut {
    EngineOut {
        code: 1,
        stdout: String::new(),
    }
}

fn fake(
    program: &'static str,
    h: impl Fn(&[&str]) -> EngineOut + Send + Sync + 'static,
) -> Arc<FakeEngine> {
    Arc::new(FakeEngine {
        program,
        handler: Box::new(h),
        calls: Mutex::new(vec![]),
    })
}

const ENV_NAME: &str = "connections.env";
const TOML_NAME: &str = "connections.toml";

struct Sandbox {
    _tmp: tempfile::TempDir,
    base: PathBuf,
    repo: PathBuf,
    outside: PathBuf,
    config: PathBuf,
    store: PathBuf,
    extra: Vec<(String, String)>,
}

#[cfg(unix)]
const FAKE_PASS: &str = r#"#!/bin/sh
# A stand-in for pass: entries are plain files named <entry>.gpg in the store.
store="$PASSWORD_STORE_DIR"
case "$1" in
  show) [ -f "$store/$2.gpg" ] || { echo "Error: $2 is not in the password store." >&2; exit 1; }
        cat "$store/$2.gpg" ;;
  insert) e="$3"; mkdir -p "$(dirname "$store/$e")"; cat > "$store/$e.gpg" ;;
  *) exit 2 ;;
esac
"#;

impl Sandbox {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().canonicalize().unwrap();
        let (repo, outside, config, store) = (
            base.join("repo"),
            base.join("elsewhere"),
            base.join("config"),
            base.join("store"),
        );
        for d in [
            &repo,
            &outside,
            &config,
            &store.join("ai-gent/db"),
            &base.join("bin"),
        ] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(repo.join("app.db"), b"").unwrap();
        std::fs::write(outside.join("prod.db"), b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let f = base.join("bin/pass");
            std::fs::write(&f, FAKE_PASS).unwrap();
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let s = Self {
            _tmp: tmp,
            base,
            repo,
            outside,
            config,
            store,
            extra: vec![],
        };
        s.write_profiles(&format!(
            "LOCALDB_ENGINE=sqlite\nLOCALDB_PATH={}\nPRODDB_ENGINE=sqlite\nPRODDB_CLASS=production\nPRODDB_PATH={}\nNOCLASS_ENGINE=sqlite\nNOCLASS_PATH={}\n",
            s.repo.join("app.db").display(),
            s.outside.join("prod.db").display(),
            s.outside.join("prod.db").display()
        ));
        s
    }

    fn write_private(&self, path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
    }

    fn env_path(&self) -> PathBuf {
        self.config.join(ENV_NAME)
    }

    fn write_profiles(&self, text: &str) {
        self.write_private(&self.env_path(), text);
    }

    fn write_toml(&self, text: &str) -> PathBuf {
        let p = self.config.join(TOML_NAME);
        self.write_private(&p, text);
        p
    }

    fn entry(&self, name: &str, text: &str) {
        let f = self.store.join(format!("{name}.gpg"));
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, text).unwrap();
    }

    fn with(mut self, k: &str, v: &str) -> Self {
        self.extra.push((k.into(), v.into()));
        self
    }

    fn ctx_with(&self, engine: Option<Arc<dyn ContainerEngine>>) -> Ctx {
        let path = format!("{}:/usr/bin:/bin", self.base.join("bin").display());
        let mut pairs: Vec<(String, String)> = vec![
            (
                "AI_GENT_DB_CONFIG".into(),
                self.config.display().to_string(),
            ),
            (
                "PASSWORD_STORE_DIR".into(),
                self.store.display().to_string(),
            ),
            ("PATH".into(), path),
            ("HOME".into(), self.base.display().to_string()),
        ];
        pairs.extend(self.extra.clone());
        let env = Env::from_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        Ctx::new(env, self.repo.clone(), engine)
    }

    fn ctx(&self) -> Ctx {
        self.ctx_with(None)
    }

    fn env_text(&self) -> String {
        std::fs::read_to_string(self.env_path()).unwrap()
    }
}

fn assert_clean(msg: &str, secrets: &[&str]) {
    for s in secrets {
        assert!(!msg.contains(s), "the output holds {s}: {msg}");
    }
}

// ---------------------------------------------------------------- ProfilesTest

#[test]
fn profiles_parse_and_classify() {
    let s = Sandbox::new();
    let ctx = s.ctx();
    let loaded = user_profiles(&ctx).unwrap();
    let names: Vec<_> = loaded.profiles.keys().cloned().collect();
    assert_eq!(names, ["LOCALDB", "NOCLASS", "PRODDB"]);
    assert_eq!(
        loaded.profiles["NOCLASS"].class,
        Class::Production,
        "unclassified = production"
    );
    let local = classify(&ctx, &loaded.profiles["LOCALDB"]).unwrap();
    assert_eq!(
        (local.class, local.writable, local.via),
        (Class::Local, true, Via::File)
    );
    let prod = classify(&ctx, &loaded.profiles["PRODDB"]).unwrap();
    assert_eq!((prod.class, prod.writable), (Class::Production, false));
}

#[test]
fn profiles_error_for_a_missing_sqlite_file_is_exit_4_and_redactable() {
    let s = Sandbox::new();
    s.write_profiles("GONE_ENGINE=sqlite\nGONE_CLASS=development\nGONE_PATH=/nowhere/private-host.db\nGONE_HOST=private-host.internal\nGONE_USER=svc_reader\n");
    let ctx = s.ctx();
    let p = &user_profiles(&ctx).unwrap().profiles["GONE"];
    let e = classify(&ctx, p).unwrap_err();
    assert_eq!(e.code, 4);
    assert_clean(&e.to_string(), &["svc_reader", "private-host.internal"]);
    let pairs = p.redaction_pairs();
    assert!(pairs.contains(&("private-host.internal".to_string(), "<host>")));
    assert!(pairs.contains(&("svc_reader".to_string(), "<user>")));
}

#[test]
fn profiles_redaction_pairs_skip_short_and_loopback_values() {
    let s = Sandbox::new();
    s.write_profiles("A_ENGINE=postgresql\nA_HOST=127.0.0.1\nA_USER=a\nA_PASSWORD=12\nB_ENGINE=postgresql\nB_HOST=localhost\n");
    let l = user_profiles(&s.ctx()).unwrap();
    let a = l.profiles["A"].redaction_pairs();
    assert_eq!(
        a,
        vec![("12".to_string(), "<password>")],
        "numeric password kept, 1-char user and loopback host skipped"
    );
    assert!(l.profiles["B"].redaction_pairs().is_empty());
}

#[test]
fn profiles_listing_never_shows_secrets() {
    let s = Sandbox::new();
    s.write_profiles("R_ENGINE=postgresql\nR_CLASS=development\nR_HOST=db.internal\nR_USER=app\nR_PASSWORD=s3cr3t-value\n");
    let l = user_profiles(&s.ctx()).unwrap();
    let json_out = rows_json(&rows(&l.profiles));
    assert_clean(&json_out, &["s3cr3t-value", "db.internal", "\"app\""]);
    let v: Value = serde_json::from_str(&json_out).unwrap();
    assert_eq!(
        (v[0]["class"].as_str(), v[0]["password"].as_str()),
        (Some("development"), Some("file"))
    );
}

#[test]
fn profiles_declared_local_without_proof_is_production() {
    let s = Sandbox::new();
    s.write_profiles("L_ENGINE=postgresql\nL_CLASS=local\nL_HOST=127.0.0.1\nL_CONTAINER=db\n");
    let ctx = s.ctx(); // no container engine
    let c = classify(&ctx, &user_profiles(&ctx).unwrap().profiles["L"]).unwrap();
    assert_eq!((c.class, c.writable), (Class::Production, false));
    assert!(c.evidence.iter().any(|e| e.starts_with("NOT PROVEN LOCAL")));
}

#[test]
fn profiles_unknown_or_odd_classes_become_production() {
    let s = Sandbox::new();
    s.write_profiles("A_ENGINE=sqlite\nA_CLASS=prod-ish\nB_ENGINE=sqlite\nB_CLASS=\nC_ENGINE=sqlite\nC_CLASS=LOCAL\nD_ENGINE=sqlite\nD_CLASS=Staging\n");
    let p = user_profiles(&s.ctx()).unwrap().profiles;
    assert_eq!(p["A"].class, Class::Production);
    assert_eq!(p["B"].class, Class::Production);
    assert_eq!(p["C"].class, Class::Local);
    assert_eq!(p["D"].class, Class::Staging);
}

// ---------------------------------------------------------------- .env format

#[test]
fn env_format_export_quotes_comments_and_last_value_wins() {
    let s = Sandbox::new();
    s.write_profiles(
        "# a comment\n\nexport A_ENGINE=postgresql\nA_HOST='db one'\nA_USER=\"u # not a comment\"\nA_DATABASE=plain # trailing comment\nA_PORT=1\nA_PORT=5433\nbadkey_ENGINE=x\nlower_case_ENGINE=y\nX_UNKNOWN=1\n",
    );
    let p = user_profiles(&s.ctx()).unwrap().profiles;
    assert_eq!(
        p.keys().cloned().collect::<Vec<_>>(),
        ["A"],
        "invalid names and unknown suffixes are ignored"
    );
    let a = &p["A"];
    assert_eq!(a.host.as_deref(), Some("db one"));
    assert_eq!(a.user.as_deref(), Some("u # not a comment"));
    assert_eq!(a.database.as_deref(), Some("plain"));
    assert_eq!(a.port.as_deref(), Some("5433"));
}

#[test]
fn env_format_a_line_without_equals_is_refused_by_line_number_never_its_text() {
    for (text, line) in [
        ("A_ENGINE=sqlite\ns3cr3t-no-equals\n", 2),
        ("1BAD=x\n", 1),
        ("A B=x\n", 1),
    ] {
        let s = Sandbox::new();
        s.write_profiles(text);
        let e = user_profiles(&s.ctx()).unwrap_err();
        assert!(
            e.to_string()
                .contains(&format!(":{line}: not a KEY=VALUE line")),
            "{e}"
        );
        assert_clean(&e.to_string(), &["s3cr3t-no-equals"]);
    }
}

#[test]
fn env_format_crlf_counts_one_line_and_oversized_or_binary_files_are_refused() {
    let s = Sandbox::new();
    s.write_profiles("A_ENGINE=sqlite\r\nbad line\r\n");
    assert!(
        user_profiles(&s.ctx())
            .unwrap_err()
            .to_string()
            .contains(":2:")
    );
    s.write_profiles(&format!("A_ENGINE=sqlite\n# {}\n", "x".repeat(1024 * 1024)));
    assert!(
        user_profiles(&s.ctx())
            .unwrap_err()
            .to_string()
            .contains("larger than")
    );
    std::fs::write(s.env_path(), b"A_ENGINE=sq\0lite\n").unwrap();
    assert!(
        user_profiles(&s.ctx())
            .unwrap_err()
            .to_string()
            .contains("NUL")
    );
    std::fs::write(s.env_path(), [0xff, 0xfe, b'=', b'x']).unwrap();
    assert!(
        user_profiles(&s.ctx())
            .unwrap_err()
            .to_string()
            .contains("UTF-8")
    );
}

#[test]
fn env_format_tls_keys_are_read() {
    let s = Sandbox::new();
    s.write_profiles("A_ENGINE=postgresql\nA_HOST=h\nA_TLS_MODE=unverified\nA_TLS_CA_FILE=/etc/ca.pem\nA_TLS_SERVER_NAME=db.example\n");
    let a = &user_profiles(&s.ctx()).unwrap().profiles["A"];
    assert_eq!(
        (
            a.tls_mode.as_deref(),
            a.tls_ca_file.as_deref(),
            a.tls_server_name.as_deref()
        ),
        (Some("unverified"), Some("/etc/ca.pem"), Some("db.example"))
    );
}

#[test]
#[cfg(unix)]
fn env_format_a_file_readable_by_others_warns_without_values() {
    use std::os::unix::fs::PermissionsExt;
    let s = Sandbox::new();
    s.write_profiles("R_ENGINE=postgresql\nR_PASSWORD=s3cr3t-value\n");
    std::fs::set_permissions(s.env_path(), std::fs::Permissions::from_mode(0o644)).unwrap();
    let l = user_profiles(&s.ctx()).unwrap();
    assert_eq!(l.warnings.len(), 1);
    assert!(l.warnings[0].contains("644") && l.warnings[0].contains("chmod 600"));
    assert_clean(&l.warnings[0], &["s3cr3t-value"]);
}

#[test]
fn files_explicit_file_must_be_toml_or_env_and_a_missing_one_is_empty() {
    let s = Sandbox::new();
    let base = s.base.display().to_string();
    for bad in ["profiles.txt", "profiles", ".env", "profiles.ENV"] {
        let ctx = s.ctx_with(None);
        let mut env = ctx.env.clone();
        env.set("AI_GENT_DB_CONNECTIONS", format!("{base}/{bad}"));
        let e = user_profiles(&Ctx::new(env, ctx.root.clone(), None)).unwrap_err();
        assert!(
            e.to_string().contains("must name a .toml or .env file"),
            "{bad}"
        );
    }
    let ctx = s.ctx();
    let mut env = ctx.env.clone();
    env.set("AI_GENT_DB_CONNECTIONS", format!("{base}/absent.toml"));
    assert!(
        user_profiles(&Ctx::new(env, ctx.root, None))
            .unwrap()
            .profiles
            .is_empty()
    );
}

// ---------------------------------------------------------------- PassTest (needs the fake pass)

#[cfg(unix)]
mod pass {
    use super::*;

    #[test]
    fn pass_sources_in_order() {
        let s = Sandbox::new();
        s.entry("ai-gent/db/shop-dev", "from-default-entry\nnotes line\n");
        s.entry("work/db-pw", "from-explicit-entry\n");
        s.write_profiles("SHOP_DEV_ENGINE=postgresql\nSHOP_DEV_HOST=db.internal\nSHOP_DEV_USER=app\nEXPLICIT_ENGINE=postgresql\nEXPLICIT_HOST=db\nEXPLICIT_USER=app\nEXPLICIT_PASSWORD=pass:work/db-pw\nPLAIN_ENGINE=postgresql\nPLAIN_PASSWORD=plain-value\nNONE_ENGINE=postgresql\n");
        let ctx = s.ctx();
        let p = user_profiles(&ctx).unwrap().profiles;
        assert_eq!(
            p["SHOP_DEV"].secret,
            SecretSource::Pass("ai-gent/db/shop-dev".into())
        );
        assert_eq!(
            p["EXPLICIT"].secret,
            SecretSource::Pass("work/db-pw".into())
        );
        assert!(
            p["EXPLICIT"].password.is_none(),
            "the pass: reference is not a password"
        );
        assert_eq!(p["PLAIN"].secret, SecretSource::File);
        assert_eq!(p["NONE"].secret, SecretSource::None);
        // Read only when connecting; the first line only.
        assert!(
            get_profile(&ctx, "SHOP_DEV", false)
                .unwrap()
                .password
                .is_none()
        );
        assert_eq!(
            get_profile(&ctx, "SHOP_DEV", true)
                .unwrap()
                .password
                .unwrap()
                .expose(),
            "from-default-entry"
        );
        assert_eq!(
            get_profile(&ctx, "EXPLICIT", true)
                .unwrap()
                .password
                .unwrap()
                .expose(),
            "from-explicit-entry"
        );
        // The value read is now redacted from any message of this process (host and user as well).
        let shown = ai_gent_core::passstore::redact(
            "auth failed (from-default-entry) at db.internal as app",
        );
        assert_clean(&shown, &["from-default-entry"]);
    }

    #[test]
    fn pass_prefix_override() {
        let s = Sandbox::new().with("AI_GENT_PASS_PREFIX", "me");
        s.entry("me/db/x", "v\n");
        s.write_profiles("X_ENGINE=postgresql\n");
        assert_eq!(
            user_profiles(&s.ctx()).unwrap().profiles["X"].secret,
            SecretSource::Pass("me/db/x".into())
        );
    }

    #[test]
    fn pass_listing_shows_source_never_value() {
        let s = Sandbox::new();
        s.entry("ai-gent/db/shop-dev", "never-shown\n");
        s.write_profiles(
            "SHOP_DEV_ENGINE=postgresql\nPLAIN_ENGINE=postgresql\nPLAIN_PASSWORD=plain-value\n",
        );
        let r = rows(&user_profiles(&s.ctx()).unwrap().profiles);
        let text = rows_json(&r);
        assert!(text.contains("ai-gent/db/shop-dev") && text.contains("\"file\""));
        assert_clean(&text, &["never-shown", "plain-value"]);
    }

    #[test]
    fn pass_bad_or_unreadable_entry() {
        // Each input alone in its file: a bad reference refuses the load, naming the profile, never the input.
        for bad in [
            "../escape",
            "-evil",
            "/abs/path",
            "a b",
            "ai-gent/db/a\nPROD_HOST=attacker.example",
        ] {
            let s = Sandbox::new();
            s.write_profiles("");
            let esc = bad.replace('\n', "\\n");
            s.write_toml(&format!(
                "[profiles.A]\nengine=\"postgresql\"\nhost=\"db\"\nuser=\"app\"\npassword=\"pass:{esc}\"\n"
            ));
            let ctx = s.ctx();
            for call in [
                user_profiles(&ctx).map(|_| ()),
                get_profile(&ctx, "A", true).map(|_| ()),
            ] {
                let e = call.unwrap_err();
                assert!(e.to_string().contains("isn't a pass entry"), "{bad:?}: {e}");
                assert!(e.to_string().contains("profile A"), "{e}");
                assert_clean(&e.to_string(), &["attacker.example", "escape", "evil"]);
            }
        }
        let s = Sandbox::new();
        s.write_profiles(
            "B_ENGINE=postgresql\nB_HOST=db\nB_USER=app\nB_PASSWORD=pass:ai-gent/db/missing\n",
        );
        let e = get_profile(&s.ctx(), "B", true).unwrap_err();
        assert!(
            e.to_string().contains("couldn't read ai-gent/db/missing"),
            "{e}"
        );
    }

    const BREAKS: [&str; 10] = [
        "\r", "\r\n", "\x0b", "\x0c", "\x1c", "\x1d", "\x1e", "\u{85}", "\u{2028}", "\u{2029}",
    ];

    #[test]
    fn pass_migrate_single_with_every_line_break_python_knows() {
        for br in BREAKS {
            let s = Sandbox::new();
            let text = [
                "A_ENGINE=postgresql",
                "A_PASSWORD=s3cr3t-a",
                "B_ENGINE=postgresql",
                "B_HOST=keep-me",
                "",
            ]
            .join(br);
            s.write_profiles(&text);
            let msg = secrets::migrate_one(&s.ctx(), "A").unwrap_or_else(|e| panic!("{br:?}: {e}"));
            assert_clean(&msg, &["s3cr3t-a"]);
            let after = s.env_text();
            assert_clean(&after, &["s3cr3t-a", "A_PASSWORD"]);
            assert_eq!(
                after,
                [
                    "A_ENGINE=postgresql",
                    "B_ENGINE=postgresql",
                    "B_HOST=keep-me",
                    ""
                ]
                .join(br),
                "{br:?}: other profiles survive"
            );
            assert_eq!(
                std::fs::read_to_string(s.store.join("ai-gent/db/a.gpg")).unwrap(),
                "s3cr3t-a\n"
            );
        }
    }

    #[test]
    fn pass_migrate_all_with_every_line_break_python_knows() {
        for br in BREAKS {
            let s = Sandbox::new();
            let text = [
                "A_ENGINE=postgresql",
                "A_USER=u1",
                "A_PASSWORD=s3cr3t-a",
                "B_ENGINE=postgresql",
                "B_PASSWORD=s3cr3t-b",
                "C_HOST=keep-me",
                "",
            ]
            .join(br);
            s.write_profiles(&text);
            let r = secrets::migrate_all(&s.ctx()).unwrap_or_else(|e| panic!("{br:?}: {e}"));
            assert_clean(&r.render(), &["s3cr3t"]);
            let after = s.env_text();
            assert_clean(&after, &["s3cr3t-a", "s3cr3t-b"]);
            assert_eq!(
                after,
                [
                    "A_ENGINE=postgresql",
                    "A_USER=u1",
                    "A_PASSWORD=pass:ai-gent/db/a",
                    "B_ENGINE=postgresql",
                    "B_PASSWORD=pass:ai-gent/db/b",
                    "C_HOST=keep-me",
                    ""
                ]
                .join(br),
                "{br:?}: only the two password lines change, terminators kept"
            );
        }
    }

    #[test]
    fn pass_migrate_refuses_a_symbolic_link_before_writing_any_entry() {
        let s = Sandbox::new();
        let real = s.base.join("real-profiles");
        std::fs::write(
            &real,
            "A_ENGINE=postgresql\nA_USER=u\nA_PASSWORD=s3cr3t-l\n",
        )
        .unwrap();
        std::fs::remove_file(s.env_path()).unwrap();
        std::os::unix::fs::symlink(&real, s.env_path()).unwrap();
        for r in [
            secrets::migrate_one(&s.ctx(), "A").map(|_| ()),
            secrets::migrate_all(&s.ctx()).map(|_| ()),
        ] {
            let e = r.unwrap_err();
            assert!(e.to_string().contains("symbolic link"), "{e}");
            assert_clean(&e.to_string(), &["s3cr3t-l"]);
        }
        assert!(
            !s.store.join("ai-gent/db/a.gpg").exists(),
            "no entry was written before the refusal"
        );
        assert!(std::fs::read_to_string(&real).unwrap().contains("s3cr3t-l"));
    }

    #[test]
    fn pass_migrate_single_moves_the_value_and_removes_the_line() {
        let s = Sandbox::new();
        s.write_profiles(
            "SHOP_DEV_ENGINE=postgresql\nSHOP_DEV_PASSWORD='s3cr3t value'\nSHOP_DEV_USER=app\n",
        );
        let msg = secrets::migrate_one(&s.ctx(), "SHOP_DEV").unwrap();
        assert_clean(&msg, &["s3cr3t"]);
        assert_eq!(
            std::fs::read_to_string(s.store.join("ai-gent/db/shop-dev.gpg")).unwrap(),
            "s3cr3t value\n"
        );
        assert!(!s.env_text().contains("PASSWORD"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(s.env_path())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        assert_eq!(
            user_profiles(&s.ctx()).unwrap().profiles["SHOP_DEV"]
                .secret
                .word(),
            "pass"
        );
        let again = secrets::migrate_one(&s.ctx(), "SHOP_DEV").unwrap();
        assert!(again.contains("nothing to migrate"), "{again}");
    }

    #[test]
    fn pass_migrate_single_refusals_leave_everything_as_it_was() {
        // 1. an entry that exists is never overwritten
        let s = Sandbox::new();
        s.write_profiles("A_ENGINE=postgresql\nA_PASSWORD=s3cr3t-a\n");
        s.entry("ai-gent/db/a", "already-there\n");
        let e = secrets::migrate_one(&s.ctx(), "A").unwrap_err();
        assert!(e.to_string().contains("already exists"), "{e}");
        assert_clean(&e.to_string(), &["s3cr3t-a", "already-there"]);
        assert_eq!(
            std::fs::read_to_string(s.store.join("ai-gent/db/a.gpg")).unwrap(),
            "already-there\n"
        );
        assert!(s.env_text().contains("A_PASSWORD=s3cr3t-a"));
        // 2. a TOML profile is never rewritten
        let s = Sandbox::new();
        s.write_profiles("");
        s.write_toml("[profiles.T]\nengine = \"postgresql\"\npassword = \"s3cr3t-t\"\n");
        let e = secrets::migrate_one(&s.ctx(), "T").unwrap_err();
        assert!(e.to_string().contains("doesn't rewrite"), "{e}");
        assert_clean(&e.to_string(), &["s3cr3t-t"]);
        // 3. an unknown profile
        let e = secrets::migrate_one(&s.ctx(), "NOPE").unwrap_err();
        assert!(e.to_string().contains("no profile 'NOPE'"));
        // 4. a profiles file that is a symbolic link is not replaced
        #[cfg(unix)]
        {
            let s = Sandbox::new();
            let real = s.base.join("real.env");
            std::fs::write(&real, "A_ENGINE=postgresql\nA_PASSWORD=s3cr3t-l\n").unwrap();
            std::fs::remove_file(s.env_path()).unwrap();
            std::os::unix::fs::symlink(&real, s.env_path()).unwrap();
            let e = secrets::migrate_one(&s.ctx(), "A").unwrap_err();
            assert!(e.to_string().contains("symbolic link"), "{e}");
            assert_clean(&e.to_string(), &["s3cr3t-l"]);
            assert!(
                std::fs::symlink_metadata(s.env_path())
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert!(
                std::fs::read_to_string(&real).unwrap().contains("s3cr3t-l"),
                "the plain value is still there"
            );
        }
    }

    #[test]
    fn pass_migrate_all_accounts_and_reuse() {
        let s = Sandbox::new();
        s.write_profiles("A_ENGINE=postgresql\nA_HOST=h1\nA_USER=app\nA_PASSWORD=shared-one\nB_ENGINE=postgresql\nB_HOST=h2\nB_USER=app\nexport B_PASSWORD='shared-one'\nC_ENGINE=postgresql\nC_HOST=h3\nC_USER=report\nC_PASSWORD=shared-one\nD_ENGINE=postgresql\nD_HOST=h4\nD_USER=solo\nD_PASSWORD=only-d\nE_ENGINE=postgresql\nE_HOST=h5\nE_USER=dup\nE_PASSWORD=pw-e1\nF_ENGINE=postgresql\nF_HOST=h6\nF_USER=dup\nF_PASSWORD=pw-f2\n");
        let r = secrets::migrate_all(&s.ctx()).unwrap();
        let text = r.render();
        for v in ["shared-one", "only-d", "pw-e1", "pw-f2"] {
            assert_clean(&text, &[v]);
        }
        let file = s.env_text();
        assert!(
            file.contains("A_PASSWORD=pass:ai-gent/db/app"),
            "one entry per shared account"
        );
        assert!(file.contains("export B_PASSWORD=pass:ai-gent/db/app"));
        assert!(
            file.contains("C_PASSWORD=pass:ai-gent/db/c"),
            "another account: its own entry"
        );
        assert!(
            file.contains("E_PASSWORD=pass:ai-gent/db/e")
                && file.contains("F_PASSWORD=pass:ai-gent/db/f"),
            "same user, different passwords"
        );
        assert_eq!(
            std::fs::read_to_string(s.store.join("ai-gent/db/app.gpg")).unwrap(),
            "shared-one\n"
        );
        assert!(
            text.contains("6 profile(s) now reference 5 new pass entries"),
            "{text}"
        );
        assert!(
            text.contains("hygiene: different accounts share one password: A, B, C"),
            "{text}"
        );
        assert!(
            user_profiles(&s.ctx())
                .unwrap()
                .profiles
                .values()
                .all(|p| p.secret.word() == "pass")
        );
        assert_eq!(r.log_line(), "6 profiles, 5 entries");
    }

    #[test]
    fn pass_migrate_all_refuses_an_entry_name_that_escapes_the_store() {
        // Two profiles share the user "../evil": the shared entry would be ai-gent/db/../evil.
        let s = Sandbox::new();
        s.write_profiles("A_ENGINE=postgresql\nA_HOST=h\nA_USER=../evil\nA_PASSWORD=s3cr3t-x\nB_ENGINE=postgresql\nB_HOST=h\nB_USER=../evil\nB_PASSWORD=s3cr3t-x\n");
        let e = secrets::migrate_all(&s.ctx()).unwrap_err();
        assert!(e.to_string().contains("isn't a pass entry"), "{e}");
        assert_clean(&e.to_string(), &["s3cr3t-x"]);
        assert!(
            s.env_text().contains("A_PASSWORD=s3cr3t-x"),
            "the file is untouched"
        );
        assert!(!s.store.join("evil.gpg").exists());
    }

    #[test]
    fn pass_migrate_all_keeps_existing_entries_and_lists_toml_profiles() {
        let s = Sandbox::new();
        s.write_profiles("A_ENGINE=postgresql\nA_USER=u\nA_PASSWORD=s3cr3t-a\n");
        s.entry("ai-gent/db/a", "old\n");
        s.write_toml("[profiles.T]\nengine = \"postgresql\"\npassword = \"s3cr3t-t\"\n");
        let r = secrets::migrate_all(&s.ctx()).unwrap();
        let text = r.render();
        assert!(
            text.contains("left as they are, the entry already exists: ai-gent/db/a"),
            "{text}"
        );
        assert!(text.contains("doesn't rewrite TOML): T"), "{text}");
        assert_clean(&text, &["s3cr3t-a", "s3cr3t-t"]);
        assert!(s.env_text().contains("A_PASSWORD=s3cr3t-a"));
    }
}

// ---------------------------------------------------------------- TomlTest

const TOML: &str = r#"
[defaults]
engine = "postgresql"
class = "development"
port = 5432

[families.shop]
host = "db.internal"
user = "shop_ro"
password = "pass:work/db/shop"

[profiles.SHOP_DEV]
family = "shop"
database = "shop_dev"

[profiles.SHOP_QA]
family = "shop"
database = "shop_qa"
host = "qa.internal"

[profiles.LEGACY]
engine = "oracle"
host = "ora.internal"
user = "app"
sid = "ORCL"
port = 1522
"#;

#[test]
fn toml_families_and_precedence() {
    let s = Sandbox::new();
    s.write_profiles("");
    s.write_toml(TOML);
    let p = user_profiles(&s.ctx()).unwrap().profiles;
    assert_eq!(
        p.keys().cloned().collect::<Vec<_>>(),
        ["LEGACY", "SHOP_DEV", "SHOP_QA"]
    );
    assert_eq!(
        p["SHOP_DEV"].host.as_deref(),
        Some("db.internal"),
        "from the family"
    );
    assert_eq!(
        p["SHOP_QA"].host.as_deref(),
        Some("qa.internal"),
        "the profile wins"
    );
    assert_eq!(
        p["SHOP_DEV"].port.as_deref(),
        Some("5432"),
        "defaults, an integer"
    );
    assert_eq!(
        p["SHOP_DEV"].secret,
        SecretSource::Pass("work/db/shop".into()),
        "one password per family"
    );
    assert_eq!(p["LEGACY"].port.as_deref(), Some("1522"));
    assert_eq!(p["LEGACY"].class, Class::Development);
}

#[test]
#[cfg(unix)]
fn toml_family_default_entry() {
    let s = Sandbox::new();
    s.entry("ai-gent/db/orders", "v\n");
    s.write_profiles("");
    s.write_toml("[families.orders]\nengine = \"postgresql\"\nuser = \"o\"\nhost = \"h\"\n[profiles.ORDERS_DEV]\nfamily = \"orders\"\n[profiles.ORDERS_QA]\nfamily = \"orders\"\n");
    let p = user_profiles(&s.ctx()).unwrap().profiles;
    assert_eq!(
        p["ORDERS_DEV"].secret,
        SecretSource::Pass("ai-gent/db/orders".into())
    );
    assert_eq!(
        p["ORDERS_QA"].secret,
        SecretSource::Pass("ai-gent/db/orders".into())
    );
}

#[test]
fn toml_errors_name_keys_never_values() {
    let cases = [
        (
            "[profiles.X]\nengine = \"postgresql\"\npasword = \"s3cr3t-typo\"\n",
            "unknown key 'pasword' in [profiles.X]",
        ),
        ("[profiles.X]\nfamily = \"nope\"\n", "names family 'nope'"),
        (
            "[secrets]\nx = \"s3cr3t-typo\"\n",
            "unknown table [secrets]",
        ),
        ("[profiles.x]\nengine = \"postgresql\"\n", "must be [A-Z]"),
        (
            "[profiles.X]\nport = true\n",
            "port must be a string or an integer",
        ),
        ("[profiles.X]\nhost = \"s3cr3t-typo\n", "line 2"),
        ("[profiles.X]\nhost = 7\n", "host must be a string"),
        (
            "[defaults]\nsecret = \"s3cr3t-typo\"\n",
            "unknown key 'secret' in [defaults]",
        ),
        (
            "[families.Bad]\nhost = \"s3cr3t-typo\"\n",
            "family 'Bad' must be lowercase",
        ),
        (
            "profiles = \"s3cr3t-typo\"\n",
            "[profiles] must hold tables",
        ),
        ("[profiles.X]\nfamily = 3\n", "family must be a string"),
    ];
    for (text, want) in cases {
        let s = Sandbox::new();
        s.write_profiles("");
        s.write_toml(text);
        let e = user_profiles(&s.ctx()).unwrap_err();
        assert!(e.to_string().contains(want), "{want}: {e}");
        assert_clean(
            &e.to_string(),
            &["s3cr3t-typo", &s.base.display().to_string()],
        );
    }
}

#[test]
fn toml_before_env_and_no_duplicates() {
    let s = Sandbox::new();
    s.write_toml(TOML);
    s.write_profiles("OTHER_ENGINE=postgresql\n");
    assert!(
        user_profiles(&s.ctx())
            .unwrap()
            .profiles
            .contains_key("OTHER")
    );
    s.write_profiles("LEGACY_ENGINE=postgresql\n");
    let e = user_profiles(&s.ctx()).unwrap_err();
    assert!(
        e.to_string()
            .contains("LEGACY is defined in both connections.toml and connections.env"),
        "{e}"
    );
}

#[test]
fn toml_explicit_file() {
    let s = Sandbox::new();
    let f = s.base.join("team.toml");
    std::fs::write(&f, TOML).unwrap();
    let ctx = s.ctx();
    let mut env = ctx.env.clone();
    env.set("AI_GENT_DB_CONNECTIONS", f.to_str().unwrap());
    let p = user_profiles(&Ctx::new(env, ctx.root, None))
        .unwrap()
        .profiles;
    assert!(
        p.contains_key("SHOP_DEV") && !p.contains_key("LOCALDB"),
        "only the named file is read"
    );
}

#[test]
fn toml_oracle_sid_and_missing_keys() {
    let s = Sandbox::new();
    s.write_profiles("");
    assert!(user_profiles(&s.ctx()).unwrap().profiles.is_empty());
    s.write_toml("[profiles.O1]\nengine=\"oracle\"\nhost=\"h\"\nport=1522\nservice=\"SVC\"\n[profiles.O2]\nengine=\"oracle\"\nhost=\"h\"\ndatabase=\"PDB1\"\n[profiles.O3]\nengine=\"oracle\"\nhost=\"h\"\nport=1522\nsid=\"ORCL\"\n[profiles.O4]\nengine=\"oracle\"\nhost=\"h\"\nuser=\"u\"\nsid=\"S\"\n[profiles.O5]\nengine=\"oracle\"\nhost=\"h\"\nuser=\"u\"\n[profiles.LITE]\nengine=\"sqlite\"\n");
    let l = user_profiles(&s.ctx()).unwrap().profiles;
    assert_eq!(oracle_dsn(&l["O1"]).unwrap(), "h:1522/SVC");
    assert_eq!(oracle_dsn(&l["O2"]).unwrap(), "h:1521/PDB1");
    assert_eq!(
        oracle_dsn(&l["O3"]).unwrap(),
        "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST=h)(PORT=1522))(CONNECT_DATA=(SID=ORCL)))"
    );
    assert!(l["O4"].missing_keys().is_empty());
    assert_eq!(l["O5"].missing_keys(), ["SERVICE|SID"]);
    assert_eq!(l["O1"].missing_keys(), ["USER"]);
    assert_eq!(l["LITE"].missing_keys(), ["PATH"]);
    let e = get_profile(&s.ctx(), "O5", true).unwrap_err();
    assert!(
        e.to_string().contains("profile O5 is missing SERVICE|SID"),
        "{e}"
    );
}

#[test]
fn oracle_descriptor_characters_are_refused() {
    let s = Sandbox::new();
    // Inputs that would restructure a descriptor, each in a different field.
    for (field, bad) in [
        ("HOST", "h)(HOST=evil"),
        ("SID", "S)(X=1"),
        ("SERVICE", "a b"),
        ("PORT", "1521\\x"),
        ("SID", "S\"q"),
        ("DATABASE", "d=1"),
    ] {
        let mut lines = vec![
            "O_ENGINE=oracle".to_string(),
            "O_HOST=h".to_string(),
            "O_SID=S".to_string(),
        ];
        lines.retain(|l| !l.starts_with(&format!("O_{field}=")));
        lines.push(format!("O_{field}={bad}"));
        s.write_profiles(&(lines.join("\n") + "\n"));
        let p = &user_profiles(&s.ctx()).unwrap().profiles["O"];
        let e = oracle_dsn(p).unwrap_err();
        let named = if field == "DATABASE" {
            "service"
        } else {
            &field.to_lowercase()
        };
        assert!(e.to_string().contains(named), "{field}: {e}");
        assert_clean(&e.to_string(), &[bad]);
    }
}

#[test]
fn toml_export_env() {
    let s = Sandbox::new();
    s.write_profiles("");
    s.write_toml(&format!("{TOML}[profiles.PLAIN]\nengine = \"postgresql\"\nhost = \"h\"\nuser = \"u\"\npassword = \"s3cr3t-plain\"\n"));
    let o = s.base.join("exported.env");
    let msg = export_env(&s.ctx(), Some(&o)).unwrap();
    let text = std::fs::read_to_string(&o).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&o).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert!(
        text.contains("SHOP_DEV_PASSWORD=pass:work/db/shop") && text.contains("LEGACY_SID=ORCL")
    );
    assert_clean(&(text.clone() + &msg), &["s3cr3t-plain"]);
    assert!(msg.contains("PLAIN") && !msg.contains("db.internal"));
    assert_eq!(
        export_env(&s.ctx(), Some(&o)).unwrap_err().to_string(),
        "exported.env exists; choose another --out"
    );
    // The export parses back to the same profiles (round trip).
    let again = files::env_profiles(&files::read_env_file(&o).unwrap(), &o);
    assert_eq!(again["SHOP_DEV"].fields["host"], "db.internal");
    assert_eq!(again["LEGACY"].fields["port"], "1522");
}

#[test]
fn export_env_refusals() {
    let s = Sandbox::new();
    s.write_profiles("");
    let o = s.base.join("o.env");
    assert!(
        export_env(&s.ctx(), None)
            .unwrap_err()
            .to_string()
            .contains("needs --out")
    );
    assert!(!o.exists());
    // A value with a line break would end the line and start another one: refused, nothing written.
    s.write_toml("[profiles.A]\nengine=\"postgresql\"\nhost=\"a\\nA_PASSWORD=injected\"\n");
    let e = export_env(&s.ctx(), Some(&o)).unwrap_err();
    assert!(e.to_string().contains("line break"), "{e}");
    assert!(!o.exists(), "no half-written file");
    assert_clean(&e.to_string(), &["injected"]);
    // Every other line break Python's splitlines knows, and control characters, are refused too.
    for bad in [
        "\\r", "\\u000b", "\\u000c", "\\u001c", "\\u0085", "\\u2028", "\\u2029", "\\u0007",
    ] {
        s.write_toml(&format!(
            "[profiles.A]\nengine=\"postgresql\"\nhost=\"a{bad}A_PASSWORD=injected\"\n"
        ));
        let e = export_env(&s.ctx(), Some(&o)).unwrap_err();
        assert!(
            e.to_string().contains("line break or control"),
            "{bad}: {e}"
        );
        assert!(!o.exists(), "{bad}: nothing written");
        assert_clean(&e.to_string(), &["injected"]);
    }
    // Both quote characters and whitespace cannot be quoted back.
    s.write_toml("[profiles.A]\nengine=\"postgresql\"\nhost=\"a 'b\\\" c\"\n");
    assert!(
        export_env(&s.ctx(), Some(&o))
            .unwrap_err()
            .to_string()
            .contains("both quote")
    );
    assert!(!o.exists());
    // A value with spaces and one kind of quote round-trips.
    s.write_toml("[profiles.A]\nengine=\"postgresql\"\nhost=\"it's here\"\n");
    export_env(&s.ctx(), Some(&o)).unwrap();
    let back = files::env_profiles(&files::read_env_file(&o).unwrap(), &o);
    assert_eq!(back["A"].fields["host"], "it's here");
}

#[test]
fn profiles_all_names_must_be_unique_and_get_profile_checks_keys_and_engine() {
    let s = Sandbox::new();
    s.write_profiles(
        "A_ENGINE=postgresql\nB_ENGINE=cobol\nB_HOST=h\nB_USER=u\nC_ENGINE=postgresql\nC_USER=u\n",
    );
    let ctx = s.ctx();
    let e = get_profile(&ctx, "NOPE", false).unwrap_err();
    assert!(
        e.to_string().contains("no profile 'NOPE' (known: A, B, C)"),
        "{e}"
    );
    let e = get_profile(&ctx, "C", true).unwrap_err();
    assert!(e.to_string().contains("profile C is missing HOST"), "{e}");
    let e = get_profile(&ctx, "B", true).unwrap_err();
    assert!(e.to_string().contains("engine 'cobol' isn't one of"), "{e}");
    assert!(
        get_profile(&ctx, "C", false).is_ok(),
        "keys are checked only when connecting"
    );
}

// ---------------------------------------------------------------- containers

type PortMap<'a> = &'a [(&'a str, &'a str, &'a str)]; // (container port/tcp, HostIp, HostPort)

fn info_for(
    name: &str,
    image: &str,
    running: bool,
    wd: &Path,
    ports: PortMap<'_>,
    env: &[&str],
) -> Value {
    let mut p = serde_json::Map::new();
    for (k, ip, hp) in ports {
        p.insert((*k).into(), json!([{"HostIp": ip, "HostPort": hp}]));
    }
    json!({
        "Id": format!("id-{name}"), "Pod": "", "Name": name,
        "State": {"Running": running, "Pid": 4242},
        "Config": {"Image": image, "Env": env, "Labels": {"com.docker.compose.project.working_dir": wd.display().to_string()}},
        "NetworkSettings": {"Ports": p}
    })
}

type Spec<'a> = (&'a str, &'a str, bool, PortMap<'a>);

/// A docker-like fake whose containers all belong to `wd`.
fn compose_engine(wd: &Path, specs: Vec<Spec<'_>>) -> Arc<FakeEngine> {
    let listing: Vec<Value> = specs
        .iter()
        .map(|(n, img, _, _)| json!({"ID": format!("id-{n}"), "Names": [n], "Image": img,
            "Labels": format!("com.docker.compose.project.working_dir={},com.docker.compose.service={n}", wd.display())}))
        .collect();
    let infos: std::collections::BTreeMap<String, Value> = specs
        .iter()
        .map(|(n, img, run, ports)| {
            (
                (*n).to_string(),
                info_for(
                    n,
                    img,
                    *run,
                    wd,
                    ports,
                    &[
                        "POSTGRES_USER=pguser",
                        "POSTGRES_PASSWORD=s3cr3t-db",
                        "POSTGRES_DB=shop",
                    ],
                ),
            )
        })
        .collect();
    fake("docker", move |a| match a {
        ["context", "inspect", ..] => out("unix:///var/run/docker.sock"),
        ["ps", ..] => out(&serde_json::to_string(&listing).unwrap()),
        ["inspect", "--format", _, name] => {
            infos.get(*name).map_or_else(fail, |v| out(&v.to_string()))
        }
        _ => fail(),
    })
}

#[test]
fn local_discovery_finds_this_repositorys_database_containers_only() {
    let s = Sandbox::new();
    let other = s.outside.clone();
    let listing = json!([
        {"ID":"1","Names":["shop-db"],"Image":"postgres:17","Labels":{"com.docker.compose.project.working_dir": s.repo.display().to_string(), "com.docker.compose.service":"db"}},
        {"ID":"2","Names":["pgb"],"Image":"edoburu/pgbouncer","Labels":{"com.docker.compose.project.working_dir": s.repo.display().to_string(), "com.docker.compose.service":"pgb"}},
        {"ID":"3","Names":["foreign-db"],"Image":"postgres:17","Labels":{"com.docker.compose.project.working_dir": other.display().to_string()}},
        {"ID":"4","Names":["nolabel"],"Image":"postgres:17","Labels":{}},
        {"ID":"5","Names":["-evil"],"Image":"postgres:17","Labels":{"com.docker.compose.project.working_dir": s.repo.display().to_string()}},
        {"ID":"6","Names":["web"],"Image":"nginx","Labels":{"com.docker.compose.project.working_dir": s.repo.display().to_string()}}
    ]);
    let info = info_for(
        "shop-db",
        "postgres:17",
        true,
        &s.repo,
        &[("5432/tcp", "0.0.0.0", "15432")],
        &[
            "POSTGRES_USER=pguser",
            "POSTGRES_PASSWORD=s3cr3t-db",
            "POSTGRES_DB=shop",
        ],
    );
    let eng = fake("docker", move |a| match a {
        ["context", ..] => out("unix:///var/run/docker.sock"),
        ["ps", ..] => out(&listing.to_string()),
        ["inspect", "--format", _, "shop-db"] => out(&info.to_string()),
        _ => fail(),
    });
    let ctx = s.ctx_with(Some(eng.clone()));
    s.write_profiles("");
    let l = all_profiles(&ctx).unwrap();
    assert_eq!(l.profiles.keys().cloned().collect::<Vec<_>>(), ["local:db"]);
    let p = &l.profiles["local:db"];
    assert_eq!(
        (
            p.engine.as_str(),
            p.class,
            p.secret.word(),
            p.host.as_deref(),
            p.port.as_deref()
        ),
        (
            "postgresql",
            Class::Local,
            "container",
            Some("127.0.0.1"),
            Some("5432")
        )
    );
    assert_eq!(
        (p.user.as_deref(), p.database.as_deref()),
        (Some("pguser"), Some("shop"))
    );
    assert_eq!(
        p.route,
        Some(Route::Published {
            host: "127.0.0.1".into(),
            port: 15432
        })
    );
    assert!(p.missing_keys().is_empty());
    // The listing never shows the container's password; Debug neither.
    assert_clean(&rows_json(&rows(&l.profiles)), &["s3cr3t-db", "pguser"]);
    assert_clean(&format!("{p:?}"), &["s3cr3t-db", "pguser", "127.0.0.1"]);
    // The engine was never asked about the foreign container, the proxy, the unlabeled one, or "-evil".
    let calls = eng.calls.lock().unwrap().clone();
    assert!(
        calls.iter().all(|c| !c
            .iter()
            .any(|a| a == "-evil" || a == "foreign-db" || a == "pgb")),
        "{calls:?}"
    );
}

#[test]
fn local_discovery_is_empty_for_a_remote_engine_or_a_failing_one() {
    let s = Sandbox::new();
    s.write_profiles("");
    let mk = || compose_engine(&s.repo, vec![("db", "postgres:17", true, &[])]);
    for (var, val) in [
        ("DOCKER_HOST", "tcp://secret-host:2375"),
        ("CONTAINER_HOST", "ssh://me@secret-host/run/podman.sock"),
    ] {
        let s2 = Sandbox::new().with(var, val);
        s2.write_profiles("");
        let eng = compose_engine(&s2.repo, vec![("db", "postgres:17", true, &[])]);
        assert!(
            all_profiles(&s2.ctx_with(Some(eng)))
                .unwrap()
                .profiles
                .is_empty(),
            "{var}"
        );
    }
    let broken = fake("docker", |_| fail());
    assert!(
        all_profiles(&s.ctx_with(Some(broken)))
            .unwrap()
            .profiles
            .is_empty()
    );
    assert_eq!(
        all_profiles(&s.ctx_with(Some(mk())))
            .unwrap()
            .profiles
            .len(),
        1
    );
}

#[test]
fn local_discovery_two_names_collide_with_a_file_profile() {
    let s = Sandbox::new();
    s.write_profiles("");
    let eng = compose_engine(&s.repo, vec![("db", "postgres:17", true, &[])]);
    // The container profile is `local:db`: a file cannot define that name (':' is not allowed), so
    // the collision branch is only reachable through a TOML key, which the name rule also refuses.
    s.write_toml("[profiles.\"local:db\"]\nengine=\"postgresql\"\n");
    let e = all_profiles(&s.ctx_with(Some(eng))).unwrap_err();
    assert!(e.to_string().contains("must be [A-Z]"), "{e}");
}

#[test]
fn engine_credentials_per_image_family() {
    let s = Sandbox::new();
    s.write_profiles("");
    let mk = |image: &str, env: Vec<&'static str>| {
        let wd = s.repo.display().to_string();
        let img = image.to_string();
        let info = json!({"Id":"i","State":{"Running":true},"Config":{"Image":img,"Env":env,"Labels":{"com.docker.compose.project.working_dir":wd}},"NetworkSettings":{"Ports":{}}});
        let list = json!([{"ID":"i","Names":["c1"],"Image":image,"Labels":{"com.docker.compose.project.working_dir":wd,"com.docker.compose.service":"svc"}}]);
        fake("docker", move |a| match a {
            ["context", ..] => out("unix:///s"),
            ["ps", ..] => out(&list.to_string()),
            ["inspect", ..] => out(&info.to_string()),
            _ => fail(),
        })
    };
    let get = |image: &str, env: Vec<&'static str>| {
        let l = all_profiles(&s.ctx_with(Some(mk(image, env)))).unwrap();
        let p = l.profiles["local:svc"].clone();
        (
            p.engine.clone(),
            p.user.clone().unwrap(),
            p.password.as_ref().map(|x| x.expose().to_string()),
            p.database.clone(),
            p.service.clone(),
            p.route.clone(),
        )
    };
    let (e, u, pw, d, _, r) = get(
        "mysql:8",
        vec!["MYSQL_ROOT_PASSWORD=rootpw", "MYSQL_DATABASE=app"],
    );
    assert_eq!(
        (e.as_str(), u.as_str(), pw.as_deref(), d.as_deref(), r),
        (
            "mysql",
            "root",
            Some("rootpw"),
            Some("app"),
            Some(Route::ContainerExec)
        )
    );
    let (_, u, pw, ..) = get("mariadb:11", vec!["MARIADB_USER=mu", "MARIADB_PASSWORD=mp"]);
    assert_eq!((u.as_str(), pw.as_deref()), ("mu", Some("mp")));
    let (e, u, pw, d, ..) = get(
        "mcr.microsoft.com/mssql/server",
        vec!["MSSQL_SA_PASSWORD=sapw"],
    );
    assert_eq!(
        (e.as_str(), u.as_str(), pw.as_deref(), d.as_deref()),
        ("sqlserver", "sa", Some("sapw"), Some("master"))
    );
    let (e, u, _, _, svc, _) = get("gvenzl/oracle-free:23-slim", vec!["ORACLE_PASSWORD=opw"]);
    assert_eq!(
        (e.as_str(), u.as_str(), svc.as_deref()),
        ("oracle", "system", Some("FREEPDB1"))
    );
    let (_, u, pw, d, ..) = get("postgres:17", vec![]);
    assert_eq!(
        (u.as_str(), pw, d.as_deref()),
        ("postgres", None, Some("postgres")),
        "empty password: none, database defaults to the user"
    );
}

fn container_profile(s: &Sandbox, extra: &str) -> (Ctx, ai_gent_dbrun::profiles::Profile) {
    s.write_profiles(&format!(
        "L_ENGINE=postgresql\nL_CLASS=local\nL_HOST=127.0.0.1\nL_CONTAINER=shop-db\n{extra}"
    ));
    let ctx = s.ctx();
    let p = user_profiles(&ctx).unwrap().profiles["L"].clone();
    (ctx, p)
}

fn classify_in(
    s: &Sandbox,
    p: &ai_gent_dbrun::profiles::Profile,
    eng: Arc<FakeEngine>,
) -> ai_gent_dbrun::profiles::Classification {
    let ctx = s.ctx_with(Some(eng));
    classify(&ctx, p).unwrap()
}

#[test]
fn classify_container_proven_local_with_a_published_port() {
    let s = Sandbox::new();
    let (_, p) = container_profile(&s, "");
    let eng = compose_engine(
        &s.repo,
        vec![(
            "shop-db",
            "postgres:17",
            true,
            &[("5432/tcp", "127.0.0.1", "55432")],
        )],
    );
    let c = classify_in(&s, &p, eng);
    assert_eq!(
        (c.class, c.writable, c.via),
        (Class::Local, true, Via::Container)
    );
    assert_eq!(
        c.route,
        Some(Route::Published {
            host: "127.0.0.1".into(),
            port: 55432
        })
    );
    let f = c.container.unwrap();
    assert_eq!((f.id.as_str(), f.pid), ("id-shop-db", 4242));
    assert!(
        c.evidence
            .iter()
            .any(|e| e.contains("published port 127.0.0.1:55432")),
        "{:?}",
        c.evidence
    );
}

#[test]
fn classify_container_without_a_published_port_selects_the_exec_path() {
    let s = Sandbox::new();
    let (_, p) = container_profile(&s, "");
    let eng = compose_engine(&s.repo, vec![("shop-db", "postgres:17", true, &[])]);
    let c = classify_in(&s, &p, eng);
    assert_eq!(
        (c.class, c.route),
        (Class::Local, Some(Route::ContainerExec))
    );
}

#[test]
fn classify_container_not_proven_cases() {
    // (what is wrong, the engine, the evidence text)
    let s = Sandbox::new();
    let (_, p) = container_profile(&s, "");
    let outside_wd = s.outside.clone();
    let cases: Vec<(&str, Arc<FakeEngine>, &str)> = vec![
        (
            "not running",
            compose_engine(&s.repo, vec![("shop-db", "postgres:17", false, &[])]),
            "is not running",
        ),
        (
            "a proxy image",
            compose_engine(&s.repo, vec![("shop-db", "edoburu/pgbouncer", true, &[])]),
            "not a postgresql server",
        ),
        (
            "another engine",
            compose_engine(&s.repo, vec![("shop-db", "mysql:8", true, &[])]),
            "not a postgresql server",
        ),
        (
            "another repository's compose project",
            compose_engine(&outside_wd, vec![("shop-db", "postgres:17", true, &[])]),
            "wasn't started by this repository",
        ),
        (
            "no such container",
            compose_engine(&s.repo, vec![]),
            "not found",
        ),
    ];
    for (what, eng, says) in cases {
        let c = classify_in(&s, &p, eng);
        assert_eq!((c.class, c.writable), (Class::Production, false), "{what}");
        assert!(
            c.evidence
                .iter()
                .any(|e| e.contains("NOT PROVEN LOCAL") && e.contains(says)),
            "{what}: {:?}",
            c.evidence
        );
    }
}

#[test]
fn classify_container_mysql_profile_accepts_a_mariadb_image() {
    let s = Sandbox::new();
    s.write_profiles("L_ENGINE=mysql\nL_CLASS=local\nL_CONTAINER=shop-db\n");
    let p = user_profiles(&s.ctx()).unwrap().profiles["L"].clone();
    let eng = compose_engine(&s.repo, vec![("shop-db", "mariadb:11", true, &[])]);
    assert_eq!(classify_in(&s, &p, eng).class, Class::Local);
}

#[test]
fn classify_container_names_that_could_be_read_as_options_are_never_passed_to_the_engine() {
    for bad in [
        "-v",
        "--privileged",
        "a b",
        "x;y",
        "$(id)",
        "",
        "../x",
        "a/b",
    ] {
        let s = Sandbox::new();
        s.write_profiles(&format!(
            "L_ENGINE=postgresql\nL_CLASS=local\nL_CONTAINER={bad}\n"
        ));
        let p = user_profiles(&s.ctx()).unwrap().profiles["L"].clone();
        let eng = compose_engine(&s.repo, vec![("shop-db", "postgres:17", true, &[])]);
        let c = classify_in(&s, &p, eng.clone());
        assert_eq!((c.class, c.writable), (Class::Production, false), "{bad:?}");
        let calls = eng.calls.lock().unwrap().clone();
        assert!(
            !calls
                .iter()
                .any(|a| a.first().is_some_and(|f| f == "inspect")),
            "{bad:?}: the engine was asked: {calls:?}"
        );
        assert!(!valid_container_name(bad), "{bad:?}");
    }
    assert!(valid_container_name("shop_db-1.x"));
}

#[test]
fn classify_a_container_name_makes_a_remote_class_attempt_proof_and_a_failed_one_keeps_the_declared_class()
 {
    let s = Sandbox::new();
    s.write_profiles("S_ENGINE=postgresql\nS_CLASS=staging\nS_CONTAINER=shop-db\nP_ENGINE=postgresql\nP_HOST=db.example\n");
    let ctx = s.ctx();
    let l = user_profiles(&ctx).unwrap().profiles;
    // No engine: staging stays staging; no CONTAINER and no local claim: only the declaration.
    assert_eq!(classify(&ctx, &l["S"]).unwrap().class, Class::Staging);
    let c = classify(&ctx, &l["P"]).unwrap();
    assert_eq!(
        (c.class, c.via, c.writable),
        (Class::Production, Via::Host, false)
    );
    assert!(c.evidence[0].starts_with("declared production in "));
}

#[test]
fn socket_endpoint_rules() {
    let docker = |host: &'static str, code: i32| {
        fake("docker", move |a| match a {
            ["context", "inspect", ..] => EngineOut {
                code,
                stdout: host.into(),
            },
            _ => fail(),
        })
    };
    let env = Env::from_pairs([("PATH", "/bin")]);
    assert!(engine_endpoint_local(&*docker("unix:///var/run/docker.sock", 0), &env).0);
    for (host, code) in [
        ("tcp://10.0.0.5:2375", 0),
        ("npipe:////./pipe/docker_engine", 0),
        ("", 0),
        ("unix:///x", 1),
    ] {
        assert!(
            !engine_endpoint_local(&*docker(host, code), &env).0,
            "{host}"
        );
    }
    // DOCKER_HOST / CONTAINER_HOST other than a unix socket: remote, and the message names the scheme only.
    for var in ["DOCKER_HOST", "CONTAINER_HOST"] {
        let env = Env::from_pairs([(var, "tcp://secret-host:2375")]);
        let (ok, why) = engine_endpoint_local(&*docker("unix:///x", 0), &env);
        assert!(
            !ok && why.contains("tcp://") && !why.contains("secret-host"),
            "{why}"
        );
    }
    let env = Env::from_pairs([("DOCKER_HOST", "unix:///run/user/1000/docker.sock")]);
    assert!(
        engine_endpoint_local(&*docker("unix:///x", 0), &env).0,
        "a unix socket in DOCKER_HOST is local"
    );
    // podman: local only when it says the service is not remote.
    let podman = |v: &'static str, code: i32| {
        fake("podman", move |_| EngineOut {
            code,
            stdout: v.into(),
        })
    };
    assert!(engine_endpoint_local(&*podman("false\n", 0), &Env::default()).0);
    assert!(!engine_endpoint_local(&*podman("true\n", 0), &Env::default()).0);
    assert!(!engine_endpoint_local(&*podman("false\n", 1), &Env::default()).0);
}

#[test]
fn image_to_engine_matches_the_original() {
    for (img, want) in [
        ("postgres:17", Some("postgresql")),
        ("postgis/postgis", Some("postgresql")),
        ("timescale/timescaledb", Some("postgresql")),
        ("mariadb:11.4", Some("mariadb")),
        ("mysql:8.4", Some("mysql")),
        ("docker.io/library/mysql", Some("mysql")),
        ("percona:8", Some("mysql")),
        ("mcr.microsoft.com/mssql/server:2022", Some("sqlserver")),
        ("gvenzl/oracle-free", Some("oracle")),
        ("edoburu/pgbouncer", None),
        ("postgres-exporter-nginx", None),
        ("envoyproxy/envoy", None),
        ("redis", None),
        ("mymysql", None),
    ] {
        assert_eq!(image_engine(img), want, "{img}");
    }
}

#[test]
fn published_port_detection() {
    let ports = |v: Value| json!({"NetworkSettings": {"Ports": v}});
    let route = |v: Value, p| detect_route(&ports(v), p);
    let pub_ = |h: &str, p| Route::Published {
        host: h.into(),
        port: p,
    };
    assert_eq!(
        route(
            json!({"5432/tcp":[{"HostIp":"0.0.0.0","HostPort":"15432"}]}),
            5432
        ),
        pub_("127.0.0.1", 15432)
    );
    assert_eq!(
        route(json!({"5432/tcp":[{"HostIp":"","HostPort":"15432"}]}), 5432),
        pub_("127.0.0.1", 15432)
    );
    assert_eq!(
        route(
            json!({"5432/tcp":[{"HostIp":"127.0.0.1","HostPort":"15432"}]}),
            5432
        ),
        pub_("127.0.0.1", 15432)
    );
    assert_eq!(
        route(
            json!({"5432/tcp":[{"HostIp":"::","HostPort":"15432"}]}),
            5432
        ),
        pub_("::1", 15432)
    );
    assert_eq!(
        route(json!({"5432/tcp":[{"HostIp":"::1","HostPort":"1"}]}), 5432),
        pub_("::1", 1)
    );
    // the second mapping is used when the first is unusable
    assert_eq!(
        route(
            json!({"5432/tcp":[{"HostIp":"10.0.0.9","HostPort":"1"},{"HostIp":"0.0.0.0","HostPort":"2"}]}),
            5432
        ),
        pub_("127.0.0.1", 2)
    );
    // everything below must send the profile to the exec path, never a driver to an address the proof did not cover
    for (what, v) in [
        (
            "another address",
            json!({"5432/tcp":[{"HostIp":"10.0.0.9","HostPort":"15432"}]}),
        ),
        (
            "a public address",
            json!({"5432/tcp":[{"HostIp":"192.168.1.5","HostPort":"15432"}]}),
        ),
        ("no mapping", json!({"5432/tcp": null})),
        ("empty list", json!({"5432/tcp": []})),
        (
            "another port",
            json!({"3306/tcp":[{"HostIp":"0.0.0.0","HostPort":"3306"}]}),
        ),
        (
            "udp only",
            json!({"5432/udp":[{"HostIp":"0.0.0.0","HostPort":"5432"}]}),
        ),
        (
            "port 0",
            json!({"5432/tcp":[{"HostIp":"0.0.0.0","HostPort":"0"}]}),
        ),
        (
            "not a number",
            json!({"5432/tcp":[{"HostIp":"0.0.0.0","HostPort":"x"}]}),
        ),
        (
            "too large",
            json!({"5432/tcp":[{"HostIp":"0.0.0.0","HostPort":"70000"}]}),
        ),
        (
            "a lookalike host",
            json!({"5432/tcp":[{"HostIp":"127.0.0.1.evil.com","HostPort":"1"}]}),
        ),
        ("not a list", json!({"5432/tcp":"x"})),
    ] {
        assert_eq!(route(v, 5432), Route::ContainerExec, "{what}");
    }
    assert_eq!(detect_route(&json!({}), 5432), Route::ContainerExec);
    assert_eq!(
        detect_route(&json!({"HostConfig":{"NetworkMode":"host"}}), 5432),
        Route::ContainerExec,
        "host networking"
    );
}

// ---------------------------------------------------------------- sqlite classification

#[test]
fn classify_sqlite_inside_outside_symlink_and_missing() {
    let s = Sandbox::new();
    s.write_profiles("");
    let write = |lines: &str| s.write_profiles(lines);
    write(&format!(
        "IN_ENGINE=sqlite\nIN_PATH={}\nOUT_ENGINE=sqlite\nOUT_CLASS=local\nOUT_PATH={}\n",
        s.repo.join("app.db").display(),
        s.outside.join("prod.db").display()
    ));
    let ctx = s.ctx();
    let l = user_profiles(&ctx).unwrap().profiles;
    let c = classify(&ctx, &l["IN"]).unwrap();
    assert!(
        c.writable && c.evidence[0].contains("inside this repository (app.db)"),
        "{:?}",
        c.evidence
    );
    let c = classify(&ctx, &l["OUT"]).unwrap();
    assert_eq!(
        (c.class, c.writable),
        (Class::Production, false),
        "declared local but outside: production"
    );
    #[cfg(unix)]
    {
        // A link inside the repository pointing out, and one pointing in: neither is a local copy.
        std::os::unix::fs::symlink(s.outside.join("prod.db"), s.repo.join("link-out.db")).unwrap();
        std::os::unix::fs::symlink(s.repo.join("app.db"), s.repo.join("link-in.db")).unwrap();
        for name in ["link-out.db", "link-in.db"] {
            write(&format!(
                "LN_ENGINE=sqlite\nLN_CLASS=local\nLN_PATH={}\n",
                s.repo.join(name).display()
            ));
            let l = user_profiles(&ctx).unwrap().profiles;
            let c = classify(&ctx, &l["LN"]).unwrap();
            assert_eq!((c.class, c.writable), (Class::Production, false), "{name}");
        }
        // ../ out of the repository
        write(&format!(
            "DD_ENGINE=sqlite\nDD_CLASS=local\nDD_PATH={}/../elsewhere/prod.db\n",
            s.repo.display()
        ));
        let l = user_profiles(&ctx).unwrap().profiles;
        assert!(!classify(&ctx, &l["DD"]).unwrap().writable);
    }
    write("NO_ENGINE=sqlite\nNO_PATH=/nowhere/x.db\nDIR_ENGINE=sqlite\nDIR_PATH=/tmp\n");
    let l = user_profiles(&ctx).unwrap().profiles;
    assert_eq!(classify(&ctx, &l["NO"]).unwrap_err().code, 4);
    assert_eq!(
        classify(&ctx, &l["DIR"]).unwrap_err().code,
        4,
        "a directory is not a database file"
    );
}

// ---------------------------------------------------------------- TLS fields of a profile

#[test]
fn profile_tls_unverified_follows_the_class_and_the_proof() {
    let s = Sandbox::new();
    // proven local container, published on loopback, declared local: accepted
    let (ctx, p) = container_profile(&s, "L_TLS_MODE=unverified\n");
    let eng = compose_engine(
        &s.repo,
        vec![(
            "shop-db",
            "postgres:17",
            true,
            &[("5432/tcp", "0.0.0.0", "55432")],
        )],
    );
    let c = classify(&ctx_with_engine(&ctx, &eng), &p).unwrap();
    assert!(p.tls_authorized(&c, None).is_ok());
    // not proven (the container is not running): refused, naming tls_mode
    let off = compose_engine(&s.repo, vec![("shop-db", "postgres:17", false, &[])]);
    let c = classify(&ctx_with_engine(&ctx, &off), &p).unwrap();
    let e = p.tls_authorized(&c, None).unwrap_err();
    assert_eq!(e.field(), "tls_mode");
    // declared production (or unclassified) is refused even when proven: the declaration is the owner's word
    s.write_profiles(
        "L_ENGINE=postgresql\nL_HOST=127.0.0.1\nL_CONTAINER=shop-db\nL_TLS_MODE=unverified\n",
    );
    let p = user_profiles(&ctx).unwrap().profiles["L"].clone();
    let c = classify(&ctx_with_engine(&ctx, &eng), &p).unwrap();
    assert_eq!(c.class, Class::Local);
    let e = p.tls_authorized(&c, None).unwrap_err();
    assert_eq!(e.field(), "tls_mode");
    assert!(e.to_string().contains("production"), "{e}");
    // a production profile that verifies is fine; a bad value names its field
    s.write_profiles("P_ENGINE=postgresql\nP_HOST=db.example\nP_TLS_CA_FILE=relative.pem\n");
    let p = user_profiles(&ctx).unwrap().profiles["P"].clone();
    let c = classify_with(&p, None, &ctx.env, &ctx.root).unwrap();
    assert_eq!(
        p.tls_authorized(&c, None).unwrap_err().field(),
        "tls_ca_file"
    );
    s.write_profiles("P_ENGINE=postgresql\nP_HOST=db.example\nP_TLS_MODE=disable\n");
    let p = user_profiles(&ctx).unwrap().profiles["P"].clone();
    assert_eq!(p.tls_authorized(&c, None).unwrap_err().field(), "tls_mode");
    s.write_profiles("P_ENGINE=postgresql\nP_HOST=db.example\n");
    let p = user_profiles(&ctx).unwrap().profiles["P"].clone();
    assert!(p.tls_authorized(&c, None).is_ok(), "default verify");
}

fn ctx_with_engine(ctx: &Ctx, e: &Arc<FakeEngine>) -> Ctx {
    Ctx::new(
        ctx.env.clone(),
        ctx.root.clone(),
        Some(e.clone() as Arc<dyn ContainerEngine>),
    )
}

#[test]
fn config_dir_resolution() {
    let mk = |pairs: &[(&str, &str)]| {
        Ctx::new(
            Env::from_pairs(pairs.iter().copied()),
            PathBuf::from("/r"),
            None,
        )
    };
    assert_eq!(
        mk(&[("AI_GENT_DB_CONFIG", "/c")]).config_dir(),
        PathBuf::from("/c")
    );
    assert_eq!(
        mk(&[("XDG_CONFIG_HOME", "/x"), ("HOME", "/h")]).config_dir(),
        PathBuf::from("/x/ai-gent/db")
    );
    assert_eq!(
        mk(&[("HOME", "/h")]).config_dir(),
        PathBuf::from("/h/.config/ai-gent/db")
    );
    assert_eq!(
        mk(&[("AI_GENT_DB_CONFIG", ""), ("HOME", "/h")]).config_dir(),
        PathBuf::from("/h/.config/ai-gent/db"),
        "empty means unset"
    );
}

#[test]
fn raw_profile_debug_hides_every_secret_named_field() {
    let s = Sandbox::new();
    s.write_profiles("R_ENGINE=postgresql\nR_HOST=db.internal\nR_PASSWORD=s3cr3t-value\n");
    let path = s.env_path();
    let raw = files::env_profiles(&files::read_env_file(&path).unwrap(), &path);
    let shown = format!("{:?} {:#?}", raw["R"], raw);
    assert_clean(&shown, &["s3cr3t-value"]);
    assert!(shown.contains("<redacted>") && shown.contains("postgresql"));
    let toml = files::toml_profiles("[profiles.T]\npassword = \"s3cr3t-toml\"\n", &path).unwrap();
    assert_clean(&format!("{toml:?}"), &["s3cr3t-toml"]);
}

#[test]
fn host_and_user_are_redacted_as_whole_words_not_as_substrings() {
    let s = Sandbox::new();
    s.write_profiles("H_ENGINE=postgresql\nH_HOST=private-host.internal\nH_USER=postgres\nH_PASSWORD=plain-pw-value\n");
    let p = get_profile(&s.ctx(), "H", true).unwrap();
    // The process-wide redactor must not turn ordinary words that contain the user name into tokens.
    let text = "engine 'postgresql'; image postgres:16";
    assert_eq!(ai_gent_core::passstore::redact(text), text);
    // The password is still hidden everywhere.
    assert_clean(
        &ai_gent_core::passstore::redact("failed with plain-pw-value"),
        &["plain-pw-value"],
    );
    // Host and user are offered with labels for the whole-word redactor.
    let pairs = p.redaction_pairs();
    assert!(
        pairs
            .iter()
            .any(|(v, l)| v == "private-host.internal" && *l == "<host>")
    );
    assert!(pairs.iter().any(|(v, l)| v == "postgres" && *l == "<user>"));
}

#[test]
#[cfg(unix)]
fn a_fifo_in_place_of_a_profiles_file_is_refused_without_blocking() {
    let s = Sandbox::new();
    let fifo = s.base.join("team.toml");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let ctx = s.ctx();
    let mut env = ctx.env.clone();
    env.set("AI_GENT_DB_CONNECTIONS", fifo.to_str().unwrap());
    let e = user_profiles(&Ctx::new(env, ctx.root, None)).unwrap_err();
    assert!(e.to_string().contains("not a regular file"), "{e}");
}
