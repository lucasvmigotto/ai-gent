# Backup test fixture hardening

The database backup integration tests now use a prebuilt, std-only Rust example instead of writing executable shell scripts immediately before spawning them. Production backup code and Cargo dependencies are unchanged. Modes use per-child environment values; synthetic password input remains on stdin.

Independent review and the focused suite passed all 13 backup tests. Existing argument, password, private-file, deadline, output-cap and cleanup assertions remain intact. Ordinary `cargo test` builds the example; for a filtered run first execute `cargo build -p ai-gent-dbrun --example backup_fake_engine` using the same profile and target directory.

This does not establish the cause of the historical intermittent spawn failure. Two uninstrumented full checks on the performance slice failed; subsequent isolated, workspace and diagnostic full-script runs passed without capturing a failing errno. Temporary production diagnostics were removed. The native hang no longer incidentally exercises pipes inherited by a shell grandchild; a separate inherited-pipe regression remains desirable. Production promises bounded return, not process-tree termination.

Full uninstrumented `scripts/check.sh --quiet` passed on 2026-11-25, including installer/update tests, pinned Rust format/lint/workspace tests and dependency checks. The local log was `/tmp/ai-gent-backup-native-fixture-full-check.log`; this committed result is the portable record. No production diagnostic instrumentation was present. No platform or full-feature qualification is implied by local tests.
