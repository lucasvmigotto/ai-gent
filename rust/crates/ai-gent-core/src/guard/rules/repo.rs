//! Port of `plugins/repo/hooks/guard.sh`.
use super::{attributed, re};
use crate::guard::reasons::decide;
use crate::guard::segment::View;
use crate::guard::{Decision, grant};

/// Whether the active grant lifts one reporun write segment: `reporun cli github`, then the gh
/// words (`pr create --base <covered>`, `pr merge`, `issue create|comment|edit`).
fn lifted(segment: &str, cwd: &str) -> bool {
    let head = re(r"^.*reporun(\.py)?[[:space:]]+cli[[:space:]]+github[[:space:]]*");
    if !head.is_match(segment) {
        return false;
    }
    let words = head.replace(segment, "").to_string();
    let words = re(r"--write([[:space:]]|$)")
        .replace_all(&words, "")
        .to_string();
    let words = re(r"--entry[[:space:]]+[^[:space:]]+")
        .replace(&words, "")
        .to_string();
    let words = re(r"^[[:space:]]*(--[[:space:]]+)?(gh[[:space:]]+)?")
        .replace(&words, "")
        .to_string();
    let mut fields = words.split_whitespace();
    let (noun, verb) = (fields.next().unwrap_or(""), fields.next().unwrap_or(""));
    let Some(g) = grant::load(cwd) else {
        return false;
    };
    match (noun, verb) {
        ("pr", "create") => {
            g.has("pr")
                && re(r"^.*[[:space:]](--base|-B)[[:space:]=]+([^[:space:]]+)")
                    .captures(&words)
                    .is_some_and(|c| g.covers(&c[2]))
        }
        ("pr", "merge") => g.has("merge"),
        ("issue", "create" | "comment" | "edit") => g.has("issues"),
        _ => false,
    }
}

/// Evaluate the repo guard on a shell command.
#[must_use]
pub fn evaluate(v: &View, cwd: &str) -> Decision {
    let runner = re(
        r"^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*[[:space:]]+)*((python3?|uv[[:space:]]+run([[:space:]]+python3?)?)[[:space:]]+)?([^[:space:]]*/)?reporun(\.py)?[[:space:]]",
    );
    let write = re(r"[[:space:]]--write([[:space:]]|$)");
    let writes: Vec<&String> = v
        .segments
        .iter()
        .filter(|s| runner.is_match(s) && write.is_match(s))
        .collect();
    if writes.is_empty() {
        return Decision::none();
    }
    if attributed(&v.raw) {
        return decide("repo.attrib", &[]);
    }
    if writes.iter().all(|s| lifted(s, cwd)) {
        return Decision::none();
    }
    decide("repo.write", &[])
}
