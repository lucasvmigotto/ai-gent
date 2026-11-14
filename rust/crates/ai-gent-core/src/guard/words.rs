//! The guards' own words and log rows (specs 015 and 024, as decided 2026-11-14).
//!
//! A guard prints its decision with the plain words a person reads (`systemMessage`) and, on a
//! deny, one line of context for the model (`additionalContext`), and appends one row per deny
//! or ask to the session's guard log, which the `project` plugin's `/guards` reads. The table is
//! `words.json`, a copy of `shared/guard-words.json` (kept equal by `scripts/consistency.py`); the
//! shell fallback in `plugins/*/hooks/guard.sh` and `opencode/guard/rules.mjs` do the same.
use super::event::{Decision, Level};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const RAW: &str = include_str!("words.json");

/// One entry of a guard's table.
#[derive(Debug, Deserialize)]
pub struct Entry {
    /// A fragment of the reason that stays the same when a value in it changes.
    #[serde(rename = "match")]
    pub matches: String,
    /// One sentence for the person.
    pub say: String,
    /// A deny: what to do instead.
    pub instead: Option<String>,
    /// An ask: when allowing is right.
    #[serde(rename = "allowIf")]
    pub allow_if: Option<String>,
}

/// One guard's table.
#[derive(Debug, Deserialize)]
pub struct Guard {
    /// The prefix of each of its reasons.
    pub marker: String,
    /// The skill that explains the rule.
    pub skill: String,
    /// Its entries, first match wins.
    pub entries: Vec<Entry>,
}

/// The whole file.
#[derive(Debug, Deserialize)]
pub struct Table {
    /// Rows a log keeps.
    #[serde(rename = "maxRows")]
    pub max_rows: usize,
    /// By guard id.
    pub guards: BTreeMap<String, Guard>,
}

/// The table.
///
/// # Panics
/// Never: the file is checked by the unit tests.
pub fn table() -> &'static Table {
    static CELL: OnceLock<Table> = OnceLock::new();
    CELL.get_or_init(|| serde_json::from_str(RAW).expect("words.json is valid"))
}

/// The guard a reason belongs to, its entry (if any) and the text after its marker.
fn find(reason: &str) -> Option<(&'static str, &'static Guard, Option<&'static Entry>, &str)> {
    table().guards.iter().find_map(|(id, g)| {
        let at = reason.find(&g.marker)?;
        let text = reason[at + g.marker.len()..].trim();
        let entry = g.entries.iter().find(|e| text.contains(&e.matches));
        Some((id.as_str(), g, entry, text))
    })
}

/// What a decision says beyond its reason.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Words {
    /// For the person (`systemMessage`).
    pub message: Option<String>,
    /// For the model, on a deny (`additionalContext`).
    pub context: Option<String>,
}

/// The words for a deny or an ask; none for a reason no guard owns.
#[must_use]
pub fn words(d: &Decision) -> Words {
    let Some((_, g, entry, text)) = find(&d.reason) else {
        return Words::default();
    };
    let name = g.marker.trim_end_matches(':');
    match d.level {
        Level::Deny => {
            let why = entry.map_or_else(
                || text.to_string(),
                |e| match &e.instead {
                    Some(i) => format!("{} Instead: {i}", e.say),
                    None => e.say.clone(),
                },
            );
            Words {
                message: Some(format!("{name} refused this. {why} ({})", g.skill)),
                context: entry
                    .and_then(|e| e.instead.as_ref())
                    .map(|i| format!("{name}: instead, {i}")),
            }
        }
        Level::Ask => Words {
            message: entry.map(|e| match &e.allow_if {
                Some(a) => format!("{name} asks: {} {a} ({})", e.say, g.skill),
                None => format!("{name} asks: {} ({})", e.say, g.skill),
            }),
            context: None,
        },
        Level::None => Words::default(),
    }
}

/// The hook line with the words added (`None` for no decision).
#[must_use]
pub fn hook_line(d: &Decision, w: &Words) -> Option<String> {
    if d.level == Level::None {
        return None;
    }
    let mut specific = json!({
        "hookEventName": "PreToolUse",
        "permissionDecision": d.level.name(),
        "permissionDecisionReason": d.reason,
    });
    if let Some(c) = &w.context {
        specific["additionalContext"] = json!(c);
    }
    let mut out = json!({ "hookSpecificOutput": specific });
    if let Some(m) = &w.message {
        out["systemMessage"] = json!(m);
    }
    Some(out.to_string())
}

/// The verb of a Bash command: the first word (a path reduced to its last segment), only a plain
/// lowercase word with fewer than seven digits; for git, gh, pass, podman and docker also a plain
/// second word. Never a value, an argument or a path.
#[must_use]
pub fn verb(command: Option<&str>, tool: &str) -> Option<String> {
    if tool != "Bash" {
        return None;
    }
    let mut words = command?.split_whitespace();
    let raw = words.next().unwrap_or("");
    if raw.contains("://") {
        return None;
    }
    let first = raw.rsplit('/').next().unwrap_or("");
    let first_ok = first.len() <= 24
        && first.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && first.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_' || c == '-'
        })
        && first.chars().filter(char::is_ascii_digit).count() < 7;
    if !first_ok {
        return None;
    }
    let second = words.next().unwrap_or("");
    let second_ok = second.len() <= 24
        && second
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase())
        && second.chars().all(|c| c.is_ascii_lowercase() || c == '-');
    if matches!(first, "git" | "gh" | "pass" | "podman" | "docker") && second_ok {
        Some(format!("{first} {second}"))
    } else {
        Some(first.to_string())
    }
}

fn tool_name(tool: &str) -> &str {
    let ok = tool.len() <= 64
        && tool.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && tool
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'));
    if ok { tool } else { "unknown" }
}

/// The log row of a deny or ask a guard owns: the entry's `match` (or `generic`) and the verb,
/// never the reason, the command or a path.
#[must_use]
pub fn row(d: &Decision, tool: &str, command: Option<&str>, at_ms: u64) -> Option<Value> {
    if d.level == Level::None {
        return None;
    }
    let (id, _, entry, _) = find(&d.reason)?;
    let tool = tool_name(tool);
    Some(json!({
        "at": at_ms,
        "guard": id,
        "decision": d.level.name(),
        "entry": entry.map_or("generic", |e| e.matches.as_str()),
        "tool": tool,
        "verb": verb(command, tool),
        "outcome": if d.level == Level::Ask { Some("asked") } else { None },
    }))
}

/// The guard log's directory: `AI_GENT_GUARD_LOG_DIR`, else `${XDG_STATE_HOME}/ai-gent/guard-log`,
/// `~/.local/state/ai-gent/guard-log`, `%LOCALAPPDATA%\ai-gent\state\guard-log`.
#[must_use]
pub fn log_dir(get: impl Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    if let Some(p) = get("AI_GENT_GUARD_LOG_DIR") {
        return Some(p);
    }
    if let Some(p) = get("XDG_STATE_HOME") {
        return Some(p.join("ai-gent").join("guard-log"));
    }
    if let Some(p) = get("HOME") {
        return Some(p.join(".local/state/ai-gent/guard-log"));
    }
    get("LOCALAPPDATA").map(|p| p.join("ai-gent").join("state").join("guard-log"))
}

/// A session id safe as a file name.
#[must_use]
pub fn is_session_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Append a row to `<dir>/<session>.jsonl` (0700 directory, 0600 file on Unix); past twice the
/// row limit the file is cut to the newest rows.
///
/// # Errors
/// When the directory or file cannot be created or written.
pub fn append(dir: &Path, session: &str, row: &Value) -> std::io::Result<()> {
    if !is_session_id(session) {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{session}.jsonl"));
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut line = row.to_string();
    line.push('\n');
    opts.open(&path)?.write_all(line.as_bytes())?;
    let max = table().max_rows;
    let text = std::fs::read_to_string(&path)?;
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() > 2 * max {
        let tmp = dir.join(format!("{session}.jsonl.{}", std::process::id()));
        std::fs::write(&tmp, lines[lines.len() - max..].join("\n") + "\n")?;
        std::fs::rename(&tmp, &path)?;
    }
    Ok(())
}

/// Whether a plugin option is set to false in the hook's environment (`CLAUDE_PLUGIN_OPTION_<key>`,
/// as written or upper-cased).
#[must_use]
pub fn option_off(get: impl Fn(&str) -> Option<String>, key: &str) -> bool {
    [key.to_string(), key.to_uppercase()]
        .iter()
        .any(|k| get(&format!("CLAUDE_PLUGIN_OPTION_{k}")).as_deref() == Some("false"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard::reasons::decide;

    #[test]
    fn every_reason_has_its_guard() {
        for r in crate::guard::reasons::all() {
            let g = &table().guards[r.guard.as_str()];
            assert_eq!(g.marker, r.marker, "{}", r.id);
            let text = &r.text;
            assert!(
                g.entries.iter().any(|e| text.contains(&e.matches)),
                "{} has no entry",
                r.id
            );
        }
    }

    #[test]
    fn deny_and_ask_words() {
        let d = decide("git.nohook", &[]);
        let w = words(&d);
        assert_eq!(
            w.message.as_deref(),
            Some(
                "git:workflow guard refused this. Hooks are never bypassed (--no-verify, -n). Instead: fix what the hook reports, then commit again. (git:workflow)"
            )
        );
        assert_eq!(
            w.context.as_deref(),
            Some("git:workflow guard: instead, fix what the hook reports, then commit again.")
        );
        let a = decide("git.push", &[]);
        let w = words(&a);
        assert_eq!(
            w.message.as_deref(),
            Some(
                "git:workflow guard asks: This publishes commits to the remote. Allow only if you asked for this push. (git:workflow)"
            )
        );
        assert_eq!(w.context, None);
        let line: Value = serde_json::from_str(&hook_line(&a, &w).unwrap()).unwrap();
        assert_eq!(line["hookSpecificOutput"]["permissionDecision"], "ask");
        assert_eq!(
            line["hookSpecificOutput"]["permissionDecisionReason"],
            a.reason
        );
        assert!(line["systemMessage"].is_string());
        assert_eq!(hook_line(&Decision::none(), &Words::default()), None);
    }

    #[test]
    fn rows_hold_the_entry_and_verb_only() {
        let d = decide("git.on_main", &[("branch", "main")]);
        let r = row(&d, "Bash", Some("git commit -m x"), 7).unwrap();
        assert_eq!(
            r,
            json!({"at": 7, "guard": "git", "decision": "ask", "entry": "the workflow works on a branch", "tool": "Bash", "verb": "git commit", "outcome": "asked"})
        );
        assert!(!r.to_string().contains("main"));
        let n = Decision {
            level: Level::Deny,
            reason: "db guard: something new".into(),
            rule: "",
        };
        assert_eq!(row(&n, "Read", None, 1).unwrap()["entry"], "generic");
        assert_eq!(row(&n, "bad tool!", None, 1).unwrap()["tool"], "unknown");
    }

    #[test]
    fn the_verb_never_holds_a_value() {
        let token = format!("ghp_{}", "a1b2c3d4e5".repeat(4));
        assert_eq!(verb(Some(&format!("{token} push")), "Bash"), None);
        assert_eq!(
            verb(Some(&format!("git push https://{token}@x/y")), "Bash"),
            Some("git push".into())
        );
        assert_eq!(
            verb(Some("/usr/bin/psql -c x"), "Bash"),
            Some("psql".into())
        );
        assert_eq!(verb(Some("https://x/y"), "Bash"), None);
        assert_eq!(verb(Some("abc1234567 x"), "Bash"), None);
        assert_eq!(verb(Some("git push"), "Read"), None);
        assert_eq!(verb(Some("gh pr create"), "Bash"), Some("gh pr".into()));
    }

    #[test]
    fn append_cuts_to_the_newest_rows() {
        let dir = tempfile::tempdir().unwrap();
        let max = table().max_rows;
        for i in 0..=(2 * max) {
            append(dir.path(), "s-1", &json!({ "at": i })).unwrap();
        }
        let text = std::fs::read_to_string(dir.path().join("s-1.jsonl")).unwrap();
        assert_eq!(text.lines().count(), max);
        assert!(text.ends_with(&format!("{{\"at\":{}}}\n", 2 * max)));
        append(dir.path(), "../x", &json!({})).unwrap();
        assert!(!dir.path().join("../x.jsonl").exists());
    }

    #[test]
    fn options_and_dirs() {
        let env = |k: &str| (k == "CLAUDE_PLUGIN_OPTION_GUARDUX").then(|| "false".to_string());
        assert!(option_off(env, "guardUx"));
        assert!(!option_off(env, "guardLog"));
        let get = |k: &str| (k == "HOME").then(|| PathBuf::from("/h"));
        assert_eq!(
            log_dir(get),
            Some(PathBuf::from("/h/.local/state/ai-gent/guard-log"))
        );
    }
}
