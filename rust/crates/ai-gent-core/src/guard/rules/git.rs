//! Port of `plugins/git/hooks/guard.sh`.
use super::{any, attributed, re};
use crate::guard::reasons::decide;
use crate::guard::segment::View;
use crate::guard::{Ctx, Decision};
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
    if has(s, "push", None) {
        return decide("git.push", &[]);
    }
    if gh(s, "pr[[:space:]]+create") {
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
        let branch = (ctx.branch_of)(&working_dir(s, cwd)).unwrap_or_default();
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
