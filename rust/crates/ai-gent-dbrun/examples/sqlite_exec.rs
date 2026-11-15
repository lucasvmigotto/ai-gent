//! The `_exec` child over the SQLite connector, as a stand-alone binary: one JSON request on stdin, one
//! JSON result line on stdout (spec 043 T021). It lets `scripts/test-dbrun-engines.py --runner <this>
//! --engine sqlite` run the eight `SqliteEndToEndTest` journeys through the Rust connector before the
//! `ai-gent db _exec` subcommand exists (T011). An example: built by `cargo test` or
//! `cargo build --example`, never part of a release.
use ai_gent_dbrun::connector::isolate::serve_exec;
use ai_gent_dbrun::connector::sqlite::SqliteConnector;

fn main() {
    let code = serve_exec(
        &SqliteConnector,
        std::io::stdin().lock(),
        std::io::stdout().lock(),
    );
    std::process::exit(code);
}
