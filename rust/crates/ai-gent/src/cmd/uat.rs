//! `ai-gent uat`: lint a roadmap and diff two versions (`qa:uat`, spec 055). Read-only.
use ai_gent_core::exit::Exit;
use ai_gent_core::output::{self, ErrorCode};
use ai_gent_core::uat::{self, diff, lint, model::Roadmap};
use clap::{Args, Subcommand};
use serde_json::json;
use std::path::{Path, PathBuf};

/// Arguments of `uat`.
#[derive(Debug, Args)]
pub struct UatArgs {
    /// What to do.
    #[command(subcommand)]
    pub action: UatAction,
}

/// Uat subcommands.
#[derive(Debug, Subcommand)]
pub enum UatAction {
    /// Check a roadmap source; exit 1 when it has errors (warnings alone exit 0).
    Lint {
        /// `roadmap.json`.
        roadmap: PathBuf,
        /// Where `specs/` lives, for mode B (default: the current directory).
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// The per-document budget in bytes (default: the roadmap's own setting).
        #[arg(long)]
        budget: Option<u64>,
    },
    /// Compare two versions of a roadmap; exit 1 when the new one must be refused.
    Diff {
        /// The published version.
        old: PathBuf,
        /// The edited version.
        new: PathBuf,
    },
}

fn load(path: &Path) -> Result<Roadmap, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Roadmap::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn fail(json: bool, msg: &str) -> Exit {
    eprintln!("error: {msg}");
    if json {
        println!("{}", output::error_json(ErrorCode::Runtime, msg, None));
    }
    Exit::Runtime
}

/// Run `ai-gent uat`.
pub fn run(args: &UatArgs, json: bool) -> Exit {
    match &args.action {
        UatAction::Lint {
            roadmap,
            root,
            budget,
        } => {
            let r = match load(roadmap) {
                Ok(r) => r,
                Err(e) => return fail(json, &e),
            };
            let findings = lint::lint(&r, &uat::context_for(&r, root, *budget));
            let errors = lint::has_errors(&findings);
            if json {
                println!(
                    "{}",
                    output::ok_json(
                        1,
                        json!({"clean": findings.is_empty(), "errors": errors, "findings": findings})
                    )
                );
            } else {
                print!("{}", lint::text(&findings));
            }
            if errors { Exit::Runtime } else { Exit::Success }
        }
        UatAction::Diff { old, new } => {
            let (a, b) = match (load(old), load(new)) {
                (Ok(a), Ok(b)) => (a, b),
                (Err(e), _) | (_, Err(e)) => return fail(json, &e),
            };
            let d = diff::diff(&a, &b);
            if json {
                println!("{}", output::ok_json(1, &d));
            } else {
                print!("{}", diff::text(&d));
            }
            if d.refused.is_empty() {
                Exit::Success
            } else {
                Exit::Runtime
            }
        }
    }
}
