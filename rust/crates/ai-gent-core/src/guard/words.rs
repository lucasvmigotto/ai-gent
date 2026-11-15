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
    /// The most bytes read from a log (its tail) when trimming it.
    #[serde(rename = "maxBytes")]
    pub max_bytes: u64,
    /// The subcommands kept as a second word, by first word.
    pub verbs: BTreeMap<String, Vec<String>>,
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
/// lowercase word with fewer than seven digits, and for a first word the table's `verbs` lists, the
/// second word when it is one of its listed subcommands. Never a value, an entry, a branch or a path.
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
    if table()
        .verbs
        .get(first)
        .is_some_and(|subs| subs.iter().any(|v| v == second))
    {
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

/// Append a row to `<dir>/<session>.jsonl`. The directory is created 0700 and the file 0600 on
/// Unix (an existing one we own is tightened to that); the file is opened without following a
/// symlink and without blocking, and anything but a regular file is refused. Past twice the row
/// limit (or `maxBytes`) the file is cut to its newest rows, read from its last `maxBytes` only,
/// under a lock on `<session>.jsonl.lock`; a lock held by another writer skips the cut this time,
/// and a file that grew while the cut was written is left whole (the cut waits for the next row).
///
/// # Errors
/// When the directory or file cannot be created or written, or the path is not a regular file.
pub fn append(dir: &Path, session: &str, row: &Value) -> std::io::Result<()> {
    if !is_session_id(session) {
        return Ok(());
    }
    make_dir(dir)?;
    let path = dir.join(format!("{session}.jsonl"));
    let mut file = open_log(&path, true)?;
    let mut line = row.to_string();
    line.push('\n');
    file.write_all(line.as_bytes())?;
    let len = file.metadata()?.len();
    drop(file);
    let t = table();
    // Past maxBytes it is cut without counting; under it, its rows are counted (at most maxBytes read).
    if len > t.max_bytes || count_lines(&path, t.max_bytes)? > 2 * t.max_rows {
        trim(dir, session, &path, t.max_rows, t.max_bytes)?;
    }
    Ok(())
}

fn make_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        let meta = std::fs::metadata(dir)?;
        if meta.permissions().mode() & 0o077 != 0 && owned(&meta) {
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)?;
    Ok(())
}

#[cfg(unix)]
fn owned(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    meta.uid() == rustix::process::getuid().as_raw()
}

/// Open `<session>.jsonl.lock` to lock it: read and write access, which Windows needs to lock a
/// file (an append-only handle is refused there), never through a symlink or into a FIFO.
fn open_lock(path: &Path) -> std::io::Result<std::fs::File> {
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(not(unix))]
    if std::fs::symlink_metadata(path).is_ok_and(|m| !m.file_type().is_file()) {
        return Err(not_regular(path));
    }
    let file = opts.open(path)?;
    if !file.metadata()?.file_type().is_file() {
        return Err(not_regular(path));
    }
    Ok(file)
}

/// Open the log (to append, or to read) without following a symlink or blocking on a FIFO; refuse
/// anything that is not a regular file, and tighten a file we own to 0600.
fn open_log(path: &Path, write: bool) -> std::io::Result<std::fs::File> {
    let mut opts = std::fs::OpenOptions::new();
    if write {
        opts.create(true).append(true);
    } else {
        opts.read(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(not(unix))]
    if std::fs::symlink_metadata(path).is_ok_and(|m| !m.file_type().is_file()) {
        return Err(not_regular(path));
    }
    let file = opts.open(path)?;
    let meta = file.metadata()?;
    if !meta.file_type().is_file() {
        return Err(not_regular(path));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if write && meta.permissions().mode() & 0o077 != 0 && owned(&meta) {
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
    }
    Ok(file)
}

fn not_regular(path: &Path) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{} is not a regular file", path.display()),
    )
}

/// The last `cap` bytes of the log, and its length when read.
fn read_tail(path: &Path, cap: u64) -> std::io::Result<(Vec<u8>, u64)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = open_log(path, false)?;
    let len = file.metadata()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(cap)))?;
    let mut buf = Vec::new();
    file.take(cap).read_to_end(&mut buf)?;
    Ok((buf, len))
}

#[allow(clippy::naive_bytecount)] // at most maxBytes, once per row; no crate for it
fn count_lines(path: &Path, cap: u64) -> std::io::Result<usize> {
    Ok(read_tail(path, cap)?
        .0
        .iter()
        .filter(|b| **b == b'\n')
        .count())
}

fn trim(dir: &Path, session: &str, path: &Path, max: usize, cap: u64) -> std::io::Result<()> {
    let lock_path = dir.join(format!("{session}.jsonl.lock"));
    let lock = open_lock(&lock_path)?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => return Ok(()),
        Err(std::fs::TryLockError::Error(e)) => return Err(e),
    }
    let (tail, len) = read_tail(path, cap)?;
    let text = String::from_utf8_lossy(&tail);
    let mut lines: Vec<&str> = text.lines().collect();
    if tail.len() as u64 >= cap && !lines.is_empty() {
        lines.remove(0); // the first line read from the middle of the file may be cut
    }
    let keep = &lines[lines.len().saturating_sub(max)..];
    if keep.is_empty() {
        return Ok(());
    }
    let (tmp, mut file) = temp_file(dir, session)?;
    let body = keep.join("\n") + "\n";
    let written = file.write_all(body.as_bytes());
    drop(file); // closed before the rename, which Windows needs
    // A row appended since the snapshot (the file grew) would be lost by the rename: leave the
    // file whole and cut at the next row. Only a row appended between this check and the rename
    // can still be lost.
    let result = written
        .and_then(|()| std::fs::symlink_metadata(path))
        .and_then(|m| {
            if m.len() == len {
                std::fs::rename(&tmp, path).map(|()| true)
            } else {
                Ok(false)
            }
        });
    if !matches!(result, Ok(true)) {
        let _ = std::fs::remove_file(&tmp);
    }
    drop(lock);
    result.map(|_| ())
}

/// A new file `<dir>/.<session>.jsonl.<random>` (`create_new`, 0600, never through a symlink).
fn temp_file(dir: &Path, session: &str) -> std::io::Result<(PathBuf, std::fs::File)> {
    use std::hash::{BuildHasher, Hasher};
    for _ in 0..8 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u32(std::process::id());
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        );
        let tmp = dir.join(format!(".{session}.jsonl.{:016x}", h.finish()));
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        match opts.open(&tmp) {
            Ok(f) => return Ok((tmp, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "no free temporary name",
    ))
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
        // a second word that is not a listed subcommand (an entry, a branch) is never kept
        assert_eq!(
            verb(Some("pass example-entry"), "Bash"),
            Some("pass".into())
        );
        assert_eq!(verb(Some("git feature-x"), "Bash"), Some("git".into()));
        assert_eq!(
            verb(Some("pass show x/y"), "Bash"),
            Some("pass show".into())
        );
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
        // no temporary file is left behind
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn append_refuses_links_and_fifos_and_tightens_modes() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::write(&target, "x\n").unwrap();
        std::os::unix::fs::symlink(&target, dir.path().join("s-link.jsonl")).unwrap();
        assert!(append(dir.path(), "s-link", &json!({})).is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "x\n");
        let fifo = dir.path().join("s-fifo.jsonl");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        let start = std::time::Instant::now();
        assert!(append(dir.path(), "s-fifo", &json!({})).is_err());
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
        let loose = dir.path().join("s-loose.jsonl");
        std::fs::write(&loose, "").unwrap();
        std::fs::set_permissions(&loose, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        append(dir.path(), "s-loose", &json!({})).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&loose), 0o600);
        assert_eq!(mode(dir.path()), 0o700);
    }

    #[test]
    fn a_large_log_is_read_from_its_tail_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s-big.jsonl");
        let row = format!("{}\n", json!({ "at": 0, "pad": "x".repeat(100) }));
        let big = row.repeat(usize::try_from(2 * table().max_bytes).unwrap() / row.len());
        std::fs::write(&path, big).unwrap();
        append(dir.path(), "s-big", &json!({ "at": 1 })).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), table().max_rows);
        assert!(text.ends_with("{\"at\":1}\n"));
    }

    #[test]
    fn a_held_lock_skips_the_cut_and_loses_no_row() {
        let dir = tempfile::tempdir().unwrap();
        let max = table().max_rows;
        let lock = std::fs::File::create(dir.path().join("s-l.jsonl.lock")).unwrap();
        lock.lock().unwrap();
        for i in 0..=(2 * max) {
            append(dir.path(), "s-l", &json!({ "at": i })).unwrap();
        }
        let text = std::fs::read_to_string(dir.path().join("s-l.jsonl")).unwrap();
        assert_eq!(text.lines().count(), 2 * max + 1);
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
