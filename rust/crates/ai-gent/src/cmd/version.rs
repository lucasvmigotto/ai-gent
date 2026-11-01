//! `ai-gent version`.
use ai_gent_core::{exit::Exit, output, version::VersionInfo};

/// Run `version`.
pub fn run(json: bool) -> Exit {
    let info = VersionInfo::current(env!("CARGO_PKG_VERSION"));
    if json {
        println!("{}", output::ok_json(1, &info));
    } else {
        println!("{}", info.line());
    }
    Exit::Success
}
