# dbrun spike (spec 038)

Throwaway experiments for the Rust database runner decision (`specs/038-db-runner-spike/`). A standalone
Cargo project (own `[workspace]`, own `Cargo.lock`), `publish = false`: it is not a member of the `rust/`
workspace, is not built by CI and is never shipped in the release package.

- `src/core/` - driver-free `dbcore` (statement classification, masking); `src/main.rs` is the `dbq` binary, with one Cargo feature per engine.
- `tests/golden.rs`, `tests/parsercmp.rs` - the golden cases and the parser comparison.
- `../../fixtures/db-core/golden.json` - cases blessed from `plugins/db/scripts/dbrun.py`; regenerate with `python3 gen_golden.py`.
- `mkdb.py` - makes a small sqlite database for the sqlite experiment.

Build one engine: `cargo build --release --features sqlite`. Run the cases: `cargo test`.

T013 H-exec baseline: `cargo build --offline --bin adapter` builds an execution-only
process connector with no native driver features. It accepts the Python `_exec`
request on stdin and returns its one-line JSON result unchanged. It launches
`python3 plugins/db/scripts/dbrun.py _exec` using the source checkout's absolute
path, so it works from a temporary project directory too. The interpreter must
already have any non-SQLite driver installed; this bridge does not provision it.
Run from the repository root with `python3 scripts/test-dbrun-engines.py --runner
<absolute-target-dir>/debug/adapter --engine sqlite`. This runs the eight original
SQLite journeys, including the direct classifier-bypass write refusal, through
the process connector. Omitting `--runner` keeps Python execution in-process;
omitting `--engine` keeps the original five container engines. The harness owns
policy, profiles, plans, backups and restore; the adapter owns execution only.

E1 fixtures include the original 16 Python tests, with `derived_from` and
recorded calls/results, plus the 171-string corpus across six engines (1026
classifications with exact reasons, splits and DDL checks; 4104 cost checks
with exact reasons). The generator runs the unmodified Python tests and fails
if they fail. Run it from any directory with `python3 rust/spikes/dbrun/gen_golden.py`.

Two regex lookaround gaps are implemented by hand, not silently omitted:
Oracle's `DBMS_(?!METADATA\b|XPLAN\b)` exclusion uses a token comparison
(`DBMS_METADATA` and `DBMS_XPLAN` are exempt; suffixed names are not), and
redaction's `(?<![\w.-])value(?![\w-])` uses adjacent-character checks.
These fixtures establish parity for the tested inputs, not a proof for every
Unicode or SQL input. This remains an unshipped spike.

E4: `TMPDIR=$HOME/tmp CARGO_TARGET_DIR=$HOME/tmp/codex-spike-target cargo test --offline --features sqlite sqlite`
inside this directory runs the eight original `SqliteEndToEndTest` journeys
through `test_sqlite.py` and `sqlite-exec`, replacing Python's `execute` only.
They cover masking/logging, row cap, remote refusals, a bypassed-classifier
write, plan/apply/restore, count-mismatch rollback, tamper refusal and scripts
that never execute. Policy, plan hashing, backups and restore remain Python;
this is native connector evidence, not an independent Rust runner or T013.
A separate native test checks SQLite's ReadOnly error for main and TEMP writes:
`query_only` blocks TEMP, and `mode=ro` still blocks main writes after disabling
`query_only`. The executor also sets a progress-handler deadline and lock timeout.
Bundled SQLite is compiled locally; no database container or system SQLite is used.

Validation: the standalone core tests (4 golden tests over the 16 Python source tests, exact reasons),
the eight native-execution SQLite journeys and the independent read-only test pass with
`cargo test --features sqlite`; `python3 -m unittest plugins/db/scripts/test_dbrun.py` passes 51 runs.
Golden regeneration from another working directory is byte-for-byte identical.
