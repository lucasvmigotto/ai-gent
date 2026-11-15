//! Port of `plugins/secrets/hooks/guard.sh`.
use super::{PREFIX, any, re};
use crate::guard::Decision;
use crate::guard::passallow::{Source, clean_entry};
use crate::guard::reasons::decide;
use crate::guard::segment::{Shell, View};

const PRINTERS: [&str; 19] = [
    "env", "printenv", "set", "export", "declare", "typeset", "echo", "printf", "cat", "sh",
    "bash", "dash", "zsh", "ksh", "fish", "node", "perl", "ruby", "eval",
];

/// Evaluate the secrets guard on a shell command.
#[must_use]
pub fn evaluate(v: &View) -> Decision {
    evaluate_with(v, &Source::current())
}

/// Evaluate the secrets guard with the allowlist read from `src`.
#[must_use]
pub fn evaluate_with(v: &View, src: &Source) -> Decision {
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

    // Whatever names the allowlist file is the user's to approve.
    let var = src.allow_var.as_deref().filter(|s| !s.is_empty());
    if v.raw.contains("pass-allow")
        || v.raw.contains("AI_GENT_PASS_ALLOW")
        || var.is_some_and(|p| v.raw.contains(p))
    {
        return decide("secrets.allowlist", &[]);
    }

    let pass = re(&format!(r"{PREFIX}([^[:space:]]*/)?pass([[:space:]]|$)"));
    let Some(hit) = s.iter().find(|l| pass.is_match(l)) else {
        return Decision::none();
    };
    let rest = pass.replace(hit, "");
    let clip = re(r"^(-c|--clip|-c[0-9].*|--clip=.*|-q|--qrcode|-q[0-9].*|--qrcode=.*)$");
    let npass = s.iter().filter(|l| pass.is_match(l)).count();
    let (mut sub, mut entry, mut flags, mut args) = ("", "", 0, 0);
    for arg in rest.split([' ', '\t', '\n']).filter(|a| !a.is_empty()) {
        if clip.is_match(arg) {
            return decide("secrets.clip", &[]);
        }
        if arg.starts_with('-') {
            flags += 1;
            continue;
        }
        args += 1;
        if sub.is_empty() {
            sub = arg;
        } else if entry.is_empty() {
            entry = arg;
        }
    }
    match sub {
        "" | "ls" | "list" | "find" | "search" | "help" | "version" => Decision::none(),
        "insert" | "add" => decide("secrets.insert", &[]),
        "rm" | "remove" | "delete" | "mv" | "rename" | "cp" | "copy" | "init" | "git" => {
            decide("secrets.store_change", &[("sub", sub)])
        }
        _ => {
            // Only `pass show <entry>`: no flags, one clean entry, the only pass command in the
            // line, no store or config override in the text. The allowlist is the POSIX shell
            // guard's: PowerShell and cmd.exe keep the plain denial (fixture secrets-041).
            let eligible = v.shell == Shell::Sh
                && sub == "show"
                && flags == 0
                && args == 2
                && npass == 1
                && clean_entry(entry)
                && !re(r"PASSWORD_STORE_|XDG_CONFIG_HOME|(^|[^A-Za-z0-9_])HOME=").is_match(&v.raw);
            if eligible {
                if src.allows(entry) {
                    return Decision::none();
                }
                return decide("secrets.show_hint", &[]);
            }
            decide("secrets.show", &[])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard::segment::{Shell, view};
    use std::rc::Rc;

    fn run(cmd: &str, var: Option<&str>, file: Option<&'static str>) -> (String, String) {
        let src = Source {
            allow_var: var.map(str::to_string),
            xdg: None,
            home: Some("/h".into()),
            read: Rc::new(move |_| file.map(str::to_string)),
        };
        let d = evaluate_with(&view(Shell::Sh, cmd, false), &src);
        (d.level.name().to_string(), d.reason)
    }

    const LIST: Option<&str> = Some("# mine\nai-gent/github/pat\nwork/\nbad line ??\n");

    #[test]
    fn the_decision_table() {
        let hint = crate::guard::reasons::decide("secrets.show_hint", &[]).reason;
        let plain = crate::guard::reasons::decide("secrets.show", &[]).reason;
        let ask = crate::guard::reasons::decide("secrets.allowlist", &[]).reason;
        let cases: [(&str, Option<&str>, (&str, &str)); 11] = [
            ("pass show ai-gent/github/pat", LIST, ("none", "")),
            ("pass show work/vpn", LIST, ("none", "")),
            ("pass show other/x", LIST, ("deny", &hint)),
            ("pass show ai-gent/github/pat", None, ("deny", &hint)),
            ("pass show -n ai-gent/github/pat", LIST, ("deny", &plain)),
            ("pass ai-gent/github/pat", LIST, ("deny", &plain)),
            (
                "pass show ai-gent/db/x",
                Some("ai-gent/\nai-gent/db/x\n"),
                ("deny", &plain),
            ),
            ("pass show work/vpn | cat", LIST, ("none", "")),
            (
                "pass show work/vpn; pass show work/b",
                LIST,
                ("deny", &plain),
            ),
            ("HOME=/t pass show work/vpn", LIST, ("deny", &plain)),
            ("cat ~/.config/ai-gent/pass-allow", LIST, ("ask", &ask)),
        ];
        for (cmd, file, (level, reason)) in cases {
            assert_eq!(run(cmd, None, file), (level.into(), reason.into()), "{cmd}");
        }
    }

    #[test]
    fn mentioning_the_file_or_its_variable_asks() {
        let ask = crate::guard::reasons::decide("secrets.allowlist", &[]).reason;
        for (cmd, var) in [
            ("echo x >> $AI_GENT_PASS_ALLOW", None),
            ("echo work/ >> /tmp/mine.txt", Some("/tmp/mine.txt")),
        ] {
            assert_eq!(run(cmd, var, LIST), ("ask".into(), ask.clone()), "{cmd}");
        }
    }
}
