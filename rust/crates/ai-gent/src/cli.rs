//! Command line definition.
use clap::{Parser, Subcommand};

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
}
