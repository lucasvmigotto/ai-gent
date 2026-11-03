//! Agent delegation (spec 041): the adapter for a delegate agent (Codex first), the brief builder, the
//! supervisor's diff checks, the status files and the delegation log.
//!
//! Nothing here decides what to build. The skill (a later task) calls `ai-gent delegate ...`; no hook
//! or skill is switched to the binary yet (decision D7).
pub mod adapter;
pub mod brief;
pub mod checks;
pub mod claude;
pub mod codex;
pub mod commit;
pub mod diff;
pub mod events;
pub mod gitx;
pub mod glob;
pub mod opencode;
pub mod schedule;
pub mod sources;
pub mod state;
pub mod status;
pub mod watch;

use std::path::PathBuf;

/// What went wrong in a delegate operation.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An input does not satisfy a contract (a track id, a brief field, a status field).
    #[error("{0}")]
    Invalid(String),
    /// A file or process operation failed.
    #[error("{context}: {source}")]
    Io {
        /// What was being done.
        context: String,
        /// The cause.
        source: std::io::Error,
    },
    /// The agent is not logged in (its own login; never worked around).
    #[error("{0} is not logged in")]
    NotLoggedIn(String),
    /// The agent produced no event in time and was stopped.
    #[error("no first event from the agent within {0}s; stopped")]
    NoFirstEvent(u64),
    /// A policy refused the operation (a path outside `allowed_paths`, a marker in a message).
    #[error("{0}")]
    Refused(String),
    /// A git command failed.
    #[error("git {args}: {message}")]
    Git {
        /// The arguments.
        args: String,
        /// Its stderr, first line.
        message: String,
    },
}

/// Result of a delegate operation.
pub type Res<T> = Result<T, Error>;

pub(crate) fn io<T>(context: impl Into<String>, r: std::io::Result<T>) -> Res<T> {
    r.map_err(|source| Error::Io {
        context: context.into(),
        source,
    })
}

/// Seconds since the Unix epoch.
#[must_use]
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A path as text with forward slashes, for files that are compared or written.
#[must_use]
pub fn slash(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// `p` relative to `root` when it is under it, else `p` (both as given).
#[must_use]
pub fn rel_to(root: &std::path::Path, p: &std::path::Path) -> PathBuf {
    p.strip_prefix(root).unwrap_or(p).to_path_buf()
}

#[cfg(test)]
pub(crate) mod tests_support {
    use std::path::PathBuf;

    /// The fixtures directory, `rust/fixtures/delegate`.
    pub fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/delegate")
    }

    /// Compare `got` with the golden file `name` (rewritten when `UPDATE_GOLDEN` is set).
    pub fn golden(name: &str, got: &str) {
        let path = fixtures().join(name);
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, got).unwrap();
        }
        let want = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{}: {e} (UPDATE_GOLDEN=1 writes it)", path.display()));
        assert_eq!(got, want.replace("\r\n", "\n"), "golden {name}");
    }
}
