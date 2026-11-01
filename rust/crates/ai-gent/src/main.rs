//! The `ai-gent` command line.
mod cli;
mod cmd;

use ai_gent_core::{
    exit::Exit,
    output::{self, ErrorCode},
    version::VersionInfo,
};
use clap::{Parser, error::ErrorKind};
use cli::{Cli, Command};
use std::process::ExitCode;

fn real_main() -> Exit {
    let args: Vec<String> = std::env::args().collect();
    let wants_json = args.iter().any(|a| a == "--json");
    let cli = match Cli::try_parse_from(&args) {
        Ok(c) => c,
        Err(e) => {
            let help = matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion);
            if help {
                print!("{e}");
                return Exit::Success;
            }
            if wants_json {
                let msg = e.to_string();
                println!(
                    "{}",
                    output::error_json(ErrorCode::Usage, msg.trim_end(), None)
                );
            }
            eprint!("{e}");
            return Exit::Usage;
        }
    };
    if cli.version {
        println!("{}", VersionInfo::current(env!("CARGO_PKG_VERSION")).line());
        return Exit::Success;
    }
    match cli.command {
        Some(Command::Version) => cmd::version::run(cli.json),
        Some(Command::Doctor) => cmd::doctor::run(cli.json),
        Some(Command::Guard(a)) => cmd::guard::run(&a, cli.json),
        None => {
            eprintln!("usage: ai-gent [--json] <version|doctor|guard>");
            if cli.json {
                println!(
                    "{}",
                    output::error_json(ErrorCode::Usage, "no subcommand given", None)
                );
            }
            Exit::Usage
        }
    }
}

fn main() -> ExitCode {
    let result = std::panic::catch_unwind(real_main);
    ExitCode::from(result.unwrap_or(Exit::Internal).code())
}
