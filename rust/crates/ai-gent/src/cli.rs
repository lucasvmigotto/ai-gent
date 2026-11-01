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
    Doctor,
    /// Evaluate a guard on a hook event read from stdin (shadow phase: the shell guards decide).
    Guard(GuardArgs),
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
}

/// Guard subcommands.
#[derive(Debug, Subcommand)]
pub enum GuardAction {
    /// Print every reason entry.
    ListReasons,
}
