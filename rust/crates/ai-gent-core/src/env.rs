//! A snapshot of the environment, so tests build their own instead of touching the process's.
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;

/// Environment variables, by name.
#[derive(Debug, Clone, Default)]
pub struct Env {
    vars: HashMap<String, OsString>,
}

impl Env {
    /// The process environment now.
    #[must_use]
    pub fn real() -> Self {
        Self {
            vars: std::env::vars_os()
                .filter_map(|(k, v)| k.into_string().ok().map(|k| (k, v)))
                .collect(),
        }
    }

    /// An environment of exactly these pairs.
    #[must_use]
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        Self {
            vars: pairs
                .into_iter()
                .map(|(k, v)| (k.to_string(), OsString::from(v)))
                .collect(),
        }
    }

    /// Set a variable.
    pub fn set(&mut self, key: &str, value: impl Into<OsString>) {
        self.vars.insert(key.to_string(), value.into());
    }

    /// A non-empty variable as a path.
    #[must_use]
    pub fn path(&self, key: &str) -> Option<PathBuf> {
        self.vars
            .get(key)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    }

    /// A non-empty variable as text.
    #[must_use]
    pub fn text(&self, key: &str) -> Option<String> {
        self.vars
            .get(key)
            .filter(|v| !v.is_empty())
            .and_then(|v| v.to_str().map(str::to_string))
    }

    /// Whether the variable is set at all (even empty), as the shell's `${VAR+x}` would say.
    #[must_use]
    pub fn is_set(&self, key: &str) -> bool {
        self.vars.contains_key(key)
    }

    /// The user's home directory.
    #[must_use]
    pub fn home_dir(&self) -> Option<PathBuf> {
        if cfg!(windows) {
            self.path("USERPROFILE").or_else(|| self.path("HOME"))
        } else {
            self.path("HOME")
        }
    }

    /// The install root: `$AI_GENT_HOME`, else the per-OS default.
    #[must_use]
    pub fn install_root(&self) -> Option<PathBuf> {
        crate::paths::root_from(|k| self.path(k))
    }

    /// The directory of the update cache and the shadow log.
    #[must_use]
    pub fn state_dir(&self) -> Option<PathBuf> {
        crate::guard::shadow::log_dir(|k| self.path(k))
    }
}
