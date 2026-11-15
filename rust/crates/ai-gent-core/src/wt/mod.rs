//! `ai-gent wt`: the port of `plugins/agents/scripts/wt.py` (spec 036).
//!
//! Worktrees for `agents:parallel`: one branch per track under `.worktrees/`, kept out of git, with a
//! compose project name, a port offset and ownership labels, plus the container half (disk floor,
//! leftovers, dangling images). Parity with the Python tool is the contract until the 3.0.0 cutover
//! (decision D7). git and the container engine are subprocesses, as in Python. Nothing here prunes:
//! container resources go by ID or by this project's label (`shared/containers.md` section 9a).
//!
//! Paths are kept as `String`s with `/` separators (what git prints, also on Windows) so comparisons
//! and the JSON written match the Python output.
// A line-for-line port of a Python tool whose arithmetic (bytes as u64, GB as f64) is part of the parity
// contract; these pedantic lints would only obscure the mapping.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    clippy::many_single_char_names,
    clippy::collapsible_if,
    clippy::semicolon_if_nothing_returned
)]
pub mod commands;
pub mod disk;
pub mod engine;
pub mod git;
pub mod ports;
pub mod procs;
pub mod readme;

use crate::tokens::util::fmt_g;

/// The directory holding the worktrees, under the main checkout.
pub const DIR: &str = ".worktrees";
/// Gap between the port offsets of two live tracks.
pub const PORT_STEP: i64 = 100;
/// Label naming the project.
pub const LABEL_PROJECT: &str = "me.lucasvmigotto.ai-gent.project";
/// Label naming `<project>/<track>`.
pub const LABEL_WORKTREE: &str = "me.lucasvmigotto.ai-gent.worktree";
/// The compose project label.
pub const COMPOSE_LABEL: &str = "com.docker.compose.project";
/// A gigabyte (decimal, as in the Python tool).
pub const GB: f64 = 1e9;

/// A refusal or failure: printed as `error: <message>` on stderr, exit 1.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Failed(pub String);

/// Result of a `wt` operation.
pub type Res<T> = Result<T, Failed>;

/// What a command prints; the caller writes both streams (stdout first).
#[derive(Debug, Default)]
pub struct Out {
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
}

impl Out {
    /// A line on stdout.
    pub fn line(&mut self, s: impl AsRef<str>) {
        self.stdout.push_str(s.as_ref());
        self.stdout.push('\n');
    }

    /// A line on stderr.
    pub fn eline(&mut self, s: impl AsRef<str>) {
        self.stderr.push_str(s.as_ref());
        self.stderr.push('\n');
    }
}

/// The lowercased branch with runs of non-`[a-z0-9]` collapsed to `-`, trimmed of `-`.
#[must_use]
pub fn slug(branch: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in branch.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if gap && !out.is_empty() {
                out.push('-');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    out
}

/// The project name: the slug of the main checkout's directory name.
#[must_use]
pub fn project(root: &str) -> String {
    slug(root.rsplit('/').next().unwrap_or(""))
}

/// Settings from `.ai-gent/parallel.yaml` over the defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Every `key: number` found, defaults included.
    pub values: Vec<(String, f64)>,
}

impl Settings {
    /// A value, if set.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<f64> {
        self.values
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| *v)
    }

    /// `max_volumes`, else `max_parallel`.
    #[must_use]
    pub fn volume_cap(&self) -> i64 {
        self.get("max_volumes")
            .or_else(|| self.get("max_parallel"))
            .unwrap_or(3.0) as i64
    }

    /// `volume_estimate_gb`.
    #[must_use]
    pub fn volume_estimate_gb(&self) -> f64 {
        self.get("volume_estimate_gb").unwrap_or(10.0)
    }
}

/// Flat `key: number` lines of `.ai-gent/parallel.yaml` over the defaults.
#[must_use]
pub fn settings(root: &str) -> Settings {
    let mut values = vec![
        ("max_parallel".to_string(), 3.0),
        ("volume_estimate_gb".to_string(), 10.0),
    ];
    let path = std::path::Path::new(root)
        .join(".ai-gent")
        .join("parallel.yaml");
    let Ok(bytes) = std::fs::read(path) else {
        return Settings { values };
    };
    for line in String::from_utf8_lossy(&bytes).lines() {
        if let Some((k, v)) = parse_setting(line) {
            values.push((k, v));
        }
    }
    Settings { values }
}

fn parse_setting(line: &str) -> Option<(String, f64)> {
    let rest = line.trim_start();
    let key_end = rest
        .find(|c: char| !(c.is_ascii_lowercase() || c == '_'))
        .unwrap_or(rest.len());
    if key_end == 0 {
        return None;
    }
    let (key, rest) = rest.split_at(key_end);
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let num_end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    if num_end == 0 {
        return None;
    }
    let (num, rest) = rest.split_at(num_end);
    let rest = rest.trim_start();
    if !(rest.is_empty() || rest.starts_with('#')) {
        return None;
    }
    Some((key.to_string(), num.parse().ok()?))
}

/// `{x:g}` of the Python tool.
#[must_use]
pub fn g(x: f64) -> String {
    fmt_g(x)
}

/// Pad on the right to `width` characters (`f"{s:<width}"`).
#[must_use]
pub fn pad(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - n))
    }
}

/// The last `n` characters.
#[must_use]
pub fn tail(s: &str, n: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    c[c.len().saturating_sub(n)..].iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_collapse_and_trim() {
        assert_eq!(slug("feat/002-Orgs"), "feat-002-orgs");
        assert_eq!(slug("--A..b__"), "a-b");
        assert_eq!(slug("///"), "");
    }

    #[test]
    fn settings_lines() {
        assert_eq!(
            parse_setting("max_volumes: 1"),
            Some(("max_volumes".into(), 1.0))
        );
        assert_eq!(
            parse_setting("  volume_estimate_gb : 1.5 # small"),
            Some(("volume_estimate_gb".into(), 1.5))
        );
        assert_eq!(parse_setting("key: yes"), None);
        assert_eq!(parse_setting("# comment"), None);
        assert_eq!(parse_setting("a: 1 b"), None);
    }

    #[test]
    fn project_is_the_directory_slug() {
        assert_eq!(project("/tmp/My Shop"), "my-shop");
    }
}
