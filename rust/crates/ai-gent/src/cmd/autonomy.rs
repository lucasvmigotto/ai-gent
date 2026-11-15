//! `ai-gent autonomy`: write, show and remove the autonomy grant (`.ai-gent/autonomy`).
//!
//! The grant is the user's permission for a hands-off run; the guards deny an agent running
//! `grant` or `revoke`, so these are for the user's own terminal. The logic is in
//! `ai_gent_core::guard::grant_write`; this file is the arguments, the prompts and the output.
use ai_gent_core::exit::Exit;
use ai_gent_core::guard::grant::{self, FILE};
use ai_gent_core::guard::grant_write::{
    self as gw, ACTIONS, DEFAULT_BRANCHES, PROTECTED, Status, fmt_local, human_left,
    local_offset_at, now_secs,
};
use ai_gent_core::output::{self, ErrorCode};
use clap::{Args, Subcommand};
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

/// Arguments of `autonomy`.
#[derive(Debug, Args)]
pub struct AutonomyArgs {
    /// What to run.
    #[command(subcommand)]
    action: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Write the grant after the user approves it (run it in your own terminal, not through an agent).
    Grant(GrantArgs),
    /// Show the active grant, its expiry and the time left (read-only).
    Show,
    /// Remove the grant file.
    Revoke,
}

const GRANT_HELP: &str = "\
Expiry and actions are mandatory: there is no default grant. With none of --for, --until,
--actions and --branches, on a terminal, it asks for each. The file is written to
.ai-gent/autonomy at the repository root of the current directory (mode 0600) and git-excluded.
It lasts at most 30 days and cannot cover main, master, homolog or staging.

Actions (the file stores the first word; aliases are accepted):
  action  alias      on each platform                                           lifted today
  push    -          git push, any remote                                       any remote
  pr      mr         pull request: GitHub, Azure DevOps, Bitbucket;             GitHub only
                     merge request: GitLab
  merge   -          merging that pull/merge request (same platforms)           GitHub only (reporun)
  issues  workitems  issues: GitHub, GitLab; Boards work items: Azure DevOps;   GitHub only (reporun)
                     Jira and Bitbucket issues

A grant for an action on a platform without guard rules is accepted but lifts nothing there.
See plugins/git/skills/workflow/references/autonomy.md.";

/// Arguments of `autonomy grant`.
#[derive(Debug, Args)]
#[command(after_long_help = GRANT_HELP)]
pub struct GrantArgs {
    /// How long the grant lasts: 30m, 4h, 1d (at most 30d).
    #[arg(long = "for", value_name = "DURATION", conflicts_with = "until")]
    for_: Option<String>,
    /// When it ends, local time: `YYYY-MM-DD HH:MM` or RFC 3339.
    #[arg(long, value_name = "DATETIME")]
    until: Option<String>,
    /// What to lift: push, pr (mr), merge, issues (workitems), comma separated.
    #[arg(long, value_name = "LIST")]
    actions: Option<String>,
    /// The integration branches it covers (default develop,dev).
    #[arg(long, value_name = "LIST")]
    branches: Option<String>,
    /// Write without the final question.
    #[arg(long)]
    yes: bool,
}

/// A failed run: the message and the exit code.
struct Fail(Exit, String);

fn usage<T>(m: impl Into<String>) -> Result<T, Fail> {
    Err(Fail(Exit::Usage, m.into()))
}

/// Run the subcommand.
pub fn run(a: &AutonomyArgs, json: bool) -> Exit {
    let res = match &a.action {
        Action::Grant(g) => grant_cmd(g, json),
        Action::Show => show_cmd(json),
        Action::Revoke => revoke_cmd(json),
    };
    match res {
        Ok(e) => e,
        Err(Fail(code, msg)) => {
            if json {
                let c = if code == Exit::Usage {
                    ErrorCode::Usage
                } else {
                    ErrorCode::Runtime
                };
                println!("{}", output::error_json(c, &msg, None));
            }
            eprintln!("error: {msg}");
            code
        }
    }
}

fn root_here() -> Result<PathBuf, Fail> {
    let cwd = std::env::current_dir().map_err(|e| Fail(Exit::Runtime, e.to_string()))?;
    gw::repo_root(&cwd).map_err(|e| Fail(Exit::Runtime, e))
}

/// The choice a run settled on.
struct Choice {
    expires: i64,
    actions: Vec<&'static str>,
    branches: Vec<String>,
}

/// Refuse unless a person is at a terminal (exit 2, nothing touched). `--yes` never skips this.
fn need_terminal() -> Result<(), Fail> {
    gw::require_terminal().or_else(usage)
}

fn grant_cmd(a: &GrantArgs, json: bool) -> Result<Exit, Fail> {
    need_terminal()?;
    let root = root_here()?;
    let now = now_secs();
    let no_flags =
        a.for_.is_none() && a.until.is_none() && a.actions.is_none() && a.branches.is_none();
    let tty = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let mut asked = false;
    let choice = if no_flags && tty {
        asked = true;
        let stdin = std::io::stdin();
        let mut p = Prompter {
            inp: &mut stdin.lock(),
            out: &mut std::io::stdout(),
        };
        let have = local_branches(&root);
        match interactive(&mut p, now, &have) {
            Ok(c) => c,
            Err(m) => return usage(m),
        }
    } else {
        from_flags(a, now)?
    };
    gw::check_expiry(now, choice.expires).or_else(usage)?;
    let bad = gw::protected_in(&choice.branches);
    if !bad.is_empty() {
        return usage(format!(
            "refusing to cover {}: a grant covers integration branches only, and promotion to {} stays the user's own step",
            bad.join(", "),
            PROTECTED.join(", ")
        ));
    }
    let off = local_offset_at(choice.expires);
    if !a.yes {
        if !tty {
            return usage("nothing written: confirm with --yes (no terminal to ask on)");
        }
        let stdin = std::io::stdin();
        let mut p = Prompter {
            inp: &mut stdin.lock(),
            out: &mut std::io::stdout(),
        };
        summary(&mut p, &choice, now, off);
        if !confirm(&mut p, &format!("Write {FILE}? [y/N] ")) {
            println!("nothing written");
            return Ok(Exit::NothingToDo);
        }
    } else if asked {
        println!("{}", summary_text(&choice, now, off));
    }
    if let Status::Active {
        expires,
        actions,
        branches,
    } = gw::inspect(&root, now)
    {
        eprintln!(
            "notice: replacing the active grant (actions: {}; branches: {}; expires {} UTC)",
            actions.join(" "),
            branches.join(" "),
            grant::format_utc(expires)
        );
    }
    let text = gw::render(choice.expires, &choice.actions, &choice.branches);
    // Ignore first, then write; a grant that ends up live and not ignored is removed.
    let excluded = gw::ensure_excluded(&root).map_err(|e| Fail(Exit::Runtime, e))?;
    let path = gw::write_atomic(&root, &text).map_err(|e| Fail(Exit::Runtime, e))?;
    if !gw::is_ignored(&root) {
        let _ = std::fs::remove_file(&path);
        return Err(Fail(
            Exit::Runtime,
            format!("git does not ignore {FILE}; the grant was removed"),
        ));
    }
    report(&path, &choice, now, off, excluded.as_deref(), json);
    Ok(Exit::Success)
}

fn from_flags(a: &GrantArgs, now: i64) -> Result<Choice, Fail> {
    let expires = match (&a.for_, &a.until) {
        (Some(d), _) => now + gw::parse_duration(d).or_else(usage)?,
        (None, Some(u)) => gw::parse_until(u, &local_offset_at).or_else(usage)?,
        (None, None) => {
            return usage(
                "no grant without an expiry: pass --for <30m|4h|1d> or --until <datetime> (and --actions), or run it on a terminal to be asked",
            );
        }
    };
    let Some(list) = &a.actions else {
        return usage(
            "no grant without actions: pass --actions push,pr,merge,issues (or a subset), or run it on a terminal to be asked",
        );
    };
    let actions = gw::parse_actions(list).or_else(usage)?;
    let branches = match &a.branches {
        Some(b) => gw::parse_branches(b).or_else(usage)?,
        None => DEFAULT_BRANCHES.iter().map(ToString::to_string).collect(),
    };
    Ok(Choice {
        expires,
        actions,
        branches,
    })
}

fn summary_text(c: &Choice, now: i64, off: i64) -> String {
    format!(
        "Grant to write:\n  actions:  {}\n  branches: {}\n  expires:  {} ({} UTC, in {})",
        c.actions.join(" "),
        c.branches.join(" "),
        fmt_local(c.expires, off),
        grant::format_utc(c.expires),
        human_left(c.expires - now)
    )
}

fn summary(p: &mut Prompter, c: &Choice, now: i64, off: i64) {
    let _ = writeln!(p.out, "{}", summary_text(c, now, off));
}

/// Where each non-lifting platform of the granted actions is named.
fn lifts_nothing(actions: &[&str]) -> Vec<String> {
    ACTIONS
        .iter()
        .filter(|a| actions.contains(&a.name))
        .filter_map(|a| {
            let none: Vec<&str> = a
                .platforms
                .iter()
                .filter(|(_, lifted)| !lifted)
                .map(|(p, _)| *p)
                .collect();
            (!none.is_empty()).then(|| {
                format!(
                    "{} lifts nothing on {} (no guard rules for them yet)",
                    a.name,
                    none.join(", ")
                )
            })
        })
        .collect()
}

fn report(path: &Path, c: &Choice, now: i64, off: i64, excluded: Option<&str>, json: bool) {
    let notes = lifts_nothing(&c.actions);
    if json {
        println!(
            "{}",
            serde_json::json!({
                "schema": 1,
                "ok": true,
                "path": path.display().to_string(),
                "expires_utc": grant::format_utc(c.expires),
                "expires_local": fmt_local(c.expires, off),
                "seconds_left": c.expires - now,
                "actions": c.actions,
                "branches": c.branches,
                "excluded": excluded,
                "lifts_nothing": notes,
            })
        );
        return;
    }
    println!("wrote {}", path.display());
    println!("  actions:  {}", c.actions.join(" "));
    println!("  branches: {}", c.branches.join(" "));
    println!(
        "  expires:  {} ({} UTC, in {})",
        fmt_local(c.expires, off),
        grant::format_utc(c.expires),
        human_left(c.expires - now)
    );
    if let Some(l) = excluded {
        println!("  added {l} to .git/info/exclude");
    }
    for n in notes {
        println!("  note: {n}");
    }
}

fn show_cmd(json: bool) -> Result<Exit, Fail> {
    let root = root_here()?;
    let now = now_secs();
    let status = gw::inspect(&root, now);
    let off = local_offset_at(now);
    let path = gw::path_in(&root).display().to_string();
    if json {
        let v = match &status {
            Status::Active {
                expires,
                actions,
                branches,
            } => serde_json::json!({
                "schema": 1, "ok": true, "active": true, "reason": null, "path": path,
                "expires_utc": grant::format_utc(*expires),
                "expires_local": fmt_local(*expires, local_offset_at(*expires)),
                "seconds_left": expires - now,
                "actions": actions, "branches": branches,
            }),
            other => {
                let (reason, detail) = match other {
                    Status::Missing => ("missing", String::new()),
                    Status::Expired(e) => ("expired", grant::format_utc(*e)),
                    Status::Unreadable(m) => ("unreadable", m.clone()),
                    Status::Active { .. } => unreachable!(),
                };
                serde_json::json!({
                    "schema": 1, "ok": true, "active": false, "reason": reason,
                    "detail": detail, "path": path,
                })
            }
        };
        println!("{v}");
        return Ok(Exit::Success);
    }
    match status {
        Status::Active {
            expires,
            actions,
            branches,
        } => {
            println!("active grant ({FILE})");
            println!("  actions:  {}", actions.join(" "));
            println!("  branches: {}", branches.join(" "));
            println!(
                "  expires:  {} UTC, {}",
                grant::format_utc(expires),
                fmt_local(expires, local_offset_at(expires))
            );
            println!("  left:     {}", human_left(expires - now));
            let known: Vec<&str> = actions.iter().map(String::as_str).collect();
            let canon: Vec<&str> = ACTIONS
                .iter()
                .map(|a| a.name)
                .filter(|n| known.contains(n))
                .collect();
            for n in lifts_nothing(&canon) {
                println!("  note: {n}");
            }
        }
        Status::Missing => println!("no active grant: missing ({FILE} does not exist)"),
        Status::Expired(e) => println!(
            "no active grant: expired at {} UTC ({})",
            grant::format_utc(e),
            fmt_local(e, off)
        ),
        Status::Unreadable(m) => println!("no active grant: unreadable ({m})"),
    }
    Ok(Exit::Success)
}

fn revoke_cmd(json: bool) -> Result<Exit, Fail> {
    need_terminal()?;
    let root = root_here()?;
    let path = gw::path_in(&root);
    let removed = match std::fs::remove_file(&path) {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => {
            return Err(Fail(
                Exit::Runtime,
                format!("cannot remove {}: {e}", path.display()),
            ));
        }
    };
    if json {
        println!(
            "{}",
            serde_json::json!({"schema": 1, "ok": true, "removed": removed, "path": path.display().to_string()})
        );
    } else if removed {
        println!("removed {}", path.display());
    } else {
        println!("no grant to remove ({FILE} does not exist)");
    }
    Ok(if removed {
        Exit::Success
    } else {
        Exit::NothingToDo
    })
}

// ---- prompts, by hand on std ---------------------------------------------------------------

/// Where the questions go and the answers come from.
struct Prompter<'a> {
    inp: &'a mut dyn BufRead,
    out: &'a mut dyn Write,
}

impl Prompter<'_> {
    /// Ask one question; the answer line, trimmed. End of input is an error.
    fn ask(&mut self, q: &str) -> Result<String, String> {
        let _ = write!(self.out, "{q}");
        let _ = self.out.flush();
        let mut line = String::new();
        match self.inp.read_line(&mut line) {
            Ok(0) | Err(_) => {
                Err("input closed before the grant was approved; nothing written".into())
            }
            Ok(_) => Ok(line.trim().to_string()),
        }
    }

    fn say(&mut self, s: &str) {
        let _ = writeln!(self.out, "{s}");
    }
}

fn confirm(p: &mut Prompter, q: &str) -> bool {
    p.ask(q)
        .is_ok_and(|a| matches!(a.to_ascii_lowercase().as_str(), "y" | "yes"))
}

/// Local and remote branch names, `origin/` stripped, without repeats.
fn local_branches(root: &Path) -> Vec<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads",
            "refs/remotes",
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    let mut names: Vec<String> = Vec::new();
    for r in String::from_utf8_lossy(&out.map(|o| o.stdout).unwrap_or_default()).lines() {
        let name = if let Some(n) = r.strip_prefix("refs/heads/") {
            n
        } else if let Some(n) = r.strip_prefix("refs/remotes/") {
            n.split_once('/').map_or("", |(_, b)| b)
        } else {
            ""
        };
        if !name.is_empty() && name != "HEAD" && !names.iter().any(|x| x == name) {
            names.push(name.to_string());
        }
    }
    names
}

/// The questions: actions, branches, expiry. The summary and the yes come after.
fn interactive(p: &mut Prompter, now: i64, have: &[String]) -> Result<Choice, String> {
    let actions = pick_actions(p)?;
    let branches = pick_branches(p, have)?;
    let expires = pick_expiry(p, now)?;
    Ok(Choice {
        expires,
        actions,
        branches,
    })
}

fn pick_actions(p: &mut Prompter) -> Result<Vec<&'static str>, String> {
    p.say("What may the agent do without asking? (numbers or names, e.g. 1 2; 'all')");
    for (i, a) in ACTIONS.iter().enumerate() {
        let alias = if a.aliases.is_empty() {
            String::new()
        } else {
            format!(" (or {})", a.aliases.join(", "))
        };
        p.say(&format!("  {}) {}{alias} - {}", i + 1, a.name, a.note));
    }
    loop {
        let ans = p.ask("actions: ")?;
        let mut words = Vec::new();
        for w in ans
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|w| !w.is_empty())
        {
            if w.eq_ignore_ascii_case("all") {
                words.extend(ACTIONS.iter().map(|a| a.name.to_string()));
            } else if let Some(a) = w
                .parse::<usize>()
                .ok()
                .and_then(|n| ACTIONS.get(n.wrapping_sub(1)))
            {
                words.push(a.name.to_string());
            } else {
                words.push(w.to_string());
            }
        }
        match gw::parse_actions(&words.join(" ")) {
            Ok(a) => return Ok(a),
            Err(m) => p.say(&format!("  {m}; pick at least one")),
        }
    }
}

fn pick_branches(p: &mut Prompter, have: &[String]) -> Result<Vec<String>, String> {
    let pre: Vec<String> = DEFAULT_BRANCHES
        .iter()
        .filter(|d| have.iter().any(|h| h == *d))
        .map(ToString::to_string)
        .collect();
    p.say("Which integration branches does it cover?");
    for (i, b) in have.iter().enumerate() {
        let mark = if pre.contains(b) { "[x]" } else { "[ ]" };
        let warn = if PROTECTED.contains(&b.as_str()) {
            "  (protected: refused)"
        } else {
            ""
        };
        p.say(&format!("  {}) {mark} {b}{warn}", i + 1));
    }
    if have.is_empty() {
        p.say("  (no branches found)");
    }
    let hint = if pre.is_empty() {
        "branches (numbers or names): ".to_string()
    } else {
        format!("branches (Enter = {}): ", pre.join(" "))
    };
    loop {
        let ans = p.ask(&hint)?;
        let picked: Vec<String> = if ans.is_empty() {
            pre.clone()
        } else {
            ans.split(|c: char| c == ',' || c.is_whitespace())
                .filter(|w| !w.is_empty())
                .map(|w| {
                    w.parse::<usize>()
                        .ok()
                        .and_then(|n| have.get(n.wrapping_sub(1)).cloned())
                        .unwrap_or_else(|| w.to_string())
                })
                .collect()
        };
        let list = gw::parse_branches(&picked.join(" "));
        match list {
            Err(m) => p.say(&format!("  {m}")),
            Ok(l) => {
                let bad = gw::protected_in(&l);
                if bad.is_empty() {
                    return Ok(l);
                }
                p.say(&format!(
                    "  {} is protected: a grant never covers it; pick again",
                    bad.join(", ")
                ));
            }
        }
    }
}

fn pick_expiry(p: &mut Prompter, now: i64) -> Result<i64, String> {
    let off = local_offset_at(now);
    let eod = (now + off).div_euclid(86_400) * 86_400 + 23 * 3_600 + 59 * 60 - off;
    let mut opts: Vec<(String, i64)> = vec![
        ("1h".into(), now + 3_600),
        ("4h".into(), now + 4 * 3_600),
        ("8h".into(), now + 8 * 3_600),
    ];
    if eod > now + 60 {
        opts.push(("end of day".into(), eod));
    }
    p.say("When does it expire?");
    for (i, (label, t)) in opts.iter().enumerate() {
        p.say(&format!(
            "  {}) {label}: {} ({} UTC)",
            i + 1,
            fmt_local(*t, local_offset_at(*t)),
            grant::format_utc(*t)
        ));
    }
    let custom = opts.len() + 1;
    p.say(&format!(
        "  {custom}) custom local datetime (YYYY-MM-DD HH:MM)"
    ));
    loop {
        let ans = p.ask("expiry: ")?;
        match ans.parse::<usize>() {
            Ok(n) if (1..=opts.len()).contains(&n) => return Ok(opts[n - 1].1),
            Ok(n) if n == custom => loop {
                let when = p.ask("local datetime: ")?;
                match gw::parse_until(&when, &local_offset_at)
                    .and_then(|t| gw::check_expiry(now, t).map(|()| t))
                {
                    Ok(t) => return Ok(t),
                    Err(m) => p.say(&format!("  {m}")),
                }
            },
            _ => p.say("  pick one of the numbers"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const NOW: i64 = 1_793_966_400; // 2026-11-06T12:00:00Z

    fn drive<T>(
        input: &str,
        f: impl FnOnce(&mut Prompter) -> Result<T, String>,
    ) -> (Result<T, String>, String) {
        let mut inp = Cursor::new(input.as_bytes().to_vec());
        let mut out: Vec<u8> = Vec::new();
        let r = f(&mut Prompter {
            inp: &mut inp,
            out: &mut out,
        });
        (r, String::from_utf8(out).unwrap())
    }

    fn have() -> Vec<String> {
        ["main", "develop", "dev", "feat/x"]
            .map(String::from)
            .to_vec()
    }

    #[test]
    fn actions_by_number_name_alias_and_all() {
        assert_eq!(drive("1 2\n", pick_actions).0, Ok(vec!["push", "pr"]));
        assert_eq!(drive("push,mr\n", pick_actions).0, Ok(vec!["push", "pr"]));
        assert_eq!(drive("all\n", pick_actions).0.unwrap().len(), 4);
        // An empty or unknown answer asks again: there is no default grant.
        let (r, out) = drive("\nfoo\nworkitems\n", pick_actions);
        assert_eq!(r, Ok(vec!["issues"]));
        assert!(out.contains("unknown action 'foo'"));
    }

    #[test]
    fn branches_preselect_develop_and_dev_and_refuse_protected() {
        let (r, out) = drive("\n", |p| pick_branches(p, &have()));
        assert_eq!(r, Ok(vec!["develop".to_string(), "dev".to_string()]));
        assert!(out.contains("1) [ ] main  (protected: refused)"));
        assert!(out.contains("2) [x] develop"));
        let (r, out) = drive("1\n4\n", |p| pick_branches(p, &have()));
        assert_eq!(r, Ok(vec!["feat/x".to_string()]));
        assert!(out.contains("main is protected"));
    }

    #[test]
    fn without_develop_or_dev_an_answer_is_needed() {
        let only = vec!["trunk".to_string()];
        let (r, _) = drive("\n1\n", |p| pick_branches(p, &only));
        assert_eq!(r, Ok(vec!["trunk".to_string()]));
    }

    #[test]
    fn expiry_presets_and_custom() {
        let (r, out) = drive("2\n", |p| pick_expiry(p, NOW));
        assert_eq!(r, Ok(NOW + 4 * 3_600));
        assert!(out.contains("UTC)"));
        let (r, out) = drive("9\n5\n2026-11-05 10:00\n2026-11-20 10:00\n", |p| {
            pick_expiry(p, NOW)
        });
        assert!(r.is_ok());
        assert!(out.contains("pick one of the numbers"));
        assert!(out.contains("in the past"));
    }

    #[test]
    fn closed_input_writes_nothing() {
        let (r, _) = drive("", pick_actions);
        assert!(r.unwrap_err().contains("nothing written"));
    }

    #[test]
    fn the_whole_dialogue() {
        let (r, _) = drive("1 3\n\n1\n", |p| interactive(p, NOW, &have()));
        let c = r.unwrap();
        assert_eq!(c.actions, ["push", "merge"]);
        assert_eq!(c.branches, ["develop", "dev"]);
        assert_eq!(c.expires, NOW + 3_600);
    }

    #[test]
    fn confirmation_defaults_to_no() {
        for (ans, want) in [
            ("y\n", true),
            ("YES\n", true),
            ("\n", false),
            ("n\n", false),
            ("", false),
        ] {
            let mut inp = Cursor::new(ans.as_bytes().to_vec());
            let mut out: Vec<u8> = Vec::new();
            let mut p = Prompter {
                inp: &mut inp,
                out: &mut out,
            };
            assert_eq!(confirm(&mut p, "ok? "), want, "{ans:?}");
        }
    }

    #[test]
    fn platforms_without_rules_are_named() {
        assert!(lifts_nothing(&["push"]).is_empty());
        let n = lifts_nothing(&["pr", "issues"]);
        assert_eq!(n.len(), 2);
        assert!(n[0].contains("GitLab merge requests"));
        assert!(n[1].contains("Azure DevOps Boards work items"));
    }

    #[test]
    fn the_help_table_names_every_action_and_alias() {
        for a in &ACTIONS {
            assert!(GRANT_HELP.contains(a.name), "{}", a.name);
            for al in a.aliases {
                assert!(GRANT_HELP.contains(al), "{al}");
            }
        }
    }
}
