//! How the runners read and write secrets in pass (`shared/secrets.md`; a port of
//! `shared/passstore.py`, spec 037 Group A).
//!
//! The store is `PASSWORD_STORE_DIR` (default `~/.password-store`). Values are never printed:
//! callers get a [`Secret`] back, and [`redact`] hides every value read in this process.
//! Backends: `pass` first (the contract), then `gopass` when `pass` is not on `PATH`.
mod gopass;
mod pass;
pub mod redact;

pub use redact::{Secret, redact};

use crate::env::Env;
use std::path::PathBuf;
use std::time::Duration;

/// A read or write that could not be done; the message never holds a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError(String);

impl StoreError {
    pub(crate) fn new(m: impl Into<String>) -> Self {
        Self(m.into())
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StoreError {}

/// What the module needs from the environment, explicit so tests never touch the process's.
#[derive(Debug, Clone)]
pub struct Config {
    env: Env,
    /// Seconds a `read` waits for the passphrase (90 by default).
    pub read_timeout: Duration,
    /// Seconds a `write` waits (60 by default).
    pub write_timeout: Duration,
}

impl Config {
    /// From an environment snapshot.
    #[must_use]
    pub fn new(env: Env) -> Self {
        Self {
            env,
            read_timeout: Duration::from_secs(90),
            write_timeout: Duration::from_secs(60),
        }
    }

    /// From the process environment.
    #[must_use]
    pub fn from_process() -> Self {
        Self::new(Env::real())
    }

    pub(crate) fn child_env(&self) -> Env {
        self.env.clone()
    }

    /// `AI_GENT_PASS_PREFIX` (default `ai-gent`), without surrounding `/`.
    #[must_use]
    pub fn prefix(&self) -> String {
        self.env
            .text("AI_GENT_PASS_PREFIX")
            .unwrap_or_else(|| "ai-gent".to_string())
            .trim_matches('/')
            .to_string()
    }

    /// `PASSWORD_STORE_DIR`, else `~/.password-store`.
    #[must_use]
    pub fn store(&self) -> PathBuf {
        self.env.path("PASSWORD_STORE_DIR").unwrap_or_else(|| {
            self.env
                .home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".password-store")
        })
    }
}

/// A chosen backend: its program path and flavour.
#[derive(Debug, Clone)]
pub(crate) struct Backend {
    pub(crate) path: PathBuf,
    pub(crate) name: &'static str,
}

impl Backend {
    pub(crate) fn show_args(&self, entry: &str) -> Vec<String> {
        if self.name == "gopass" {
            gopass::show_args(entry)
        } else {
            vec!["show".into(), entry.into()]
        }
    }

    pub(crate) fn insert_args(&self, entry: &str) -> Vec<String> {
        if self.name == "gopass" {
            gopass::insert_args(entry)
        } else {
            vec!["insert".into(), "--multiline".into(), entry.into()]
        }
    }
}

fn select(cfg: &Config) -> Option<Backend> {
    if let Some(path) = crate::exec::which("pass", &cfg.env) {
        return Some(Backend { path, name: "pass" });
    }
    crate::exec::which("gopass", &cfg.env).map(|path| Backend {
        path,
        name: "gopass",
    })
}

fn segment_ok(seg: &str) -> bool {
    let mut chars = seg.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '@' | '+'));
    first_ok && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '@' | '+' | '-'))
}

/// Validate an entry name (`ENTRY_RE`, no `..`, no leading `-` or `/`).
///
/// # Errors
/// When the name is not a pass entry name.
pub fn check(entry: &str) -> Result<&str, StoreError> {
    if entry.split('/').all(segment_ok) && !entry.split('/').any(|s| s == "..") {
        Ok(entry)
    } else {
        Err(StoreError::new(format!(
            "{entry:?} isn't a pass entry name (no leading - or /, no ..)"
        )))
    }
}

fn has_with(cfg: &Config, entry: &str) -> bool {
    let store = cfg.store();
    ["gpg", "age"]
        .iter()
        .any(|ext| store.join(format!("{entry}.{ext}")).is_file())
}

/// Whether the entry exists: a file check, nothing decrypted.
///
/// # Errors
/// When the name is invalid.
pub fn has(cfg: &Config, entry: &str) -> Result<bool, StoreError> {
    check(entry)?;
    Ok(has_with(cfg, entry))
}

/// The secret (first line) and the entry's other lines. The value is registered for redaction.
///
/// # Errors
/// When the name is invalid, no backend is installed, or the read fails (locked, missing, empty,
/// timed out). The message never holds a value.
pub fn read(cfg: &Config, entry: &str) -> Result<(Secret, Vec<String>), StoreError> {
    check(entry)?;
    let Some(backend) = select(cfg) else {
        return Err(StoreError::new(format!(
            "{entry} is meant to be in pass, but pass isn't installed"
        )));
    };
    let unlock = format!(
        "If the store is locked, unlock it in your own terminal (`pass show {entry} >/dev/null`) and retry"
    );
    let lines = pass::show(cfg, &backend, entry).map_err(|why| {
        StoreError::new(format!("couldn't read {entry} from pass ({why}). {unlock}"))
    })?;
    let secret = lines.first().map_or("", |l| l.trim()).to_string();
    if secret.is_empty() {
        return Err(StoreError::new(format!(
            "couldn't read {entry} from pass (it's empty). {unlock}"
        )));
    }
    let secret = Secret::new(secret);
    secret.register();
    Ok((secret, lines.into_iter().skip(1).collect()))
}

/// Add a new entry with `pass insert` (never overwrites).
///
/// # Errors
/// When the name is invalid, pass is missing, the entry exists, or the insert fails.
pub fn write(cfg: &Config, entry: &str, value: &str) -> Result<(), StoreError> {
    check(entry)?;
    let Some(backend) = select(cfg) else {
        return Err(StoreError::new("pass isn't installed; see secrets:pass"));
    };
    if has_with(cfg, entry) {
        return Err(StoreError::new(format!("{entry} already exists")));
    }
    pass::insert(cfg, &backend, entry, value)
}
