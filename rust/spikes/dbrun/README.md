# dbrun spike (spec 038)

Throwaway experiments for the Rust database runner decision (`specs/038-db-runner-spike/`). A standalone
Cargo project (own `[workspace]`, own `Cargo.lock`), `publish = false`: it is not a member of the `rust/`
workspace, is not built by CI and is never shipped in the release package.

- `src/` - `dbcore` (statement classification, masking) and the `dbq` binary, one Cargo feature per engine.
- `tests/golden.rs`, `tests/parsercmp.rs` - the golden cases and the parser comparison.
- `golden.json` - cases blessed from `plugins/db/scripts/dbrun.py`; regenerate with `python3 gen_golden.py`.
- `mkdb.py` - makes a small sqlite database for the sqlite experiment.

Build one engine: `cargo build --release --features sqlite`. Run the cases: `cargo test`.
