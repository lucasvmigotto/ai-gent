# 046 registration slice — development handoff

Date: 2026-11-18. Branch `feat/task-usage-ledger`, based on the published plan ead8e96 and unchanged develop 0a61fc4. No merges, release changes or runtime plugin/skill/hook/shared-reference edits. First slice is US1, not completion of 046 or the token tracker.

## What works

`ai-gent usage register|segment|transition|replay` and a reusable Rust core API. Registered TaskKey includes explicit repository UUID, feature slug and task ID. Task declarations are checked in the chosen project; fingerprints ignore checkbox state. Attempt/run/segment IDs use OS-random UUIDv4. All model/provider/host/config data is explicit; reported and unknown identities stay distinct. Optional host_session_id can be supplied rather than inferred. Immutable parent links, scoped attempt reuse and generation checks protect the execution graph.

Each mutation is idempotent by request ID under an advisory OS lock, with checksummed sequential frames and file flush before acknowledgement. Unix additionally flushes containing directories and creates private files/directories. Retries re-flush prior records before acknowledging them. Terminal execution never becomes accepted task completion. Unknown usage is emitted as missing, without fake zero counters.

See [the exact CLI contract](../contracts/registration.md) and `rust/crates/ai-gent/tests/fixtures/agent_046/register.json`. Normalized counters, ingestion/dedup, shared task scopes, closures and receipt publication are future work.

## Test evidence

- Four initial CLI tests failed against the baseline because usage was not a subcommand: tests first.
- Four core tests pass: digest shape, malformed/unknown/nil identifiers, input size bounds, scoped/idempotent registration and role mismatch rejection.
- Fourteen CLI integration tests pass: registration/replay, unknown identities, provider changes preserving prior segments, unknown task/parent/version/field rejection, cross-project identity, stale generation, same-attempt independent runs, related attempts, terminal states, task-definition changes, conflicting run IDs, two 16-process contention cases, torn/corrupt journal preservation, read-only replay, storage failure and Unix permissions/symlink boundaries, and automatic Git exclusion in a different project.
- Existing delegate_events (6) and delegate_plan (7) tests pass alongside the new suite: 27 focused CLI tests total. The source-fetch fixture first failed under restricted socket access, then passed when rerun with the required local socket permission; no test was changed.
- Pinned Rust 1.98.1 clippy passes with warnings denied. A first root-directory run selected the machine's newer Rust and exposed unrelated newer-lint diagnostics; it was replaced with the repository-pinned invocation from rust/.
- Full `scripts/check.sh --quiet` passed, exit 0, on the final source with all temporary diagnostics removed. Optional cargo-deny is unavailable and skipped by the existing script; Linux only. An earlier run encountered a transient compile error while the ignore-marker fix was being edited; that was corrected before the final source freeze. A subsequent full run failed in the unchanged dbrun backup fixture `real_exec::arguments_carry_no_password_and_stdin_reaches_the_tool` with “the container engine could not be started”. All 13 backup tests then passed unchanged in isolated/environment-matched diagnostic runs; a full workspace diagnostic run and ten repeated backup-suite runs also passed. Temporary errno diagnostics were removed before the final full check. The process-start failure cause remains unresolved; retain this baseline flake risk rather than claiming it was fixed. No tests/gates were weakened or given hidden retries.

No source/provider calls are made by registration tests. No live host surface or macOS/Windows support is newly qualified. No actual token usage was captured for this development phase: missing remains missing.

## Limits and review status

Seven tasks are checked for this bounded build slice. Independent review/convergence T010 and QA qualification remain open, so 046 stays In progress. Do not use local registrations as admission authorization or source/billing attestation. This CLI operates for the invoking local user; worker/supervisor isolation and owner policy belong to the later admission/collector integration.

Current journal limit is 64 MiB per replay, with 1 MiB request/frame limit. A torn tail is preserved and refuses further writes; automatic tail quarantine/repair and scalable projections remain US3. Unix directory sync is implemented; Windows durability/ACL semantics remain unqualified. Ancestor path replacement by an adversarial same-user process is outside the current local-integrity guarantee. Private journals are local to the selected project root; a single shared supervisor store for worktrees/machines requires explicit later integration. Local attempt ordinals are informational, not global sequence clocks.

## Resume

Fetch origin/feat/task-usage-ledger and use a separate worktree. Run the feature tests from rust/ with the pinned toolchain:

```sh
cargo test --locked -p ai-gent-core usage::
cargo test --locked -p ai-gent --test agent_046
```

Next: 046 US2 T005/T014 first, then normalized counters/dedup T015/T016. Preserve the distinction between observations and authoritative contributions, input/cache/reasoning subset semantics, reported vs verified identity and exclusive/shared attribution. Finish US3 failure drills/receipt envelopes before calling the feature complete. The old dependency fix branch remains separate and is not pulled into this registration slice.

Final full-check log SHA-256: `e19a6319bf48e2fe465772cc7bdeca1ebbcc57da5370b8f61021fa2fbfc253ae`. Local log: `/tmp/ai-gent-usage-046-full-check.log`. Final check environment used explicit existing Rust/Cargo homes/cache and unset inherited NO_COLOR for installer color fixtures. This does not claim the intermittent backup process-start failure was repaired.
