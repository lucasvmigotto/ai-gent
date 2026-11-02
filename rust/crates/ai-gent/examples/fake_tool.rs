//! Same fake tool as the core crate's example (spec 037 T001), built here so `cargo test -p ai-gent` has it.
#[path = "../../ai-gent-core/examples/fake_tool.rs"]
mod fake;

fn main() {
    fake::main();
}
