//! `ai-gent doctor` (spec 034): the checks live in `ai_gent_core::doctor`.
use ai_gent_core::doctor::{self, Ctx};
use ai_gent_core::{env::Env, exit::Exit, install::InstallRoot, output, version::VersionInfo};
use clap::Args;

/// Arguments of `doctor`.
#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Skip hashing every file against MANIFEST.
    #[arg(long)]
    pub fast: bool,
    /// Exit 1 on a warning too.
    #[arg(long)]
    pub strict: bool,
}

/// Run `doctor`.
pub fn run(a: &DoctorArgs, json: bool) -> Exit {
    let env = Env::real();
    let exe = std::env::current_exe().unwrap_or_default();
    let ctx = Ctx {
        root: InstallRoot::discover(&exe, &env),
        release: VersionInfo::current(env!("CARGO_PKG_VERSION")).release,
        exe,
        env,
        fast: a.fast,
    };
    let report = doctor::run(&ctx);
    let failed = report.failed(a.strict);
    if json {
        println!("{}", output::ok_json(1, &report));
    } else {
        print!("{}", report.text());
    }
    if failed { Exit::Runtime } else { Exit::Success }
}
