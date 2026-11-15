//! The five rule sets, ported from `plugins/*/hooks/guard.sh`.
use regex::Regex;

pub mod containers;
pub mod db;
pub mod git;
pub mod repo;
pub mod secrets;

/// Compile a pattern that is part of the source.
///
/// # Panics
/// When the pattern is invalid (a bug the tests catch).
pub(crate) fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("static pattern")
}

/// Whether some segment matches.
pub(crate) fn any(segments: &[String], r: &Regex) -> bool {
    segments.iter().any(|s| r.is_match(s))
}

/// The attribution check the git and repo guards share (a trailer or a signature, not prose
/// about one). Case-insensitive on ASCII.
pub(crate) fn attributed(raw: &str) -> bool {
    re(r"co-authored-by[[:space:]]*:|generated[[:space:]]+(with|by)[[:space:]]{0,3}\[?claude")
        .is_match(&raw.to_ascii_lowercase())
}

/// The command-word prefix of the db and secrets guards: sudo, env, exec, nice, timeout and
/// variable assignments.
pub(crate) const PREFIX: &str = r"^[[:space:]]*((sudo([[:space:]]+-[^[:space:]]+([[:space:]]+[^\-[:space:]][^[:space:]]*)?)*|env|exec|nice|timeout[[:space:]]+[0-9smh]+|[A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*)[[:space:]]+)*";
