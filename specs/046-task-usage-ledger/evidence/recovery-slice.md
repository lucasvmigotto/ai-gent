# Recovery and snapshot implementation evidence — 2026-11-18

Status: bounded 046 US3 slice on `feat/usage-recovery`, initially stacked on ingestion commit `6d2fa8890e44ff694a083eb1155a52fd2150941d` while that PR was reviewed. Before publication it was aligned to merged develop `87d0861b4323656d24b0d8b05476ba45016f258e`; the reviewed implementation diff was byte-compared before/after alignment and unchanged. This is implementation evidence, not final convergence, live host qualification or completed US3/FR-009.

## Red-to-green and executable coverage

Four initial CLI tests failed because `usage snapshot` and `usage recover` did not exist (`/tmp/ai-gent-usage-recovery-red.log`, local diagnostic only). Implemented protocol tests are committed in `rust/crates/ai-gent/tests/agent_046_recovery.rs`; no handoff depends on that temporary log.

13 CLI tests pass: immutable content-addressed local snapshots and exact retries; predecessor/sequence fork prevention; read-only recovery planning and explicit apply retaining original bytes; corrupt interior refusal; recovery retries after later work without truncation; changed-source digest pinning; corrupt receipt preservation; concurrent snapshot retries publishing one object; refusal to discard a snapshot-bound record; global request identity across operations; malformed/version/nil identifier refusal before publication; and snapshot-retry refusal after either bound registration or usage prefixes are lost. Existing 14 registration and 18 ingestion tests also passed on the starting US2 baseline. The parent's review fixes are now incorporated, and all 22 corrected ingestion tests passed in final workspace validation.

Five new core test suites in `rust/crates/ai-gent-core/src/usage/recovery_tests.rs` exercise:

- failed append writes, failed flush and failed directory sync return errors and retain exactly-once retry behavior;
- partial append is preserved until explicit digest-pinned recovery;
- failures before write/flush/link/directory-sync publication boundaries leave no final receipt or a complete retryable receipt;
- recovery interrupted before/after truncation retains a durable original copy and resumes;
- competing locks time out after the bounded wait with a retryable error and no mutation.

All 13 core usage tests passed (eight existing plus these five). Fault checkpoints are thread-local under `cfg(test)`; runtime callers cannot select them. They simulate filesystem errors/interrupted states, not actual process death or power failure. Pinned Rust 1.98.1 clippy passed for `ai-gent-core`/`ai-gent`, all targets, warnings denied. Focused commands used offline locked dependencies in an isolated target directory. Final full `scripts/check.sh --quiet` passed, including workspace tests, lint, guard parity, installers, migration and packaging checks (local log `/tmp/ai-gent-usage-recovery-final-check.log`, final line `all checks passed`). The initial full-check attempt was intentionally stopped to relocate its owned cache off a small temporary filesystem; the final check ran from the repository disk. Optional cargo-deny was unavailable locally. The plan validator passed with 12 completed tasks; full US3 remains open.

## Delivered boundary and remaining work

The implemented wire/storage contract is `../contracts/recovery-and-snapshots.md`; snapshot shape is `../contracts/ledger-snapshot.schema.json`. `usage_snapshot` receipts bind the durable journal prefixes and a sanitized numeric summary, with predecessor digest/schema and explicit `local_unverified`/`unclosed` labels. No accepted/integrated state or provider certification exists. Ordinary replay/ingest never repairs evidence implicitly.

Recovery requires an existing complete registration, validates both complete log prefixes, preserves damaged originals and a canonical recovery manifest before truncation, and refuses to remove published snapshot-bound evidence. Directory/file permissions follow the existing private store. Atomic no-clobber hard-link publication fails on unsupported filesystems; Unix durability is exercised locally, and other operating systems/filesystems remain unqualified.

Bulk versioned import with dry-run validation, disposable projections, 100k-event/16-writer target performance, actual process-kill/power-loss testing, broad cross-platform qualification, independent role/authorization journeys and final convergence remain open. The numeric snapshot JSON is exportable, but it is not the missing bulk import/export workflow. Do not close FR-009, the complete US3 layer pointer, T010 or final QA from these results alone.

## Independent review correction

A fresh reviewer found that an identical snapshot request could return its old success result after the current journal had been rolled back to a valid shorter prefix. Normal replay detected the missing receipt-bound record, but the retry path skipped source-evidence verification. Two real CLI regressions reproduced the issue independently for registration and usage logs before the fix. Snapshot now verifies all bound journal prefixes under the repository lock before any duplicate-request acknowledgement. This preserves original retry results only while their evidence still exists. Focused revalidation and the fresh reviewer's independent repeat passed: all 13 recovery CLI tests, five new core fault suites and receipt/quarantine symlink-escape probes. No remaining blocker was found for this bounded slice; the recommendation is limited to its implemented boundary, not whole-feature convergence or live qualification.
