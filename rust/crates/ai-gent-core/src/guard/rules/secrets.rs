//! Port of `plugins/secrets/hooks/guard.sh`.
use super::{PREFIX, any, re};
use crate::guard::Decision;
use crate::guard::reasons::decide;
use crate::guard::segment::View;

const PRINTERS: [&str; 19] = [
    "env", "printenv", "set", "export", "declare", "typeset", "echo", "printf", "cat", "sh",
    "bash", "dash", "zsh", "ksh", "fish", "node", "perl", "ruby", "eval",
];

/// Evaluate the secrets guard on a shell command.
#[must_use]
pub fn evaluate(v: &View) -> Decision {
    let s = &v.segments;
    let gpg = re(&format!(r"{PREFIX}([^[:space:]]*/)?gpg2?([[:space:]]|$)"));
    if any(s, &gpg) && re(r"\.password-store|PASSWORD_STORE_DIR").is_match(&v.raw) {
        return decide("secrets.gpg", &[]);
    }

    let pass_run = re(&format!(
        r"{PREFIX}([^[:space:]]*/)?pass-run([[:space:]]|$)"
    ));
    if let Some(runner) = s.iter().find(|l| pass_run.is_match(l)) {
        if re(r"(^|[=:[:space:]])([A-Za-z0-9_.@+-]+/)?ai-gent/db/").is_match(&v.raw) {
            return decide("secrets.run_db", &[]);
        }
        let words: Vec<&str> = runner
            .split([' ', '\t'])
            .filter(|w| !w.is_empty())
            .collect();
        let after = words
            .iter()
            .take(words.len().saturating_sub(1))
            .position(|w| *w == "--")
            .map_or("", |i| words[i + 1]);
        let base = after.rsplit('/').next().unwrap_or("");
        if PRINTERS.contains(&base) || base.starts_with("python") {
            return decide("secrets.run_print", &[]);
        }
    }

    let pass = re(&format!(r"{PREFIX}([^[:space:]]*/)?pass([[:space:]]|$)"));
    let Some(hit) = s.iter().find(|l| pass.is_match(l)) else {
        return Decision::none();
    };
    let rest = pass.replace(hit, "");
    let clip = re(r"^(-c|--clip|-c[0-9].*|--clip=.*|-q|--qrcode|-q[0-9].*|--qrcode=.*)$");
    let mut sub = "";
    for arg in rest.split([' ', '\t', '\n']).filter(|a| !a.is_empty()) {
        if clip.is_match(arg) {
            return decide("secrets.clip", &[]);
        }
        if arg.starts_with('-') {
            continue;
        }
        if sub.is_empty() {
            sub = arg;
        }
    }
    match sub {
        "" | "ls" | "list" | "find" | "search" | "help" | "version" => Decision::none(),
        "insert" | "add" => decide("secrets.insert", &[]),
        "rm" | "remove" | "delete" | "mv" | "rename" | "cp" | "copy" | "init" | "git" => {
            decide("secrets.store_change", &[("sub", sub)])
        }
        _ => decide("secrets.show", &[]),
    }
}
