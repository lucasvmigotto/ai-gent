//! Command line definition.
use clap::{Args, Parser, Subcommand};

/// Skills and plugins for AI coding agents.
#[derive(Debug, Parser)]
#[command(name = "ai-gent", disable_version_flag = true)]
pub struct Cli {
    /// Print one JSON object on stdout instead of text.
    #[arg(long, global = true)]
    pub json: bool,
    /// Print `ai-gent <version> (<target>)`.
    #[arg(long)]
    pub version: bool,
    /// What to run.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print the version, release, target and commit.
    Version,
    /// Check that the binary and its install are healthy.
    Doctor(crate::cmd::doctor::DoctorArgs),
    /// Check for, install, list or roll back releases (notify-only: never self-replaces).
    Update(crate::cmd::update::UpdateArgs),
    /// Evaluate a guard on a hook event read from stdin (shadow phase: the shell guards decide).
    Guard(GuardArgs),
    /// Token usage per card, estimates and the budget (a port of `token_report.py`).
    Tokens(crate::cmd::tokens::TokensArgs),
    /// Worktrees for parallel tracks and their container leftovers (a port of `wt.py`).
    Wt(crate::cmd::wt::WtArgs),
}

/// Arguments of `guard`.
#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct GuardArgs {
    /// What to run instead of evaluating.
    #[command(subcommand)]
    pub action: Option<GuardAction>,
    /// Which guard: git, db, secrets, repo, containers or all.
    #[arg(long, default_value = "all")]
    pub guard: String,
    /// Shell syntax: sh, powershell or cmd (default from the tool name, else sh).
    #[arg(long)]
    pub shell: Option<String>,
    /// Input and output format: claude or opencode.
    #[arg(long, default_value = "claude")]
    pub format: String,
    /// Compare with the shell guard's decision, append a shadow record, print nothing.
    #[arg(long, requires = "shell_decision")]
    pub shadow: bool,
    /// With --shadow: what the shell guard decided (deny, ask or none).
    #[arg(long, value_parser = ["deny", "ask", "none"])]
    pub shell_decision: Option<String>,
    /// With --shadow: the shell guard's reason, to compare.
    #[arg(long)]
    pub shell_reason: Option<String>,
}

/// Guard subcommands.
#[derive(Debug, Subcommand)]
pub enum GuardAction {
    /// Print every reason entry.
    ListReasons,
    /// Summarize the shadow log; exit 1 when a disagreement is unexplained.
    ShadowReport {
        /// Only records newer than this (`90s`, `15m`, `24h`, `7d`).
        #[arg(long)]
        since: Option<String>,
        /// Only this guard.
        #[arg(long)]
        guard: Option<String>,
    },
}
