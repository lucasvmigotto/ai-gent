//! `ai-gent wt`: the port of `plugins/agents/scripts/wt.py` (spec 036).
//!
//! Same subcommands, flags, output and exit codes as the Python tool, which stays in place until the
//! 3.0.0 cutover (decision D7). A failure prints `error: <message>` on stderr and exits 1.
use ai_gent_core::exit::Exit;
use ai_gent_core::wt::{Out, Res, commands};
use clap::{Args, Subcommand};

/// Arguments of `wt`.
#[derive(Debug, Args)]
pub struct WtArgs {
    /// What to run.
    #[command(subcommand)]
    action: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Features ready to start, from specs/README.md.
    Plan,
    /// A worktree for a new branch off the chain's dev, after a disk check of the container store.
    Add {
        /// The new branch.
        branch: String,
        /// Branch to start from (default: the chain's dev).
        #[arg(long)]
        base: Option<String>,
        /// The disk floor for this call, in GB (default max(20, 10%)).
        #[arg(long)]
        min_free_gb: Option<f64>,
    },
    /// The worktrees under .worktrees/.
    List,
    /// Remove a merged, clean worktree, its branch, containers and volumes.
    Remove {
        /// The branch of the worktree.
        branch: String,
        /// Leave the track's containers and volumes.
        #[arg(long)]
        keep_containers: bool,
    },
    /// Record the store's external (buildah) containers.
    Baseline,
    /// This project's container leftovers and the disk, report only.
    Resources,
    /// End of a wave: dangling images, new buildah containers.
    Finish,
}

/// Run the subcommand.
pub fn run(a: &WtArgs, json: bool) -> Exit {
    let mut out = Out::default();
    let res: Res<i32> = match &a.action {
        Action::Plan => commands::plan(json, &mut out).map(|()| 0),
        Action::Add {
            branch,
            base,
            min_free_gb,
        } => commands::add(branch, base.as_deref(), *min_free_gb, &mut out).map(|()| 0),
        Action::List => commands::list(json, &mut out).map(|()| 0),
        Action::Remove {
            branch,
            keep_containers,
        } => commands::remove(branch, *keep_containers, &mut out).map(|()| 0),
        Action::Baseline => commands::baseline(&mut out).map(|()| 0),
        Action::Resources => commands::resources(json, &mut out).map(|()| 0),
        Action::Finish => commands::finish(&mut out),
    };
    print!("{}", out.stdout);
    eprint!("{}", out.stderr);
    match res {
        Ok(0) => Exit::Success,
        Ok(_) => Exit::Runtime,
        Err(e) => {
            eprintln!("error: {e}");
            Exit::Runtime
        }
    }
}
