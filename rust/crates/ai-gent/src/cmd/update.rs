//! `ai-gent update` (spec 034): the notify-only check, and the update through the release's installer.
use ai_gent_core::update::apply::{self, Failure};
use ai_gent_core::update::{cache, check, source::ReleaseSource};
use ai_gent_core::{env::Env, exit::Exit, install::InstallRoot, output};
use clap::{ArgGroup, Args};
use std::io::{BufRead, IsTerminal, Write};

/// Arguments of `update`.
#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)] // command line flags
#[command(group(ArgGroup::new("mode").args(["check", "rollback", "list", "refresh"])))]
pub struct UpdateArgs {
    /// Print the update notice if a newer release is cached; never blocks, never installs.
    #[arg(long)]
    pub check: bool,
    /// Install X.Y.Z (pins it), or `latest` (clears the pin).
    #[arg(long, conflicts_with_all = ["check", "rollback", "list", "refresh"])]
    pub to: Option<String>,
    /// Do not ask for confirmation.
    #[arg(long)]
    pub yes: bool,
    /// Also run `gh attestation verify` on the download.
    #[arg(long)]
    pub verify_attestation: bool,
    /// Go back to the previous version.
    #[arg(long)]
    pub rollback: bool,
    /// List the installed versions.
    #[arg(long)]
    pub list: bool,
    /// The detached refresh worker (started by --check).
    #[arg(long, hide = true)]
    pub refresh: bool,
}

/// Run `update`.
pub fn run(a: &UpdateArgs, json: bool) -> Exit {
    let env = Env::real();
    let exe = std::env::current_exe().unwrap_or_default();
    let root = InstallRoot::discover(&exe, &env);
    if a.refresh {
        if let Some(dir) = env.state_dir() {
            let _ = check::refresh(&env, &cache::path_in(&dir));
        }
        return Exit::Success; // silent, whatever happened
    }
    if a.check {
        return run_check(&root, &env, &exe, json);
    }
    if a.list {
        return run_list(&root, &env, json);
    }
    let result = if a.rollback {
        run_rollback(&root, &env, a.yes, json)
    } else {
        run_update(&root, &env, a, json)
    };
    match result {
        Ok(data) => {
            if json {
                println!("{}", output::ok_json(1, data));
            }
            Exit::Success
        }
        Err(f) => fail(&f, json),
    }
}

fn fail(f: &Failure, json: bool) -> Exit {
    eprintln!("ai-gent: {}", f.message);
    if let Some(h) = &f.hint {
        eprintln!("  {h}");
    }
    if json {
        println!(
            "{}",
            output::error_json(f.code, &f.message, f.hint.as_deref())
        );
    }
    f.exit
}

fn run_check(root: &InstallRoot, env: &Env, exe: &std::path::Path, json: bool) -> Exit {
    let Some(dir) = env.state_dir() else {
        // No state directory: nothing can be cached, so nothing to say.
        let d = check::decide(root, env, std::path::Path::new(""), 0);
        return emit_check(&d.report, json);
    };
    let mut d = check::decide(root, env, &cache::path_in(&dir), check::now_secs());
    if d.refresh {
        d.report.checking = check::spawn_refresh(exe);
    }
    emit_check(&d.report, json)
}

fn emit_check(report: &check::CheckReport, json: bool) -> Exit {
    if json {
        println!("{}", output::ok_json(1, report));
    } else if let Some(n) = &report.notice {
        println!("{n}");
    }
    Exit::Success
}

fn run_list(root: &InstallRoot, env: &Env, json: bool) -> Exit {
    if json {
        println!("{}", output::ok_json(1, apply::list(root)));
        return Exit::Success;
    }
    match apply::list_with_installer(root, env) {
        Ok(true) => Exit::Success,
        Ok(false) => {
            let l = apply::list(root);
            for v in &l.versions {
                let mark = if v.current {
                    " (current)"
                } else if v.previous {
                    " (previous)"
                } else {
                    ""
                };
                println!("{}{mark}", v.version);
            }
            if let Some(p) = l.pinned {
                println!("pinned: {p}");
            }
            Exit::Success
        }
        Err(f) => fail(&f, false),
    }
}

/// Ask unless `--yes`. Without a terminal and without `--yes` the answer is exit 2.
fn confirm(yes: bool, json: bool, plan: &str) -> Result<(), Failure> {
    if !json {
        println!("{plan}");
    }
    if yes {
        return Ok(());
    }
    if json || !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err(Failure::usage(
            "confirmation needed: run it from a terminal, or pass --yes",
        ));
    }
    print!("Proceed? [y/N] ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
    if matches!(line.trim(), "y" | "Y" | "yes") {
        Ok(())
    } else {
        Err(Failure::nothing_to_do("cancelled"))
    }
}

fn run_update(
    root: &InstallRoot,
    env: &Env,
    a: &UpdateArgs,
    json: bool,
) -> Result<apply::Applied, Failure> {
    let src = ReleaseSource::from_env(env);
    let plan = apply::plan(root, env, &src, a.to.as_deref())?;
    let text = format!(
        "update {} -> {}{}\n  into    {}\n  from    {}\n  assets  {}, SHA256SUMS (the installer then fetches its archive)",
        plan.from,
        plan.to,
        if plan.pin_after {
            " (pins this version)"
        } else {
            ""
        },
        plan.home.display(),
        plan.installer_url
            .rsplit_once('/')
            .map_or("", |(base, _)| base),
        plan.installer_name,
    );
    confirm(a.yes, json, &text)?;
    apply::apply(&plan, &src, a.verify_attestation, json)
}

fn run_rollback(
    root: &InstallRoot,
    env: &Env,
    yes: bool,
    json: bool,
) -> Result<apply::Applied, Failure> {
    let (cur, prev, home) = apply::rollback_plan(root)?;
    confirm(
        yes,
        json,
        &format!("roll back {cur} -> {prev}\n  in {}", home.display()),
    )?;
    apply::rollback(root, env, json)
}
