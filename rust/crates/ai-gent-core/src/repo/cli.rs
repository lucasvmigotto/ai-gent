//! The CLI path: classify a call as read or write, check it, and run the platform's own CLI with the
//! token in that process's environment only.
use super::entry::{check_entry, default_entry, pass_has, read_entry};
use super::platforms::{BLOCKED_FLAGS, blocked_cli, is_read_verb, platform};
use super::remote::detect_repo;
use super::{Ctx, Failed, R, note};
use crate::env::Env;
use crate::exec;
use crate::passstore::redact;
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

fn is_flag(w: &str) -> bool {
    w.starts_with('-')
}

/// Whether a CLI call changes something. Unknown is a write, and so are options before the
/// subcommand (`gh -R o/r pr merge`): put them after it.
#[must_use]
pub fn cli_is_write(cli: &str, words: &[String]) -> bool {
    if let Some(first) = words.first() {
        if is_flag(first) && !["-h", "--help", "--version"].contains(&first.as_str()) {
            return true;
        }
    }
    let positional: Vec<&str> = words
        .iter()
        .take_while(|w| !is_flag(w))
        .map(String::as_str)
        .collect();
    if positional.is_empty() {
        return false; // bare `gh`, `gh --help`
    }
    if positional[0] == "api" && (cli == "gh" || cli == "glab") {
        let mut method = "GET".to_string();
        for (i, w) in words.iter().enumerate() {
            if (w == "-X" || w == "--method") && i + 1 < words.len() {
                method = words[i + 1].to_uppercase();
            } else if let Some(m) = w.strip_prefix("--method=") {
                method = m.to_uppercase();
            } else if ["-f", "-F", "--field", "--raw-field", "--input"].contains(&w.as_str())
                || ["--field=", "--raw-field=", "--input="]
                    .iter()
                    .any(|p| w.starts_with(p))
            {
                if method == "GET" {
                    method = "POST".to_string();
                }
            }
        }
        return method != "GET" && method != "HEAD";
    }
    if cli == "az" && positional.len() >= 2 && positional[..2] == ["devops", "invoke"] {
        for (i, w) in words.iter().enumerate() {
            if w == "--http-method" && i + 1 < words.len() {
                let m = words[i + 1].to_uppercase();
                return m != "GET" && m != "HEAD";
            }
            if w == "--in-file" {
                return true;
            }
        }
        return false;
    }
    if cli == "az" && positional.len() >= 2 && positional[..2] == ["boards", "query"] {
        return false;
    }
    // The verb sits at a fixed place: gh, glab and jira are `<noun> <verb>` (a lone word is the verb);
    // az is `<group...> <verb>`, the last word before the options.
    let verb = if cli == "az" {
        positional[positional.len() - 1]
    } else if positional.len() > 1 {
        positional[1]
    } else {
        positional[0]
    };
    !is_read_verb(verb)
}

/// A checked command line and its environment.
pub struct Prepared {
    /// Program and arguments.
    pub argv: Vec<String>,
    /// Environment of the child (holds the token).
    pub env: Env,
}

/// The checked command line and its environment (token from pass, or the CLI's own login).
///
/// # Errors
/// Usage errors (no CLI), refusals (blocked verbs, a write without `--write`), missing entries.
pub fn prepare_cli(
    ctx: &Ctx,
    plat: &str,
    words: &[String],
    entry_arg: Option<&str>,
    write: bool,
) -> R<Prepared> {
    let p = platform(plat).ok_or_else(|| Failed::usage(format!("unknown platform {plat}")))?;
    let Some(cli) = p.cli else {
        return Err(Failed::usage(format!(
            "{plat} has no CLI; use `ai-gent repo api {plat} ...`"
        )));
    };
    let mut words: Vec<String> = words.to_vec();
    if words.first().is_some_and(|w| w == cli) {
        words.remove(0);
    }
    if exec::which(cli, &ctx.env).is_none() {
        return Err(Failed::usage(format!(
            "{cli} isn't installed; use `ai-gent repo api {plat} ...` (the REST API) instead"
        )));
    }
    let positional: Vec<&String> = words.iter().filter(|w| !is_flag(w)).take(2).collect();
    if let Some(first) = positional.first() {
        if blocked_cli(cli).contains(&first.as_str()) {
            return Err(Failed::refused(format!(
                "`{cli} {first}` can print a token, run arbitrary code or change the CLI's own login; not through ai-gent repo"
            )));
        }
    }
    if words
        .iter()
        .any(|w| BLOCKED_FLAGS.contains(&w.as_str()) || w.starts_with("--debug"))
    {
        return Err(Failed::refused(
            "debug and token-printing flags can log the auth header; drop them",
        ));
    }
    let head3 = words.iter().take(3).cloned().collect::<Vec<_>>().join(" ");
    if cli_is_write(cli, &words) && !write {
        return Err(Failed::refused(format!(
            "`{cli} {head3}` changes something on {plat}; only when the user asked for this specific change, re-run with --write"
        )));
    }

    let mut env = ctx.env.clone();
    for k in ["GH_DEBUG", "GLAB_DEBUG", "DEBUG"] {
        env.remove(k);
    }
    let entry = match entry_arg {
        Some(e) => check_entry(ctx, e)?,
        None => default_entry(ctx, plat),
    };
    let mut source = "the CLI's own login".to_string();
    if pass_has(ctx, &entry) {
        let (secret, _notes) = read_entry(ctx, &entry)?;
        for var in p.env {
            env.set(var, secret.expose());
        }
        let repo = detect_repo(ctx);
        let host = repo.str("host");
        if plat == "github"
            && !host.is_empty()
            && host != "github.com"
            && !host.ends_with(".ghe.com")
        {
            env.set("GH_ENTERPRISE_TOKEN", secret.expose());
        }
        if plat == "gitlab"
            && !host.is_empty()
            && host != "gitlab.com"
            && !env.is_set("GITLAB_HOST")
        {
            env.set("GITLAB_HOST", host);
        }
        source = format!("pass ({entry})");
    } else if entry_arg.is_some() {
        return Err(Failed::err(format!(
            "{entry} isn't in pass; add it in your own terminal: pass insert -m {entry}"
        )));
    }
    note(&format!("{cli} {head3} \u{b7} token from {source}"));
    let mut argv = vec![cli.to_string()];
    argv.extend(words);
    Ok(Prepared { argv, env })
}

fn command(ctx: &Ctx, argv: &[String]) -> Command {
    let program = exec::which(&argv[0], &ctx.env).unwrap_or_else(|| argv[0].clone().into());
    let mut c = Command::new(program);
    c.args(&argv[1..]);
    c
}

/// Run the CLI and return its redacted stdout (trimmed).
///
/// # Errors
/// When preparing fails or the CLI exits non-zero.
pub fn run_cli_capture(
    ctx: &Ctx,
    plat: &str,
    words: &[String],
    entry_arg: Option<&str>,
    write: bool,
) -> R<String> {
    let prep = prepare_cli(ctx, plat, words, entry_arg, write)?;
    let o = exec::run(
        command(ctx, &prep.argv),
        &prep.env,
        None,
        Duration::from_secs(300),
    )
    .map_err(|e| Failed::err(format!("cannot run {}: {}", prep.argv[0], e.kind())))?;
    let stdout = String::from_utf8_lossy(&o.stdout).to_string();
    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
    if o.code != 0 {
        let text = redact(&format!("{stdout}{stderr}"));
        let t = text.trim();
        let n = t.chars().count();
        let tail: String = t.chars().skip(n.saturating_sub(2000)).collect();
        return Err(Failed::err(format!(
            "{} exited {}: {tail}",
            prep.argv[0], o.code
        )));
    }
    Ok(redact(&stdout).trim().to_string())
}

/// Run the CLI streaming its merged, redacted output to stdout; returns its exit status.
///
/// # Errors
/// When preparing fails or the program cannot start.
pub fn run_cli_stream(
    ctx: &Ctx,
    plat: &str,
    words: &[String],
    entry_arg: Option<&str>,
    write: bool,
) -> R<i32> {
    let prep = prepare_cli(ctx, plat, words, entry_arg, write)?;
    let mut cmd = command(ctx, &prep.argv);
    cmd.env_clear();
    for (k, v) in prep.env.iter() {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| Failed::err(format!("cannot run {}: {}", prep.argv[0], e.kind())))?;
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let mut handles = Vec::new();
    for stream in [
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        handles.push(std::thread::spawn(move || {
            let mut r = std::io::BufReader::new(stream);
            let mut buf = Vec::new();
            while r.read_until(b'\n', &mut buf).is_ok_and(|n| n > 0) {
                let _ = tx.send(std::mem::take(&mut buf));
            }
        }));
    }
    drop(tx);
    let mut so = std::io::stdout().lock();
    for chunk in rx {
        let _ = so.write_all(redact(&String::from_utf8_lossy(&chunk)).as_bytes());
        let _ = so.flush();
    }
    for h in handles {
        let _ = h.join();
    }
    Ok(child.wait().map_or(1, |s| s.code().unwrap_or(1)))
}

#[cfg(test)]
mod tests {
    use super::cli_is_write;

    fn w(cli: &str, s: &str) -> bool {
        cli_is_write(
            cli,
            &s.split_whitespace().map(String::from).collect::<Vec<_>>(),
        )
    }

    #[test]
    fn classification() {
        for (cli, words) in [
            ("gh", "pr list"),
            ("gh", "pr view 3 --comments"),
            ("gh", "pr checks 3"),
            ("gh", "issue list --search bug"),
            ("gh", "api repos/o/r/pulls"),
            ("gh", "run watch 5"),
            ("az", "boards work-item show --id 3"),
            ("az", "boards query --wiql x"),
            ("az", "repos pr list"),
            ("az", "devops invoke --area wit"),
            ("glab", "mr view 2"),
            ("glab", "issue list"),
            ("jira", "issue list"),
            ("jira", "issue view X-1"),
        ] {
            assert!(!w(cli, words), "{cli} {words} is a read");
        }
        for (cli, words) in [
            ("gh", "pr create --title list"),
            ("gh", "issue comment 5 --body x"),
            ("gh", "pr merge 3 --squash"),
            ("gh", "api repos/o/r/issues -f title=x"),
            ("gh", "api -X PATCH repos/o/r/issues/1"),
            ("gh", "-R o/r pr list"),
            ("gh", "run rerun 5"),
            ("az", "boards work-item update --id 3 --state Done"),
            ("az", "devops invoke --area wit --http-method PATCH"),
            ("glab", "mr merge 2"),
            ("jira", "issue move X-1 Done"),
            ("gh", "label create status"),
            ("gh", "issue edit 12 view"),
            ("gh", "pr comment 3 list"),
            ("glab", "issue note 4 show"),
            ("jira", "issue comment add KEY-1 list"),
        ] {
            assert!(w(cli, words), "{cli} {words} is a write");
        }
    }
}
