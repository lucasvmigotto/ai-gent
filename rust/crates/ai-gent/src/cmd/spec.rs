//! `ai-gent spec`: plan, record and features (a port of `specsync.py`, spec 037 Group B).
use ai_gent_core::exit::Exit;
use ai_gent_core::spec;
use ai_gent_core::tokens::pyjson::dumps;
use clap::{Args, Subcommand};
use std::path::Path;

/// Arguments of `spec`.
#[derive(Debug, Args)]
pub struct SpecArgs {
    /// What to do.
    #[command(subcommand)]
    pub action: SpecAction,
}

/// Spec subcommands.
#[derive(Debug, Subcommand)]
pub enum SpecAction {
    /// The items a feature's specs imply, and what changed since the last sync.
    Plan {
        /// `NNN` or `specs/NNN-feature`.
        feature: String,
    },
    /// Write a batch's results into the feature's `tracker.md`.
    Record {
        /// `NNN` or `specs/NNN-feature`.
        feature: String,
        /// The platform the items live on.
        #[arg(long)]
        platform: String,
        /// `owner/repo`, `org/project`, a Jira project key.
        #[arg(long = "where")]
        wher: Option<String>,
        /// A `repo batch --out` file.
        #[arg(long = "from")]
        from_file: String,
    },
    /// Every feature's state and pending changes.
    Features,
}

fn fail(msg: &str) -> Exit {
    eprintln!("error: {msg}");
    Exit::Runtime
}

/// Run `ai-gent spec`.
pub fn run(args: &SpecArgs, json: bool) -> Exit {
    match &args.action {
        SpecAction::Plan { feature } => match spec::feature_dir(feature) {
            Ok(dir) => {
                let plan = spec::plan_for(&dir);
                if json {
                    println!("{}", dumps(&plan.to_json(), false, Some(2)));
                } else {
                    print!("{}", spec::plan_text(&plan));
                }
                Exit::Success
            }
            Err(e) => fail(&e.0),
        },
        SpecAction::Record {
            feature,
            platform,
            wher,
            from_file,
        } => match spec::feature_dir(feature).and_then(|d| {
            spec::record(
                &d,
                platform,
                wher.as_deref().unwrap_or(""),
                Path::new(from_file),
            )
        }) {
            Ok(line) => {
                println!("{line}");
                Exit::Success
            }
            Err(e) => fail(&e.0),
        },
        SpecAction::Features => {
            let rows = spec::features();
            if json {
                println!("{}", spec::features_json(&rows));
            } else {
                for (f, synced, pending, state) in rows {
                    println!(
                        "{f:<40} {state:<7} {:<11} {pending} pending",
                        if synced { "synced" } else { "not synced" }
                    );
                }
            }
            Exit::Success
        }
    }
}
