# compact spike (spec 037, T060-T062)

Throwaway experiment for porting `ai-gent compact` without Pygments (`specs/037-repo-spec-pass-compact-tools/`). A standalone Cargo project (own `[workspace]`, own `Cargo.lock`), `publish = false`: not a member of the `rust/` workspace, not built by CI and never shipped in the release package.

- `src/{tokenize,render,verify}.rs` - tree-sitter leaves as tokens, the level 1-4 renderer (the `free` and a simple `js` join rule), the verifier. The D29 scope: rust, java, c, cpp, csharp, go, javascript (JSX), typescript, tsx, kotlin, css and scss. `src/lookaround.rs` holds the hand-ported look-around patterns; Go and Kotlin are verified lexically (their grammars reject valid one-line code).
- `src/sizeprobe.rs` - links one grammar chosen by a cargo feature `g-<crate>` and parses an empty file; the per-grammar binary sizes in `grammar-table.md` come from it.
- `tests/fixtures.rs` - the 13 in-scope fixtures at levels 1-4, within 0.5% of the `compact.py` sizes (`cargo test --release`).
- `examples/sizes.rs` - `cargo run --release --example sizes DIR` prints bytes and the level reached per fixture.
- `examples/show.rs` - `cargo run --release --example show FILE LEVEL` prints the compacted file.

Results: `specs/037-repo-spec-pass-compact-tools/measurements.md`, section Compact spike.
