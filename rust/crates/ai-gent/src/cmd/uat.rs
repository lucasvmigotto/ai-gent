//! `ai-gent uat`: lint, render, diff, preflight, digest and report for a live acceptance test
//! (`qa:uat`, spec 055). Reads files; writes only the file `--out` names.
use ai_gent_core::exit::Exit;
use ai_gent_core::output::{self, ErrorCode};
use ai_gent_core::uat::digest::{self, FollowUp, Snapshot};
use ai_gent_core::uat::preflight::{self, Declared, Read};
use ai_gent_core::uat::report::{self, Records};
use ai_gent_core::uat::{self, diff, lint, model::Roadmap, render};
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
    /// Render the page from the roadmap; refuses when lint has errors.
    Render {
        /// `roadmap.json`.
        roadmap: PathBuf,
        /// Write the page here (default: stdout, not with `--json`).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Where `specs/` lives, for mode B.
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Compare two versions of a roadmap; exit 1 when the new one must be refused.
    Diff {
        /// The published version.
        old: PathBuf,
        /// The edited version.
        new: PathBuf,
    },
    /// Access before and after publishing.
    Preflight {
        /// What to do.
        #[command(subcommand)]
        action: PreflightAction,
    },
    /// The status report from register snapshots.
    Digest {
        /// `roadmap.json`.
        roadmap: PathBuf,
        /// Snapshot pages of the register (merged).
        #[arg(required = true)]
        snapshots: Vec<PathBuf>,
        /// A snapshot page of the previous check (repeat for several pages).
        #[arg(long = "since")]
        since: Vec<PathBuf>,
        /// `follow-up.json`, to state the next check or the expired timer.
        #[arg(long)]
        follow_up: Option<PathBuf>,
    },
    /// The consolidated report (`report.md`).
    Report {
        /// `roadmap.json`.
        roadmap: PathBuf,
        /// Snapshot pages of the register (merged).
        #[arg(required = true)]
        snapshots: Vec<PathBuf>,
        /// `triage.md`.
        #[arg(long)]
        triage: Option<PathBuf>,
        /// `decisions.md`.
        #[arg(long)]
        decisions: Option<PathBuf>,
        /// `preflight.md`.
        #[arg(long)]
        preflight: Option<PathBuf>,
        /// Write the report here (default: stdout, not with `--json`).
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

/// Preflight subcommands.
#[derive(Debug, Subcommand)]
pub enum PreflightAction {
    /// The checklist the owner confirms line by line; exit 1 when a line needs action.
    Checklist {
        /// `roadmap.json`.
        roadmap: PathBuf,
    },
    /// Compare the published declaration and the test reads with the contract; exit 1 on a mismatch.
    Verify {
        /// `roadmap.json`.
        roadmap: PathBuf,
        /// The `capabilities` that were published, as JSON.
        #[arg(long)]
        declared: PathBuf,
        /// A test read: `<level>=<snapshot.json>` (repeat for `view`, `interact` and `admin`).
        #[arg(long = "read")]
        reads: Vec<String>,
    },
}

fn read_file(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn load(path: &Path) -> Result<Roadmap, String> {
    Roadmap::parse(&read_file(path)?).map_err(|e| format!("{}: {e}", path.display()))
}

fn snapshot(paths: &[PathBuf]) -> Result<Snapshot, String> {
    let mut pages = Vec::new();
    for p in paths {
        pages.push(Snapshot::parse(&read_file(p)?).map_err(|e| format!("{}: {e}", p.display()))?);
    }
    Ok(Snapshot::merge(pages))
}

fn fail(json: bool, msg: &str) -> Exit {
    eprintln!("error: {msg}");
    if json {
        println!("{}", output::error_json(ErrorCode::Runtime, msg, None));
    }
    Exit::Runtime
}

fn exit_if(bad: bool) -> Exit {
    if bad { Exit::Runtime } else { Exit::Success }
}

/// Print a text artifact to stdout, or write it to `out`.
fn deliver(text: &str, out: Option<&Path>, json: bool) -> Exit {
    match out {
        Some(p) => match std::fs::write(p, text) {
            Ok(()) => {
                if json {
                    println!(
                        "{}",
                        output::ok_json(
                            1,
                            json!({"out": p.display().to_string(), "bytes": text.len()})
                        )
                    );
                } else {
                    println!("wrote {} ({} bytes)", p.display(), text.len());
                }
                Exit::Success
            }
            Err(e) => fail(json, &format!("{}: {e}", p.display())),
        },
        None if json => fail(json, "--json needs --out for this action"),
        None => {
            print!("{text}");
            Exit::Success
        }
    }
}

/// Run `ai-gent uat`.
pub fn run(args: &UatArgs, json: bool) -> Exit {
    match run_inner(&args.action, json) {
        Ok(e) | Err(e) => e,
    }
}

#[allow(clippy::too_many_lines, clippy::many_single_char_names)]
fn run_inner(action: &UatAction, json: bool) -> Result<Exit, Exit> {
    let bad = |e: String| fail(json, &e);
    match action {
        UatAction::Lint {
            roadmap,
            root,
            budget,
        } => {
            let r = load(roadmap).map_err(bad)?;
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
            Ok(exit_if(errors))
        }
        UatAction::Render { roadmap, out, root } => {
            let r = load(roadmap).map_err(bad)?;
            match render::render(&r, &uat::context_for(&r, root, None)) {
                Ok(page) => Ok(deliver(&page, out.as_deref(), json)),
                Err(errors) => {
                    if json {
                        println!(
                            "{}",
                            output::error_json(
                                ErrorCode::Refused,
                                "the roadmap has lint errors",
                                None
                            )
                        );
                    }
                    eprint!("{}", lint::text(&errors));
                    Ok(Exit::Runtime)
                }
            }
        }
        UatAction::Diff { old, new } => {
            let a = load(old).map_err(bad)?;
            let b = load(new).map_err(bad)?;
            let d = diff::diff(&a, &b);
            if json {
                println!("{}", output::ok_json(1, &d));
            } else {
                print!("{}", diff::text(&d));
            }
            Ok(exit_if(!d.refused.is_empty()))
        }
        UatAction::Preflight { action } => preflight_run(action, json),
        UatAction::Digest {
            roadmap,
            snapshots,
            since,
            follow_up,
        } => {
            let r = load(roadmap).map_err(bad)?;
            let now = snapshot(snapshots).map_err(bad)?;
            let before = if since.is_empty() {
                None
            } else {
                Some(snapshot(since).map_err(bad)?)
            };
            let fu: Option<FollowUp> = match follow_up {
                Some(p) => Some(
                    serde_json::from_str(&read_file(p).map_err(bad)?)
                        .map_err(|e| fail(json, &format!("{}: {e}", p.display())))?,
                ),
                None => None,
            };
            let s = digest::status(&r, &now, before.as_ref(), fu.as_ref());
            if json {
                println!("{}", output::ok_json(1, &s));
            } else {
                print!("{}", digest::text(&s));
            }
            Ok(Exit::Success)
        }
        UatAction::Report {
            roadmap,
            snapshots,
            triage,
            decisions,
            preflight,
            out,
        } => {
            let r = load(roadmap).map_err(bad)?;
            let s = snapshot(snapshots).map_err(bad)?;
            let opt = |p: &Option<PathBuf>| -> Result<Option<String>, Exit> {
                p.as_deref().map(read_file).transpose().map_err(bad)
            };
            let (t, d, p) = (opt(triage)?, opt(decisions)?, opt(preflight)?);
            let text = report::report(
                &r,
                &s,
                &Records {
                    triage: t.as_deref(),
                    decisions: d.as_deref(),
                    preflight: p.as_deref(),
                },
            );
            Ok(deliver(&text, out.as_deref(), json))
        }
    }
}

fn preflight_run(action: &PreflightAction, json: bool) -> Result<Exit, Exit> {
    let bad = |e: String| fail(json, &e);
    match action {
        PreflightAction::Checklist { roadmap } => {
            let r = load(roadmap).map_err(bad)?;
            let c = preflight::checklist(&r);
            if json {
                println!("{}", output::ok_json(1, &c));
            } else {
                print!("{}", preflight::checklist_text(&c));
            }
            Ok(exit_if(c.action_needed))
        }
        PreflightAction::Verify {
            roadmap,
            declared,
            reads,
        } => {
            let r = load(roadmap).map_err(bad)?;
            let d: Declared = serde_json::from_str(&read_file(declared).map_err(bad)?)
                .map_err(|e| fail(json, &format!("{}: {e}", declared.display())))?;
            let mut parsed = Vec::new();
            for spec in reads {
                let (level, file) = spec.split_once('=').ok_or_else(|| {
                    fail(json, &format!("--read wants <level>=<file>, got `{spec}`"))
                })?;
                let mut read: Read =
                    serde_json::from_str(&read_file(Path::new(file)).map_err(bad)?)
                        .map_err(|e| fail(json, &format!("{file}: {e}")))?;
                read.level = level.to_string();
                parsed.push(read);
            }
            let v = preflight::verify(&r, &d, &parsed);
            if json {
                println!("{}", output::ok_json(1, &v));
            } else {
                print!("{}", preflight::verify_text(&v));
            }
            Ok(exit_if(!v.ok))
        }
    }
}
