# Portable evidence transfer implementation — 2026-11-18

Status: bounded 046 FR-009 transfer slice on `feat/usage-transfer`, initially based on the published recovery slice `e0d2d91`. No claim of full US3 completion, final convergence, live host qualification, provider authentication or local measured usage from imported archives.

## Implementation and red-to-green evidence

The first four real-CLI tests failed because `usage export`/`usage import` were absent (`/tmp/ai-gent-usage-transfer-red.log`, local diagnostic only). Source of reproducible evidence is `rust/crates/ai-gent/tests/agent_046_transfer.rs`, not that temporary log.

Ten transfer CLI tests pass, covering:

- sanitized export, empty-root read-only dry run, isolated import, identical retries and unchanged local totals;
- direct source session/namespace/request/path sanitation while retaining model identity labels;
- altered input/version refusal without files, corrupt source refusal without rewriting evidence;
- global request UUID conflicts against existing registration/usage operations;
- corrupt imported bundle/manifest preservation and refusal on both retry and dry run;
- malformed metadata, nil IDs, parent cycles, unsupported verified claims, extra summaries, omitted nullable fields and unhashed source identifiers;
- symlink escape refusal before journal creation (Unix);
- eight concurrent import retries publishing one bundle and one manifest;
- shared/unattributed scopes retained without converting them into exclusive or locally measured usage.

Four new core tests cover invalid run claims/graphs, a failure exactly between durable bundle and manifest publication with successful idempotent retry, and a near-maximum portable bundle whose import envelope fits the 1 MiB request limit while the next export row fails with a typed bound. Seventeen core usage tests are covered in total (13 previous + four new), including the independently reviewed native segment/attempt-order regressions. Existing 14 registration, 22 ingestion and 13 recovery/snapshot CLI tests also pass with the new namespace checks.

Pinned Rust 1.98.1 `cargo clippy --offline --locked -p ai-gent-core -p ai-gent --all-targets -- -D warnings` passed. Focused suite and core logs were `/tmp/ai-gent-usage-transfer-tests.log` and `/tmp/ai-gent-usage-transfer-core.log`; lint log was `/tmp/ai-gent-usage-transfer-clippy.log`. The coordinator owns the independent review/full repository check and records their final results before committing. No commit or push was made by the builder.

## Wire and trust boundary

Contract: `../contracts/evidence-transfer.md`. Structural schema: `../contracts/evidence-transfer.schema.json`. Runtime checks additionally enforce source pseudonym vocabulary, full task/run/attempt/segment/reference invariants, acyclic parents, state/generation consistency, effective identity across streams, canonical counters, disjoint contributions and byte limits. Source summaries are recomputed, never accepted from a caller.

Import stores canonical bundle bytes and a request-bound manifest in `.ai-gent/usage/imports`. It never recreates current runs, appends current observations, modifies receipt chains or adds imported counters to local replay totals. Imported archive quality/local assurance/unclosed coverage remain explicit independently of preserved source-declared observation quality. A successful import is not certification of provider/model identity, billing, source completeness or task acceptance.

Export pseudonymizes direct source identifiers/paths and removes host sessions. Model/provider/configuration labels, TaskKeys/UUIDs and actual validated timestamps remain operational metadata. These caller-supplied labels may themselves identify an account or contain private text: this is pseudonymization, not guaranteed anonymization. Unsalted hashes permit correlation and dictionary guesses. The contract states this boundary explicitly.

## Remaining scope

Bundles are bounded at `1 MiB − 4096 bytes` to reserve request-envelope space, with at most 2048 runs/observations. Import storage is bounded at 4096 entries/64 MiB, including prospective publication. No pagination, streaming, large-ledger restore or reconciliation into authoritative totals is implemented. Large transfers, disposable projections, target-scale workload, actual crash/power-loss/platform qualification, protected collectors and full-feature convergence remain open. Do not check full FR-009/US3 or final QA merely because these bounded tests passed.


## Independent review corrections

The fresh reviewer reproduced two metadata bypasses: an unchanged configuration could be assigned a new adjacent segment UUID, and a child attempt could claim an older ordinal than its parent. Native registration/segment semantics reject those states. A new core regression failed before correction; archive validation now rejects adjacent equal segment configurations and requires every parent attempt ordinal to precede its child. A fresh reviewer independently confirmed both corrections reject through the real CLI without creating a store. Final read-only review on 2026-11-24 found no remaining blocker for the bounded transfer and Unix process-kill slice and recommends approval after the full repository check. No broader source authenticity or scope qualification follows from this validation.


## Combined continuation validation — 2026-11-24

Combined source passed 10 transfer, 14 registration, 22 ingestion and 13 recovery CLI tests, plus 23 core usage tests. One ignored unit-test fixture is explicitly invoked by the six parent process-kill tests; it is not an omitted qualification case. See `process-crash-slice.md` for 19 actual SIGKILL boundaries and their limits. Full `scripts/check.sh --quiet` passed on 2026-11-24 with the combined source, including workspace tests, lint, guard/legacy parity, installers, migrations and packaging (local log `/tmp/ai-gent-usage-transfer-final-check.log`, final line `all checks passed`). The structural plan validator also passed: 9 features, 27 stories, 81 requirements, 198 tasks, 12 completed. No whole-feature or live-provider qualification is inferred. The existing replay baseline predates archive-index guards, so archive scanning is separately recorded as an additional capacity limitation.
