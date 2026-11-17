//! Port of `plugins/git/hooks/guard.sh`.
use super::{any, attributed, re};
use crate::guard::reasons::decide;
use crate::guard::segment::{Shell, View};
use crate::guard::{Ctx, Decision, Event, grant};
use regex::Regex;

const GIT_RE: &str = r"^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*[[:space:]]+)*((sudo|env|exec|command|nohup|xargs|time)[[:space:]]+([^[:space:];&|()]+[[:space:]]+)*)?git([[:space:]]+(-[Cc][[:space:]]+[^[:space:]]+|-[Cc][^[:space:]]+|-[A-Za-z]|--[a-z-]+(=[^[:space:]]+)?))*[[:space:]]+";

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

/// The segments, plus those of the text inside quotes when the command runs a shell with `-c`
/// (`sh -c 'git commit'`): the quoted text is blanked in `segments`, so the commit would go unseen.
fn with_inner(v: &View) -> Vec<String> {
    let mut out = v.segments.clone();
    if re(r"(^|[ \t;&|(/])(sh|bash|zsh|dash|ksh)[ \t]+-[A-Za-z]*c").is_match(&stripped(&v.raw)) {
        let t = v.raw.replace("\\n", "\n");
        let t: String = t
            .chars()
            .map(|c| {
                if matches!(c, '\'' | '"' | '`' | ';' | '&' | '|' | '(' | ')') {
                    '\n'
                } else {
                    c
                }
            })
            .collect();
        out.extend(t.split('\n').map(str::to_string));
    }
    out
}

/// The command with quotes and backslashes removed (the `plain` copy of `autonomy_command`).
fn stripped(raw: &str) -> String {
    raw.chars()
        .filter(|c| !matches!(c, '\'' | '"' | '\\'))
        .collect()
}

/// `segments` of the stripped copy: `git "commit"`, `git c\ommit` and `--git-""dir` read as typed
/// to the shell. Used to recognise a commit, never to read an option's value or a deny rule.
fn stripped_segments(raw: &str) -> Vec<String> {
    let t = stripped(&raw.replace("\\n", "\n").replace("\\t", " "));
    t.chars()
        .map(|c| {
            if matches!(c, ';' | '&' | '|' | '(' | ')') {
                '\n'
            } else {
                c
            }
        })
        .collect::<String>()
        .split('\n')
        .map(str::to_string)
        .collect()
}

/// Matched against an ASCII-lowercased copy (case-insensitive without the Unicode tables).
const HAZARD: &str = r"git_|--git-dir|--work-tree|--exec-path|--config-env|core\.worktree|core\.hookspath|cdpath|(^|[[:space:]])-c[[:space:]]*(core|alias|include)\.";

/// A repository, work tree, hook path or config given before a commit or merge subcommand, or a
/// `GIT_*` or `CDPATH` assignment on its own: the branch the guard reads may not be the one used.
fn prefix_hazard(st: &[String]) -> bool {
    let pre = re(&format!("{GIT_RE}(commit|merge)"));
    let hz = re(HAZARD);
    let assign = re(r"^[[:space:]]*(export[[:space:]]+)?(GIT_[A-Za-z_]*|CDPATH)=");
    st.iter().any(|s| {
        pre.find(s)
            .is_some_and(|m| hz.is_match(&m.as_str().to_ascii_lowercase()))
            || assign.is_match(s)
    })
}

/// `git -c alias.x=...` in any spacing, or `git config alias.*` next to another git command.
fn alias_use(st: &[String]) -> bool {
    let inline = re(r"(^|[[:space:]])git[[:space:]].*-c[[:space:]]*alias\.");
    if st.iter().any(|s| inline.is_match(&s.to_ascii_lowercase())) {
        return true;
    }
    let cfg = re(
        r"(^|[[:space:]])git[[:space:]]+(-[^[:space:]]+[[:space:]]+)*config[[:space:]].*alias\.",
    );
    let git = re(&format!("{GIT_RE}[a-z]"));
    st.iter().any(|s| cfg.is_match(&s.to_ascii_lowercase()))
        && st.iter().filter(|s| git.is_match(s)).count() >= 2
}

/// A git command that can move HEAD or a branch.
fn branch_change(st: &[String]) -> bool {
    has(
        st,
        "(checkout|switch|worktree|reset|update-ref|symbolic-ref)",
        None,
    ) || has(
        st,
        "branch",
        Some(r"(^|[[:space:]])(-m|-M|--move)([[:space:]]|$)"),
    )
}

/// Evaluate the git guard on a shell command.
#[must_use]
pub fn evaluate(v: &View, cwd: &str, ctx: &Ctx) -> Decision {
    let combined = with_inner(v);
    let s = &combined;
    let st = stripped_segments(&v.raw);
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
    if alias_use(&st) {
        return decide("git.alias", &[]);
    }
    let seen = |sub: &str| has(s, sub, None) || has(&st, sub, None);
    if seen("(commit|merge)") {
        if prefix_hazard(&st) {
            return decide("git.cd_unresolved", &[]);
        }
        let (base, cd) = match cd_prefix(&decoded(v), &v.raw, cwd) {
            Cd::Bad => return decide("git.cd_unresolved", &[]),
            Cd::None => (cwd.to_string(), false),
            Cd::Dir(d) => (d, true),
        };
        if cd && branch_change(&st) {
            return decide("git.cd_unresolved", &[]);
        }
        let mut dir = working_dir(s, &base);
        if dir.contains(['"', '\'']) {
            return decide("git.cd_unresolved", &[]);
        }
        if cd && dir != base && !dir.starts_with('/') {
            if dir.contains("..") {
                return decide("git.cd_unresolved", &[]);
            }
            dir = format!("{base}/{dir}"); // a relative -C is relative to the cd
        }
        let branch = (ctx.branch_of)(&dir).unwrap_or_default();
        if cd && branch.is_empty() {
            return decide("git.cd_unresolved", &[]);
        }
        match branch.as_str() {
            "main" | "master" => return decide("git.on_main", &[("branch", &branch)]),
            "homolog" | "staging" => return decide("git.on_chain", &[("branch", &branch)]),
            "dev" | "develop" if seen("commit") => {
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
fn cd_prefix(text: &str, raw: &str, cwd: &str) -> Cd {
    let total = cd_like(raw);
    let mut rest = text;
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
            || arg.contains("..")
            || arg.contains(|c: char| "$`\\*?[]{}~\"'!#\n\r".contains(c))
        {
            return Cd::Bad;
        }
        let tail = tail.trim_start_matches(is_ws);
        let Some(r) = tail.strip_prefix("&&") else {
            return Cd::Bad;
        };
        rest = r;
        dir = if arg.starts_with('/') {
            arg.to_string()
        } else {
            format!("{dir}/{arg}")
        };
        n += 1;
    }
    if n != total || (n > 0 && (cd_hazard(raw) || !rest_ok(rest))) {
        Cd::Bad
    } else if n == 0 {
        Cd::None
    } else {
        Cd::Dir(dir)
    }
}

/// Text that changes where git runs after a followed `cd`, searched on the copy with quotes and
/// backslashes removed so splitting a word with quotes does not hide it: any `GIT_` variable,
/// `--git-dir`, `--work-tree`, `--exec-path`, `--config-env`, `core.worktree`, `-c core.*` or
/// `-c alias.*`, `CDPATH`.
fn cd_hazard(raw: &str) -> bool {
    re(HAZARD).is_match(&stripped(raw).to_ascii_lowercase())
}

/// After the leading `cd DIR &&` chain only plain git commands may follow: every segment (split on
/// `&&` outside quotes) starts with the literal word `git`, and outside quotes there is no `<`,
/// `$`, backtick, `;`, `|`, `(`, `)`, `{`, `}`, newline or lone `&` (`>&` is fine); inside double
/// quotes no backtick, `$(` or `${`. Anything else asks.
fn rest_ok(rest: &str) -> bool {
    let d: Vec<char> = rest.trim_end_matches(is_ws).chars().collect();
    let word = |t: &[char]| {
        let t: String = t.iter().collect();
        let t = t.trim_start_matches(is_ws);
        t.split(is_ws).next() == Some("git")
    };
    let (mut q, mut start, mut i) = (0u8, 0usize, 0usize);
    while i < d.len() {
        let c = d[i];
        let nx = d.get(i + 1).copied();
        match q {
            1 => {
                if c == '\'' {
                    q = 0;
                }
            }
            2 => match c {
                '"' => q = 0,
                '\\' => i += 1,
                '`' => return false,
                '$' if matches!(nx, Some('(' | '{')) => return false,
                _ => {}
            },
            _ => match c {
                '\'' => q = 1,
                '"' => q = 2,
                '\\' => i += 1,
                '&' if nx == Some('&') => {
                    if !word(&d[start..i]) {
                        return false;
                    }
                    start = i + 2;
                    i += 1;
                }
                '&' if i > 0 && d[i - 1] == '>' => {}
                '&' | '<' | '$' | '`' | ';' | '|' | '(' | ')' | '{' | '}' | '\n' | '\r' => {
                    return false;
                }
                _ => {}
            },
        }
        i += 1;
    }
    q == 0 && word(&d[start.min(d.len())..])
}

/// How many words of the command can change the working directory or run other code that can:
/// `cd`, `pushd`, `popd` anywhere; `eval`, `exec`, `source`, `.` as the command of a segment;
/// `sh -c` and friends; `env -C`. Counted on a copy with quotes and backslashes stripped (the
/// `plain` copy of `autonomy_command`), so `"cd"`, `\cd` and `eval 'cd ..'` count.
fn cd_like(raw: &str) -> usize {
    let mut t = raw
        .replace("\\n", "\n")
        .replace("\\t", " ")
        .replace("\\r", " ");
    t.retain(|c| !matches!(c, '\'' | '"' | '\\'));
    let t: String = t
        .chars()
        .map(|c| {
            if matches!(c, '`' | ';' | '&' | '|' | '(' | ')') {
                '\n'
            } else {
                c
            }
        })
        .collect();
    let skip = [
        "then", "do", "else", "elif", "if", "while", "until", "{", "!", "time", "command",
        "builtin", "nohup", "sudo",
    ];
    let mut count = 0;
    for line in t.split('\n') {
        let w: Vec<&str> = line.split([' ', '\t']).filter(|x| !x.is_empty()).collect();
        count += w
            .iter()
            .filter(|x| matches!(**x, "cd" | "pushd" | "popd"))
            .count();
        if let Some(first) = w.iter().find(|x| !skip.contains(*x))
            && matches!(*first, "eval" | "exec" | "source" | ".")
        {
            count += 1;
        }
        for (i, x) in w.iter().enumerate() {
            if matches!(*x, "sh" | "bash" | "zsh" | "dash" | "ksh")
                && w.get(i + 1)
                    .is_some_and(|y| y.starts_with('-') && !y.starts_with("--") && y.contains('c'))
            {
                count += 1;
            }
            if *x == "env"
                && w[i + 1..].iter().any(|y| {
                    y.starts_with("--chdir")
                        || (y.starts_with('-') && !y.starts_with("--") && y.contains('C'))
                })
            {
                count += 1;
            }
        }
    }
    count
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
