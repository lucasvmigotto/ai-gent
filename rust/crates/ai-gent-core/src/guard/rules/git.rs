//! Port of `plugins/git/hooks/guard.sh`.
use super::{any, attributed, re};
use crate::guard::reasons::decide;
use crate::guard::segment::{Shell, View};
use crate::guard::{Ctx, Decision, Event, grant};
use regex::Regex;

const GIT_RE: &str = r"^[[:space:]]*(sudo[[:space:]]+)?git([[:space:]]+(-[Cc][[:space:]]+[^[:space:]]+|--[a-z-]+(=[^[:space:]]+)?))*[[:space:]]+";

/// `has <subcommand regex> [<argument regex>]` of the shell guard.
fn has(segments: &[String], sub: &str, arg: Option<&str>) -> bool {
    let r = re(&format!("{GIT_RE}{sub}([[:space:]].*)?$"));
    let matched: Vec<&String> = segments.iter().filter(|s| r.is_match(s)).collect();
    if matched.is_empty() {
        return false;
    }
    match arg {
        None => true,
        Some(a) => {
            let ar = re(a);
            matched.iter().any(|s| ar.is_match(s))
        }
    }
}

fn gh(segments: &[String], sub: &str) -> bool {
    any(segments, &re(&format!("^[[:space:]]*gh[[:space:]]+{sub}")))
}

/// Evaluate the git guard on a shell command.
#[must_use]
pub fn evaluate(v: &View, cwd: &str, ctx: &Ctx) -> Decision {
    let s = &v.segments;
    if has(
        s,
        "(commit|merge|push|am|rebase|cherry-pick|revert)",
        Some(r"(^|[[:space:]])--no-verify([[:space:]]|$)"),
    ) || has(s, "commit", Some(r"(^|[[:space:]])-n([[:space:]]|$)"))
    {
        return decide("git.nohook", &[]);
    }
    if (has(s, "(commit|merge|revert|cherry-pick|tag|notes)", None)
        || gh(s, "(issue|pr)([[:space:]]|$)"))
        && attributed(&v.raw)
    {
        return decide("git.attrib", &[]);
    }
    if has(
        s,
        "push",
        Some(r"(^|[[:space:]])(--force|-[a-zA-Z]*f[a-zA-Z]*)([[:space:]]|$)"),
    ) && !has(s, "push", Some("--force-with-lease"))
    {
        return decide("git.force", &[]);
    }
    // Nothing but the user writes the grant: a command that names it and is not a read.
    if v.raw.contains(grant::FILE)
        && (v.raw.contains('>')
            || s.iter().any(|l| {
                l.contains(grant::FILE)
                    && !re(r"^[[:space:]]*(cat|ls|head|tail|grep|stat|wc|test|\[)[[:space:]]")
                        .is_match(l)
            }))
    {
        return decide("git.grant_file", &[]);
    }
    if autonomy_command(&v.raw) {
        return decide("git.autonomy_cmd", &[]);
    }
    if has(s, "push", None) && !push_lifted(s, cwd, ctx) {
        return decide("git.push", &[]);
    }
    if gh(s, "pr[[:space:]]+create") && !pr_create_lifted(s, cwd, ctx) {
        return decide("git.pr_create", &[]);
    }
    if has(
        s,
        "branch",
        Some(
            r"(^|[[:space:]])(-D|--delete[[:space:]]+--force|--force[[:space:]]+--delete)([[:space:]]|$)",
        ),
    ) {
        return decide("git.branch_d", &[]);
    }
    if has(s, "reset", Some("--hard"))
        || has(s, "clean", Some(r"(^|[[:space:]])-[a-zA-Z]*f"))
        || has(s, "commit", Some("--amend"))
        || has(s, "(filter-branch|filter-repo)", None)
    {
        return decide("git.rewrite", &[]);
    }
    if has(s, "(commit|merge)", None) {
        let (base, cd) = match cd_prefix(&decoded(v), cwd, s) {
            Cd::Bad => return decide("git.cd_unresolved", &[]),
            Cd::None => (cwd.to_string(), false),
            Cd::Dir(d) => (d, true),
        };
        let mut dir = working_dir(s, &base);
        if cd && dir != base && !dir.starts_with('/') {
            dir = format!("{base}/{dir}"); // a relative -C is relative to the cd
        }
        let branch = (ctx.branch_of)(&dir).unwrap_or_default();
        match branch.as_str() {
            "main" | "master" => return decide("git.on_main", &[("branch", &branch)]),
            "homolog" | "staging" => return decide("git.on_chain", &[("branch", &branch)]),
            "dev" | "develop" if has(s, "commit", None) => {
                return decide("git.on_dev", &[("branch", &branch)]);
            }
            _ => {}
        }
    }
    Decision::none()
}

/// A command that runs the grant CLI in any form but show/status/--help/-h, or `autonomy
/// [flags] grant|revoke` without the word ai-gent. Tested on the raw command and on a copy with
/// quotes, backslashes and `$( )` stripped. The real defence is structural (the write path needs
/// a real terminal); this is defence in depth.
fn autonomy_command(raw: &str) -> bool {
    let plain: String = raw
        .chars()
        .filter(|c| !matches!(c, '\'' | '"' | '`' | '\\'))
        .collect::<String>()
        .replace("$(", "")
        .replace(')', "");
    autonomy_denied(raw) || autonomy_denied(&plain)
}

fn autonomy_denied(text: &str) -> bool {
    let t = text.replace('\n', " ");
    let w = "(^|[^A-Za-z0-9_-])";
    if re(&format!(
        r"{w}autonomy([[:space:]]+-[^[:space:]]*)*[[:space:]]+(grant|revoke)([^A-Za-z0-9_-]|$)"
    ))
    .is_match(&t)
    {
        return true;
    }
    if !re(&format!(r"{w}ai-gent(\.exe)?([^A-Za-z0-9_/.-]|$)")).is_match(&t) {
        return false;
    }
    let rest = re(&format!(
        r"{w}autonomy[[:space:]]+(show|status|--help|-h)([^A-Za-z0-9_-]|$)"
    ))
    .replace_all(&t, " ");
    re(&format!(r"{w}autonomy([^A-Za-z0-9_-]|$)")).is_match(&rest)
}

/// A push the active grant lifts: every push names no protected branch and no bulk or forced
/// form, and runs from a branch that is not a protected one either.
fn push_lifted(segments: &[String], cwd: &str, ctx: &Ctx) -> bool {
    let dir = working_dir(segments, cwd);
    if !grant::load(&dir).is_some_and(|g| g.has("push")) {
        return false;
    }
    let branch = (ctx.branch_of)(&dir).unwrap_or_default();
    if matches!(
        branch.as_str(),
        "" | "main" | "master" | "homolog" | "staging"
    ) {
        return false;
    }
    let sel = re(&format!("{GIT_RE}push([[:space:]].*)?$"));
    let bad = re(
        r"(^|[[:space:]])(--all|--mirror|--tags|--delete|--prune|-d)([[:space:]]|$)|(^|[[:space:]:/])(main|master|homolog|staging)([[:space:]]|$)|(^|[[:space:]])\+",
    );
    !segments
        .iter()
        .filter(|s| sel.is_match(s))
        .any(|s| bad.is_match(s))
}

/// A `gh pr create` the active grant lifts: it names a `--base` the grant covers.
fn pr_create_lifted(segments: &[String], cwd: &str, _ctx: &Ctx) -> bool {
    let Some(g) = grant::load(&working_dir(segments, cwd)).filter(|g| g.has("pr")) else {
        return false;
    };
    let create = re(r"^[[:space:]]*gh[[:space:]]+pr[[:space:]]+create");
    let base = re(r"^.*[[:space:]](--base|-B)[[:space:]=]+([^[:space:]]+)");
    segments
        .iter()
        .filter(|s| create.is_match(s))
        .find_map(|s| base.captures(s).map(|c| c[2].to_string()))
        .is_some_and(|b| g.covers(&b))
}

/// An Edit, Write or similar tool aimed at the grant file.
#[must_use]
pub fn grant_file_tool(ev: &Event) -> Decision {
    let target = ["file_path", "notebook_path"]
        .iter()
        .find_map(|k| ev.tool_input.get(*k).and_then(|v| v.as_str()));
    match target {
        Some(t) if t == grant::FILE || t.ends_with(&format!("/{}", grant::FILE)) => {
            decide("git.grant_file", &[])
        }
        _ => Decision::none(),
    }
}

/// What the leading `cd` segments of a command say about where it runs.
enum Cd {
    /// No `cd` anywhere: the session's cwd.
    None,
    /// Leading `cd DIR &&` / `cd DIR;` segments, resolved against the cwd.
    Dir(String),
    /// A `cd` the guard cannot follow (variable, `~`, `-`, option, later or nested `cd`, ...).
    Bad,
}

/// The command as typed: `raw` is JSON-escaped for `sh`.
fn decoded(v: &View) -> String {
    if v.shell != Shell::Sh {
        return v.raw.clone();
    }
    let mut out = String::with_capacity(v.raw.len());
    let mut it = v.raw.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(o) => out.push(o),
            None => out.push('\\'),
        }
    }
    out
}

fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c')
}

/// Follow the leading `cd DIR` segments joined by `&&` or `;`. A path is a plain word or a quoted
/// string with no `$`, backtick, backslash, glob or `~`; any other `cd` (also after the leading
/// ones, in a subshell or after `||`) is `Bad`, so the caller keeps asking.
fn cd_prefix(raw: &str, cwd: &str, segments: &[String]) -> Cd {
    let word = re(r"(^|[[:space:]])(cd|pushd|popd)([[:space:]]|$)");
    let total = segments.iter().filter(|s| word.is_match(s)).count();
    let mut rest = raw;
    let mut dir = if cwd.is_empty() {
        ".".to_string()
    } else {
        cwd.to_string()
    };
    let mut n = 0;
    loop {
        let t = rest.trim_start_matches(is_ws);
        let Some(after) = t.strip_prefix("cd").filter(|a| a.starts_with(is_ws)) else {
            break;
        };
        let a = after.trim_start_matches(is_ws);
        let (arg, tail) = if let Some(q @ ('"' | '\'')) = a.chars().next() {
            let Some(pair) = a[1..].split_once(q) else {
                return Cd::Bad;
            };
            pair
        } else {
            let end = a
                .find(|c: char| is_ws(c) || ";&|()<>".contains(c))
                .unwrap_or(a.len());
            a.split_at(end)
        };
        if arg.is_empty()
            || arg.starts_with('-')
            || arg.contains(|c: char| "$`\\*?[]{}~\"'!#\n\r".contains(c))
        {
            return Cd::Bad;
        }
        let tail = tail.trim_start_matches(is_ws);
        rest = if let Some(r) = tail.strip_prefix("&&") {
            r
        } else if let Some(r) = tail.strip_prefix(';') {
            r
        } else {
            return Cd::Bad;
        };
        dir = if arg.starts_with('/') {
            arg.to_string()
        } else {
            format!("{dir}/{arg}")
        };
        n += 1;
    }
    if n != total {
        Cd::Bad
    } else if n == 0 {
        Cd::None
    } else {
        Cd::Dir(dir)
    }
}

/// The directory whose branch counts: the `-C` argument of the first segment that has one (the
/// last `git -C` on that segment, as the shell guard's greedy `sed`), else the session's cwd.
fn working_dir(segments: &[String], cwd: &str) -> String {
    let r: Regex = re(r"^.*git[[:space:]]+-C[[:space:]]+([^[:space:]]*)");
    for s in segments {
        if let Some(c) = r.captures(s) {
            let d = c.get(1).map_or("", |m| m.as_str());
            if !d.is_empty() {
                return d.to_string();
            }
            break; // `head -n1` took an empty first match: the cwd counts
        }
    }
    if cwd.is_empty() {
        ".".into()
    } else {
        cwd.to_string()
    }
}
