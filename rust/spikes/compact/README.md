# compact spike (spec 037, T060-T062)

Throwaway experiment for porting `ai-gent compact` without Pygments (`specs/037-repo-spec-pass-compact-tools/`). A standalone Cargo project (own `[workspace]`, own `Cargo.lock`), `publish = false`: not a member of the `rust/` workspace, not built by CI and never shipped in the release package.

- `src/{tokenize,render,verify}.rs` - tree-sitter leaves as tokens, the level 1-4 renderer (the `free` and a simple `js` join rule), the verifier. Rust, Java, TypeScript and TSX only.
- `src/sizeprobe.rs` - links one grammar chosen by a cargo feature `g-<crate>` and parses an empty file; the per-grammar binary sizes in `grammar-table.md` come from it.
- `tests/fixtures.rs` - the four matching fixtures at levels 1-4 (`cargo test --release`).
- `examples/show.rs` - `cargo run --release --example show FILE LEVEL` prints the compacted file.

Results: `specs/037-repo-spec-pass-compact-tools/measurements.md`, section Compact spike.
