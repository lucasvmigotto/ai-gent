//! Thin opt-in registration protocol adapter; legacy command envelopes remain unchanged.
use ai_gent_core::{exit::Exit, usage};
use clap::{Args, Subcommand};
use serde_json::json;
use std::io::Read;
use std::path::PathBuf;

/// Registration command arguments.
#[derive(Debug, Args)]
pub struct UsageArgs {
    /// Project containing the declared specs and its private registration journal.
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,
    /// Registration wire protocol version (only 1 is supported).
    #[arg(long, global = true, default_value_t = 1)]
    protocol_version: u32,
    #[command(subcommand)]
    action: Action,
}
#[derive(Debug, Subcommand)]
enum Action {
    /// Register an existing task before executing it; this does not collect usage.
    Register(Input),
    /// Record normalized usage for a registered run; local evidence is not provider attestation.
    Ingest(Input),
    /// Start a new immutable provider/model/configuration segment in a registered run.
    Segment(Input),
    /// Record running/finished/failed/cancelled execution; never accepts the task.
    Transition(Input),
    /// Verify and read durable registered state without creating a journal.
    Replay,
}
#[derive(Debug, Args)]
struct Input {
    /// Bounded UTF-8 JSON request file, or - for stdin.
    #[arg(long, default_value = "-")]
    input: PathBuf,
}
fn read(input: &Input) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    if input.input.as_os_str() == "-" {
        std::io::stdin()
            .take(usage::MAX_RECORD as u64 + 1)
            .read_to_end(&mut bytes)?;
    } else {
        if !std::fs::symlink_metadata(&input.input)?.is_file() {
            return Err(std::io::Error::other("request must be a regular file"));
        }
        let file = std::fs::File::open(&input.input)?;
        if !file.metadata()?.is_file() {
            return Err(std::io::Error::other("request must be a regular file"));
        }
        file.take(usage::MAX_RECORD as u64 + 1)
            .read_to_end(&mut bytes)?;
    }
    Ok(bytes)
}
fn exit(code: u8) -> Exit {
    match code {
        0 => Exit::Success,
        2 => Exit::Usage,
        3 => Exit::NothingToDo,
        4 => Exit::Refused,
        _ => Exit::ProtocolIo,
    }
}
/// Run registration protocol v1 without changing other command output/exit contracts.
pub fn run(args: &UsageArgs, json_output: bool) -> Exit {
    let operation = match args.action {
        Action::Register(_) => "register",
        Action::Ingest(_) => "ingest",
        Action::Segment(_) => "segment",
        Action::Transition(_) => "transition",
        Action::Replay => "replay",
    };
    let (code, value) = if args.protocol_version == 1 {
        let result = match &args.action {
            Action::Replay => usage::replay(&args.root),
            Action::Ingest(input) => match read(input) {
                Ok(bytes) => usage::ingest(&args.root, &bytes),
                Err(_) => return failure(operation, json_output),
            },
            Action::Register(input) | Action::Segment(input) | Action::Transition(input) => {
                match read(input) {
                    Ok(bytes) => usage::execute(&args.root, operation, &bytes),
                    Err(_) => return failure(operation, json_output),
                }
            }
        };
        match result {
            Ok(value) => (0, value),
            Err(error) => (
                error.exit_code(),
                json!({"schema_version":1,"operation":operation,"ok":false,"error":error}),
            ),
        }
    } else {
        (
            2,
            json!({"schema_version":1,"operation":operation,"ok":false,"error":{"code":"unsupported_version","retryable":false,"message":"only registration protocol 1 is supported"}}),
        )
    };
    if json_output {
        println!("{value}");
    } else if code == 0 {
        if operation == "replay" {
            println!(
                "Registration journal: {} events",
                value["result"]["sequence"]
            );
        } else if operation == "ingest" {
            println!(
                "Usage observation {} recorded (local, unclosed coverage)",
                value["result"]["event_id"]
            );
        } else {
            println!(
                "Run {}: {} (usage missing)",
                value["result"]["run_id"], value["result"]["state"]
            );
        }
    } else {
        eprintln!("{}", value["error"]["message"]);
    }
    exit(code)
}
fn failure(operation: &str, json_output: bool) -> Exit {
    if json_output {
        println!(
            "{}",
            json!({"schema_version":1,"operation":operation,"ok":false,"error":{"code":"io_failure","retryable":false,"message":"cannot read registration request"}})
        );
    } else {
        eprintln!("cannot read registration request");
    }
    Exit::ProtocolIo
}
