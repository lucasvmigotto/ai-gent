//! `passstore` against a fake `pass` / `gopass` executable and a temp store (spec 037 Group A).
//! Fixtures from `EntryTest::test_entry_rules`, `::test_notes`, `PassTest` (sources in order, prefix
//! override, bad or unreadable entry), plus gopass.
use ai_gent_core::env::Env;
use ai_gent_core::passstore::{self, Config};
use std::path::{Path, PathBuf};

fn fake_tool() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().unwrap().parent().unwrap().join("examples");
    let name = if cfg!(windows) {
        "fake_tool.exe"
    } else {
        "fake_tool"
    };
    let p = dir.join(name);
    {
        // `cargo test --test passstore` alone does not build examples; `cargo test` does. A no-op when current.
        static BUILD: std::sync::Once = std::sync::Once::new();
        BUILD.call_once(|| {
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
            let ok = std::process::Command::new(cargo)
                .args([
                    "build",
                    "--example",
                    "fake_tool",
                    "-p",
                    "ai-gent-core",
                    "--locked",
                ])
                .status()
                .is_ok_and(|s| s.success());
            assert!(ok, "could not build the fake_tool example");
        });
    }
    assert!(p.is_file(), "missing {}", p.display());
    p
}

struct Sandbox {
    _tmp: tempfile::TempDir,
    store: PathBuf,
    bin: PathBuf,
}

impl Sandbox {
    fn new(tools: &[&str]) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().canonicalize().unwrap();
        let base = PathBuf::from(base.to_string_lossy().trim_start_matches(r"\\?\"));
        let store = base.join("store");
        let bin = base.join("bin");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        for t in tools {
            let dest = bin.join(if cfg!(windows) {
                format!("{t}.exe")
            } else {
                (*t).to_string()
            });
            std::fs::copy(fake_tool(), dest).unwrap();
        }
        Self {
            _tmp: tmp,
            store,
            bin,
        }
    }

    fn entry(&self, name: &str, text: &str, ext: &str) {
        let f = self.store.join(format!("{name}.{ext}"));
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, text).unwrap();
    }

    fn cfg(&self, extra: &[(&str, &str)]) -> Config {
        let mut env = Env::from_pairs([
            ("PASSWORD_STORE_DIR", self.store.to_str().unwrap()),
            ("PATH", self.bin.to_str().unwrap()),
            ("HOME", self.store.to_str().unwrap()),
        ]);
        for (k, v) in extra {
            env.set(k, *v);
        }
        Config::new(env)
    }
}

const TOKEN: &str = "tok-3f9a1c77e0";

#[test]
fn entry_rules() {
    for bad in ["../x", "-x", "/x", "a/../b", "", "a//b", "a b"] {
        assert!(passstore::check(bad).is_err(), "{bad:?}");
    }
    for good in ["ai-gent/github/pat", "a_b/c.d/e@f+g", "x"] {
        assert_eq!(passstore::check(good).unwrap(), good);
    }
    let e = passstore::check("../x").unwrap_err().to_string();
    assert!(e.contains("isn't a pass entry name"));
}

#[test]
fn has_is_a_file_check_and_never_runs_pass() {
    let s = Sandbox::new(&[]); // no pass at all
    s.entry("ai-gent/github/pat", TOKEN, "gpg");
    s.entry("ai-gent/age-one", TOKEN, "age");
    let cfg = s.cfg(&[]);
    assert!(passstore::has(&cfg, "ai-gent/github/pat").unwrap());
    assert!(passstore::has(&cfg, "ai-gent/age-one").unwrap());
    assert!(!passstore::has(&cfg, "ai-gent/gitlab/pat").unwrap());
    assert!(passstore::has(&cfg, "../x").is_err());
}

#[test]
fn read_gives_secret_and_notes_and_registers_for_redaction() {
    let s = Sandbox::new(&["pass"]);
    let value = "canary-read-test-0001";
    s.entry(
        "ai-gent/jira/token",
        &format!("{value}\r\nemail: me@example.com\nurl: https://x/\n"),
        "gpg",
    );
    let (secret, notes) = passstore::read(&s.cfg(&[]), "ai-gent/jira/token").unwrap();
    assert_eq!(secret.expose(), value);
    assert_eq!(notes, vec!["email: me@example.com", "url: https://x/"]);
    assert_eq!(passstore::redact(&format!("a {value} b")), "a <token> b");
    assert!(!format!("{secret:?}").contains(value));
}

#[test]
fn bad_or_unreadable_entry_message_has_no_value() {
    let s = Sandbox::new(&["pass"]);
    let cfg = s.cfg(&[]);
    let e = passstore::read(&cfg, "../x").unwrap_err().to_string();
    assert!(e.contains("isn't a pass entry name"));
    let e = passstore::read(&cfg, "ai-gent/github/pat")
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("couldn't read ai-gent/github/pat from pass"),
        "{e}"
    );
    assert!(e.contains("is not in the password store"), "{e}");
    assert!(
        e.ends_with(
            "unlock it in your own terminal (`pass show ai-gent/github/pat >/dev/null`) and retry"
        ),
        "{e}"
    );
    s.entry("ai-gent/empty", "\n", "gpg");
    let e = passstore::read(&cfg, "ai-gent/empty")
        .unwrap_err()
        .to_string();
    assert!(e.contains("(it's empty)"), "{e}");
}

#[test]
fn pass_missing_is_reported() {
    let s = Sandbox::new(&[]);
    let e = passstore::read(&s.cfg(&[]), "ai-gent/github/pat")
        .unwrap_err()
        .to_string();
    assert_eq!(
        e,
        "ai-gent/github/pat is meant to be in pass, but pass isn't installed"
    );
    let e = passstore::write(&s.cfg(&[]), "ai-gent/github/pat", "v")
        .unwrap_err()
        .to_string();
    assert_eq!(e, "pass isn't installed; see secrets:pass");
}

#[test]
fn read_times_out_with_the_passphrase_message() {
    let s = Sandbox::new(&["pass"]);
    s.entry("ai-gent/github/pat", TOKEN, "gpg");
    let mut cfg = s.cfg(&[("FAKE_TOOL_SLEEP", "5")]);
    cfg.read_timeout = std::time::Duration::from_millis(300);
    let e = passstore::read(&cfg, "ai-gent/github/pat")
        .unwrap_err()
        .to_string();
    assert!(e.contains("timed out waiting for the passphrase"), "{e}");
}

#[test]
fn prefix_override() {
    let s = Sandbox::new(&[]);
    assert_eq!(s.cfg(&[]).prefix(), "ai-gent");
    assert_eq!(
        s.cfg(&[("AI_GENT_PASS_PREFIX", "/team/x/")]).prefix(),
        "team/x"
    );
    assert_eq!(s.cfg(&[]).store(), s.store);
}

#[test]
fn write_never_overwrites_and_reports_failure() {
    let s = Sandbox::new(&["pass"]);
    let cfg = s.cfg(&[]);
    passstore::write(&cfg, "ai-gent/github/pat", "canary-write-test-0002").unwrap();
    assert_eq!(
        std::fs::read_to_string(s.store.join("ai-gent/github/pat.gpg")).unwrap(),
        "canary-write-test-0002\n"
    );
    let e = passstore::write(&cfg, "ai-gent/github/pat", "x")
        .unwrap_err()
        .to_string();
    assert_eq!(e, "ai-gent/github/pat already exists");
    let bad = s.cfg(&[("FAKE_TOOL_FAIL_INSERT", "1")]);
    let e = passstore::write(&bad, "ai-gent/gitlab/pat", "x")
        .unwrap_err()
        .to_string();
    assert!(
        e.starts_with("pass insert ai-gent/gitlab/pat failed"),
        "{e}"
    );
}

#[test]
fn gopass_is_used_only_without_pass() {
    let s = Sandbox::new(&["gopass"]);
    s.entry(
        "ai-gent/github/pat",
        &format!("{TOKEN}\nemail: a@b\n"),
        "age",
    );
    let (secret, notes) = passstore::read(&s.cfg(&[]), "ai-gent/github/pat").unwrap();
    assert_eq!(secret.expose(), TOKEN);
    assert_eq!(notes, vec!["email: a@b"]);
    // Both present: pass wins (the contract). The fake pass reads .gpg only, so an .age entry is not found.
    let both = Sandbox::new(&["pass", "gopass"]);
    both.entry("ai-gent/github/pat", TOKEN, "age");
    let e = passstore::read(&both.cfg(&[]), "ai-gent/github/pat")
        .unwrap_err()
        .to_string();
    assert!(e.contains("from pass"), "{e}");
    both.entry("ai-gent/github/pat", TOKEN, "gpg");
    assert!(passstore::read(&both.cfg(&[]), "ai-gent/github/pat").is_ok());
}

#[test]
fn only_the_fake_tool_example_is_needed() {
    assert!(Path::new(&fake_tool()).exists());
}
