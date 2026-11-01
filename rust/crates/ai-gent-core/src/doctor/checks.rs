//! The thirteen checks of `contracts/cli.md`. All read-only and offline; none reads the content of
//! a credential or state file (the `permissions` check looks at modes only).
use super::{Check, Ctx, Status, selftest};
use crate::update::{
    apply::{is_executable, on_path},
    cache, version,
};
use std::path::{Path, PathBuf};

use Status::{Fail, Ok as Good, Skip, Warn};

/// Skills directories `setup.sh` links into: (id, override variable, default under `$HOME`).
/// `scripts/consistency.py` checks this table against `AGENT_IDS` and the `*_SKILLS` lines there.
pub const AGENT_DIRS: &[(&str, &str, &str)] = &[
    ("claude", "CLAUDE_SKILLS_DIR", ".claude/skills"),
    ("opencode", "OPENCODE_SKILLS_DIR", ".config/opencode/skills"),
    ("agents", "AGENTS_SKILLS_DIR", ".agents/skills"),
    ("kiro", "KIRO_SKILLS_DIR", ".kiro/skills"),
    (
        "windsurf",
        "WINDSURF_SKILLS_DIR",
        ".codeium/windsurf/skills",
    ),
    (
        "antigravity",
        "ANTIGRAVITY_SKILLS_DIR",
        ".gemini/config/skills",
    ),
];

const SETUP_CMD: &str = "sh install.sh";

/// The version directory to inspect: the binary's own, else `<home>/current`.
fn tree(ctx: &Ctx) -> Option<PathBuf> {
    if let Some(v) = &ctx.root.version_dir {
        return Some(v.clone());
    }
    let cur = ctx.root.home.as_ref()?.join("current");
    cur.is_dir().then_some(cur)
}

/// `binary`: the running executable.
pub fn binary(ctx: &Ctx) -> Check {
    let shown = ctx.exe.display();
    if !ctx.exe.is_file() {
        return Check::new("binary", Fail, format!("{shown} is not a file")).hint(SETUP_CMD);
    }
    if !is_executable(&ctx.exe) {
        return Check::new("binary", Fail, format!("{shown} is not executable"))
            .hint(format!("chmod +x {shown}"));
    }
    let what = match (&ctx.root.version, ctx.root.dev_build) {
        (Some(v), _) => format!("{v} at {shown}"),
        (None, true) => format!("development build at {shown}"),
        (None, false) => shown.to_string(),
    };
    Check::new(
        "binary",
        Good,
        format!("ai-gent {what} (release {})", ctx.release),
    )
}

/// `install_root`: `current` resolves; `previous` optional.
pub fn install_root(ctx: &Ctx) -> Check {
    let Some(home) = ctx.root.home.as_deref() else {
        return Check::new(
            "install_root",
            Fail,
            "no install root (AI_GENT_HOME, HOME or LOCALAPPDATA unset)",
        )
        .hint(SETUP_CMD);
    };
    let cur = home.join("current");
    if cur.symlink_metadata().is_err() {
        return Check::new(
            "install_root",
            Fail,
            format!("{} has no `current`", home.display()),
        )
        .hint(SETUP_CMD);
    }
    if !cur.is_dir() {
        let to = ctx.root.current().unwrap_or_else(|| "?".into());
        return Check::new(
            "install_root",
            Fail,
            format!("`current` points at {to}, which does not exist"),
        )
        .hint(SETUP_CMD);
    }
    let prev = home.join("previous");
    let current = ctx.root.current().unwrap_or_else(|| "current".into());
    if prev.symlink_metadata().is_ok() && !prev.is_dir() {
        return Check::new(
            "install_root",
            Warn,
            format!(
                "{} (current {current}); `previous` is dangling",
                home.display()
            ),
        )
        .hint("sh install.sh --prune");
    }
    let tail = ctx
        .root
        .previous()
        .map_or(String::new(), |p| format!(", previous {p}"));
    Check::new(
        "install_root",
        Good,
        format!("{} (current {current}{tail})", home.display()),
    )
}

/// `version_match`: binary release equals the `VERSION` of its directory.
pub fn version_match(ctx: &Ctx) -> Check {
    let Some(installed) = &ctx.root.version else {
        return Check::new("version_match", Skip, "not under an install root");
    };
    if ctx.release == "dev" {
        return Check::new("version_match", Skip, "development build");
    }
    if &ctx.release == installed {
        Check::new("version_match", Good, installed.clone())
    } else {
        Check::new(
            "version_match",
            Fail,
            format!("binary {}, install {installed}", ctx.release),
        )
        .hint(SETUP_CMD)
    }
}

/// `install_integrity`: every `MANIFEST` line hashes equal.
pub fn install_integrity(ctx: &Ctx) -> Check {
    if ctx.fast {
        return Check::new("install_integrity", Skip, "--fast");
    }
    let Some(dir) = tree(ctx) else {
        return Check::new("install_integrity", Skip, "no installed version to check");
    };
    let Ok(manifest) = std::fs::read_to_string(dir.join("MANIFEST")) else {
        return Check::new(
            "install_integrity",
            Warn,
            format!("{} has no MANIFEST", dir.display()),
        )
        .hint("an install before 2.26 has none; reinstall: sh install.sh");
    };
    let mut bad: Vec<String> = Vec::new();
    let mut total = 0usize;
    for line in manifest.lines().filter(|l| !l.trim().is_empty()) {
        let Some((want, rel)) = line.split_once(' ') else {
            continue;
        };
        let rel = rel.trim_start().trim_start_matches('*');
        total += 1;
        if rel.starts_with('/') || rel.split(['/', '\\']).any(|p| p == "..") {
            bad.push(format!("{rel} (unsafe path in MANIFEST)"));
            continue;
        }
        match std::fs::read(dir.join(rel)) {
            Err(_) => bad.push(format!("{rel} (missing)")),
            Ok(bytes) if crate::guard::shadow::sha256_hex(&bytes) != want => {
                bad.push(format!("{rel} (differs)"));
            }
            Ok(_) => {}
        }
    }
    if bad.is_empty() {
        return Check::new(
            "install_integrity",
            Good,
            format!("{total} files match MANIFEST"),
        );
    }
    let shown = bad.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
    let more = if bad.len() > 5 {
        format!(" and {} more", bad.len() - 5)
    } else {
        String::new()
    };
    Check::new(
        "install_integrity",
        Fail,
        format!("{} of {total} files changed: {shown}{more}", bad.len()),
    )
    .hint("reinstall this version: rm -r the version directory, then sh install.sh")
}

/// `path_link`: `~/.local/bin/ai-gent` points at `current/bin/ai-gent`; the directory is on `PATH`.
pub fn path_link(ctx: &Ctx) -> Check {
    if cfg!(windows) {
        return Check::new(
            "path_link",
            Skip,
            "the Windows installer manages the user PATH",
        );
    }
    if ctx.root.dev_build {
        return Check::new("path_link", Skip, "development build");
    }
    let Some(bin_dir) = ctx
        .env
        .path("AI_GENT_BIN_DIR")
        .or_else(|| ctx.env.home_dir().map(|h| h.join(".local/bin")))
    else {
        return Check::new("path_link", Skip, "no home directory");
    };
    let link = bin_dir.join("ai-gent");
    let expected = ctx
        .root
        .home
        .as_ref()
        .map(|h| h.join("current/bin/ai-gent"));
    let Ok(resolved) = link.canonicalize() else {
        return Check::new(
            "path_link",
            Warn,
            format!("{} is missing or dangling", link.display()),
        )
        .hint("current/setup.sh (relinks it)");
    };
    let own = ctx.exe.canonicalize().ok();
    let want = expected.and_then(|e| e.canonicalize().ok());
    if want.as_ref() != Some(&resolved) && own.as_ref() != Some(&resolved) {
        return Check::new(
            "path_link",
            Warn,
            format!(
                "{} points at {}, not this install",
                link.display(),
                resolved.display()
            ),
        )
        .hint("current/setup.sh (relinks it)");
    }
    let on_path = ctx
        .env
        .path("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d == bin_dir));
    if on_path {
        Check::new(
            "path_link",
            Good,
            format!("{} -> current/bin/ai-gent", link.display()),
        )
    } else {
        Check::new(
            "path_link",
            Warn,
            format!("{} is not on PATH", bin_dir.display()),
        )
        .hint(format!("add {} to PATH", bin_dir.display()))
    }
}

/// How an entry in an agent skills directory relates to this install.
enum LinkKind {
    Ours,
    Foreign(PathBuf),
    DanglingOurs,
    DanglingForeign(PathBuf),
    NotAi,
}

fn classify(entry: &Path, home: Option<&Path>) -> LinkKind {
    let Ok(target) = std::fs::read_link(entry) else {
        return LinkKind::NotAi;
    };
    let text = target.to_string_lossy().replace('\\', "/");
    let ours = home.is_some_and(|h| target.starts_with(h));
    // Skills link to <tree>/plugins/<plugin>/skills/<skill>; generated stubs live elsewhere.
    let looks_ai = text.contains("/plugins/") && text.contains("/skills/");
    let resolves = entry.exists();
    match (ours, looks_ai, resolves) {
        (true, _, true) => LinkKind::Ours,
        (true, _, false) => LinkKind::DanglingOurs,
        (false, true, true) => LinkKind::Foreign(target),
        (false, true, false) => LinkKind::DanglingForeign(target),
        (false, false, _) => LinkKind::NotAi,
    }
}

/// `agent_links`: skills directories hold working links into this install.
pub fn agent_links(ctx: &Ctx) -> Check {
    let Some(home_dir) = ctx.env.home_dir() else {
        return Check::new("agent_links", Skip, "no home directory");
    };
    let home = ctx.root.home.as_deref();
    let (mut ours, mut dirs) = (0usize, 0usize);
    let (mut foreign, mut dangling): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for (id, var, default) in AGENT_DIRS {
        let dir = ctx.env.path(var).unwrap_or_else(|| home_dir.join(default));
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        dirs += 1;
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            match classify(&e.path(), home) {
                LinkKind::Ours => ours += 1,
                LinkKind::Foreign(t) => foreign.push(format!("{id}/{name} -> {}", t.display())),
                LinkKind::DanglingOurs => dangling.push(format!("{id}/{name}")),
                LinkKind::DanglingForeign(t) => {
                    dangling.push(format!("{id}/{name} -> {}", t.display()));
                }
                LinkKind::NotAi => {}
            }
        }
    }
    let list = |v: &[String]| v.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    if !dangling.is_empty() {
        return Check::new(
            "agent_links",
            Fail,
            format!("{} dangling link(s): {}", dangling.len(), list(&dangling)),
        )
        .hint("current/setup.sh (relinks), or setup.sh --uninstall for what you removed");
    }
    if !foreign.is_empty() {
        return Check::new(
            "agent_links",
            Warn,
            format!(
                "{} link(s) into another ai-gent tree: {}",
                foreign.len(),
                list(&foreign)
            ),
        )
        .hint("setup.sh --migrate");
    }
    if dirs == 0 {
        return Check::new("agent_links", Skip, "no agent skills directory found");
    }
    Check::new(
        "agent_links",
        Good,
        format!(
            "{ours} links in {dirs} skills director{}",
            if dirs == 1 { "y" } else { "ies" }
        ),
    )
}

/// Collect the strings of every `command` key.
fn commands(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, x) in m {
                match (k.as_str(), x) {
                    ("command", serde_json::Value::String(s)) => out.push(s.clone()),
                    _ => commands(x, out),
                }
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| commands(x, out)),
        _ => {}
    }
}

/// Files a hook command runs, relative to the plugin root.
fn plugin_files(cmd: &str) -> Vec<String> {
    const VAR: &str = "${CLAUDE_PLUGIN_ROOT}/";
    let mut out = Vec::new();
    let mut rest = cmd;
    while let Some(i) = rest.find(VAR) {
        rest = &rest[i + VAR.len()..];
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || "_./-".contains(c)))
            .unwrap_or(rest.len());
        if end > 0 {
            out.push(rest[..end].to_string());
        }
    }
    out
}

/// `hooks`: each installed plugin's hook commands target files that exist.
pub fn hooks(ctx: &Ctx) -> Check {
    let Some(dir) = tree(ctx) else {
        return Check::new("hooks", Skip, "no installed version to check");
    };
    let Ok(plugins) = std::fs::read_dir(dir.join("plugins")) else {
        return Check::new("hooks", Skip, "no plugins directory");
    };
    let (mut missing, mut seen) = (Vec::new(), 0usize);
    for p in plugins.flatten() {
        let Ok(text) = std::fs::read_to_string(p.path().join("hooks/hooks.json")) else {
            continue;
        };
        let name = p.file_name().to_string_lossy().to_string();
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
            missing.push(format!("{name}/hooks/hooks.json (not valid JSON)"));
            continue;
        };
        let mut cmds = Vec::new();
        commands(&json, &mut cmds);
        for cmd in cmds {
            // A command that tests for a file first (`if [ -x ... ]`) treats it as optional.
            let guarded = cmd.contains("[ -");
            for f in plugin_files(&cmd) {
                seen += 1;
                if guarded && f.contains("..") {
                    continue;
                }
                if !p.path().join(&f).exists() {
                    missing.push(format!("{name}: {f}"));
                }
            }
        }
    }
    if missing.is_empty() {
        return Check::new("hooks", Good, format!("{seen} hook target(s) exist"));
    }
    let shown = missing
        .iter()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    Check::new("hooks", Fail, format!("missing: {shown}"))
        .hint("reinstall this version: sh install.sh")
}

/// `guard_selftest`: the evaluator still makes its built-in decisions.
pub fn guard_selftest(_ctx: &Ctx) -> Check {
    let bad = selftest::run(selftest::CASES);
    if bad.is_empty() {
        Check::new(
            "guard_selftest",
            Good,
            format!("{} built-in decisions hold", selftest::CASES.len()),
        )
    } else {
        Check::new("guard_selftest", Fail, bad.join("; ")).hint("reinstall: sh install.sh")
    }
}

/// `tools`: external programs.
pub fn tools(ctx: &Ctx) -> Check {
    let runner = if cfg!(windows) { "powershell" } else { "sh" };
    let missing: Vec<&str> = ["git", "curl", runner]
        .into_iter()
        .filter(|t| !on_path(&ctx.env, t))
        .collect();
    let optional: Vec<String> = ["gh", "pass", "podman", "docker"]
        .iter()
        .map(|t| format!("{t} {}", if on_path(&ctx.env, t) { "yes" } else { "no" }))
        .collect();
    let detail = format!("optional: {}", optional.join(", "));
    if missing.is_empty() {
        Check::new(
            "tools",
            Good,
            format!("git, curl, {runner} found; {detail}"),
        )
    } else {
        Check::new(
            "tools",
            Warn,
            format!("missing {}; {detail}", missing.join(", ")),
        )
        .hint("git, curl and a shell are needed for update and the git workflow")
    }
}

const SHADOW_WARN_BYTES: u64 = 20 * 1024 * 1024;

/// `state`: state and cache directories, shadow log size.
pub fn state(ctx: &Ctx) -> Check {
    let Some(dir) = ctx.env.state_dir() else {
        return Check::new(
            "state",
            Skip,
            "no state directory (HOME or LOCALAPPDATA unset)",
        );
    };
    if !dir.exists() {
        let parent_ok = dir
            .ancestors()
            .skip(1)
            .find(|a| a.exists())
            .is_some_and(|a| {
                a.is_dir() && std::fs::metadata(a).is_ok_and(|m| !m.permissions().readonly())
            });
        return if parent_ok {
            Check::new(
                "state",
                Good,
                format!(
                    "{} does not exist yet (created on first use)",
                    dir.display()
                ),
            )
        } else {
            Check::new(
                "state",
                Warn,
                format!("{} cannot be created", dir.display()),
            )
        };
    }
    let size: u64 = ["guard-shadow.jsonl", "guard-shadow.1.jsonl"]
        .iter()
        .filter_map(|n| std::fs::metadata(dir.join(n)).ok())
        .map(|m| m.len())
        .sum();
    if size > SHADOW_WARN_BYTES {
        return Check::new(
            "state",
            Warn,
            format!("shadow log is {} MiB", size / (1024 * 1024)),
        )
        .hint(format!("rm {}/guard-shadow*.jsonl", dir.display()));
    }
    Check::new(
        "state",
        Good,
        format!("{} (shadow log {} KiB)", dir.display(), size / 1024),
    )
}

/// `update_cache`: cached state only, never the network.
pub fn update_cache(ctx: &Ctx) -> Check {
    if ctx.env.is_set("AI_GENT_NO_UPDATE_CHECK") {
        return Check::new("update_cache", Skip, "AI_GENT_NO_UPDATE_CHECK is set");
    }
    if let Some(p) = ctx.root.pinned() {
        let v = if p.is_empty() {
            "a version".to_string()
        } else {
            p
        };
        return Check::new(
            "update_cache",
            Good,
            format!("pinned to {v}; no update notices"),
        );
    }
    let Some(dir) = ctx.env.state_dir() else {
        return Check::new("update_cache", Skip, "no state directory");
    };
    let file = cache::path_in(&dir);
    if !file.exists() {
        return Check::new(
            "update_cache",
            Good,
            "no cache yet (the next session start refreshes it)",
        );
    }
    let Some(c) = cache::read(&file) else {
        return Check::new(
            "update_cache",
            Warn,
            format!("{} is not readable as `<epoch> <X.Y.Z>`", file.display()),
        )
        .hint(format!("rm {}", file.display()));
    };
    let age = c.checked.map_or("an unknown time ago".to_string(), |t| {
        let secs = crate::update::check::now_secs().saturating_sub(t);
        format!("{} day(s) ago", secs / 86_400)
    });
    let newer = ctx
        .root
        .version
        .as_deref()
        .is_some_and(|v| version::ver_gt(&c.latest, v));
    let tail = if newer {
        "; an update is available: ai-gent update"
    } else {
        ""
    };
    Check::new(
        "update_cache",
        Good,
        format!("latest {} cached {age}{tail}", c.latest),
    )
}

/// `environment`: variables removed in 3.0.0.
pub fn environment(ctx: &Ctx) -> Check {
    let stale: Vec<String> = ["AI_GENT_MODE", "AI_GENT_BRANCH", "AI_GENT_DIR"]
        .iter()
        .filter(|k| ctx.env.is_set(k))
        .map(|k| (*k).to_string())
        .collect();
    if stale.is_empty() {
        Check::new("environment", Good, "no stale variables")
    } else {
        Check::new(
            "environment",
            Warn,
            format!("{} set; git mode goes away in 3.0.0", stale.join(", ")),
        )
        .hint("unset them and install the package: sh install.sh")
    }
}

#[cfg(unix)]
fn loose(path: &Path, dir: bool) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path).ok()?.permissions().mode() & 0o777;
    let want = if dir { 0o700 } else { 0o600 };
    // Only extra permission bits are a problem; 0400 is stricter than 0600.
    (mode & !want != 0).then(|| format!("{} is {mode:o}, want {want:o}", path.display()))
}

/// `permissions`: modes of ai-gent's state and config files (never their content).
pub fn permissions(ctx: &Ctx) -> Check {
    #[cfg(unix)]
    {
        // What ai-gent itself keeps private: the config directory (credential files of
        // shared/secrets.md) and, in the state directory, the shadow log the binary writes 0600.
        // Files other tools create there (`targets`, the cache) are not credentials and are not judged.
        let mut problems = Vec::new();
        let mut looked = 0usize;
        let config = ctx
            .env
            .path("XDG_CONFIG_HOME")
            .or_else(|| ctx.env.home_dir().map(|h| h.join(".config")))
            .map(|c| c.join("ai-gent"));
        if let Some(d) = config.filter(|d| d.is_dir()) {
            looked += 1;
            problems.extend(loose(&d, true));
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                if e.file_type().is_ok_and(|t| t.is_file()) {
                    problems.extend(loose(&e.path(), false));
                }
            }
        }
        if let Some(d) = ctx.env.state_dir() {
            let logs: Vec<PathBuf> = ["guard-shadow.jsonl", "guard-shadow.1.jsonl"]
                .iter()
                .map(|n| d.join(n))
                .filter(|p| p.is_file())
                .collect();
            if !logs.is_empty() {
                looked += 1;
                problems.extend(loose(&d, true));
                for l in logs {
                    problems.extend(loose(&l, false));
                }
            }
        }
        if looked == 0 {
            return Check::new(
                "permissions",
                Skip,
                "no ai-gent state or config directory yet",
            );
        }
        if problems.is_empty() {
            return Check::new(
                "permissions",
                Good,
                format!(
                    "{looked} director{} checked (modes only)",
                    if looked == 1 { "y" } else { "ies" }
                ),
            );
        }
        Check::new(
            "permissions",
            Warn,
            problems
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join("; "),
        )
        .hint("chmod 700 the directory, chmod 600 the files")
    }
    #[cfg(not(unix))]
    {
        let _ = ctx;
        Check::new("permissions", Skip, "Unix modes do not apply on Windows")
    }
}
