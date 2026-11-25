# Tasks: Task identity and durable usage ledger

**Input**: spec.md, plan.md, research.md, data-model.md, contracts/cli.md, backend.md and qa.md.
**Status**: US1 registration and US2 ingestion implementation and tests are complete; US3 recovery qualification, independent review and final qualification remain open. Tests are required.
**Roles**: executor builder, independent reviewer thinker; both roles can be Claude or Codex. Model/provider selection follows approved capabilities.
**Scope**: `046-task-usage-ledger`; task IDs below are local to this feature, never globally unique alone.

## Phase 1: Setup

- [x] T001 Review `specs/046-task-usage-ledger/contracts/cli.md` against upstream dependencies; freeze operation/JSON compatibility and file ownership before coding. Executor: builder · Reviewer: thinker · Effort: medium

**Checkpoint**: dependency versions and ownership reservations recorded; no runtime change yet.

## Phase 2: Foundational

- [x] T002 Establish synthetic fixtures and shared validation helpers under `rust/crates/ai-gent/tests/fixtures/agent_046/`; include schema/identity/version rejection cases. Executor: builder · Reviewer: thinker · Effort: medium

**Checkpoint**: malformed inputs fail for the intended reason; existing behavior remains unchanged.

## Phase 3: User Story 1 — Register work before execution

**Goal**: A coordinator registers a globally unambiguous task and execution scope before any model invocation.
**Independent test**: `identity_and_segment_boundaries`; scenarios and required negative paths in qa.md.

- [x] T003 [US1] Define failing positive, negative and replay assertions for FR-001, FR-002, FR-003, SC-001, SC-002 and SC-003 in `rust/crates/ai-gent/tests/agent_046.rs`; record the expected failure before implementation. Executor: builder · Reviewer: thinker · Effort: medium
- [x] T004 [US1] Backend implementation — see US1 in ## Backend of `specs/046-task-usage-ledger/tasks.md`; close only after its concrete tasks and independent story test pass. Executor: builder · Reviewer: thinker · Effort: medium

**Checkpoint**: story works independently against the foundation; prior story tests still pass.

## Phase 4: User Story 2 — Ingest evidence exactly once

**Goal**: A collector records source usage without double counting overlapping counters or replayed events.
**Independent test**: `usage_semantics_and_dedup`; scenarios and required negative paths in qa.md.

- [x] T005 [US2] Define failing positive, negative and replay assertions for FR-004, FR-005, FR-006, SC-001, SC-002 and SC-003 in `rust/crates/ai-gent/tests/agent_046_usage.rs`; record the expected failure before implementation. Executor: builder · Reviewer: thinker · Effort: medium
- [x] T006 [US2] Backend implementation — see US2 in ## Backend of `specs/046-task-usage-ledger/tasks.md`; close only after its concrete tasks and independent story test pass. Executor: builder · Reviewer: thinker · Effort: medium

**Checkpoint**: story works independently against the foundation; prior story tests still pass.

## Phase 5: User Story 3 — Recover durable records

**Goal**: A supervisor can restart after a crash without losing acknowledged usage or presenting a torn receipt.
**Independent test**: `journal_crash_and_concurrency`; scenarios and required negative paths in qa.md.

- [ ] T007 [US3] Define failing positive, negative and replay assertions for FR-007, FR-008, FR-009, SC-001, SC-002 and SC-003 in `rust/crates/ai-gent/tests/agent_046.rs`; record the expected failure before implementation. Executor: builder · Reviewer: thinker · Effort: medium
- [ ] T008 [US3] Backend implementation — see US3 in ## Backend of `specs/046-task-usage-ledger/tasks.md`; close only after its concrete tasks and independent story test pass. Executor: builder · Reviewer: thinker · Effort: medium

**Checkpoint**: story works independently against the foundation; prior story tests still pass.

## Phase 6: Polish

- [ ] T009 Reconcile compatibility, errors and examples in `specs/046-task-usage-ledger/contracts/cli.md`; update affected README, CHANGELOG, AGENTS and docs-site pages if runtime skills/plugins changed, following skill-docs and prior approval. Executor: builder · Reviewer: thinker · Effort: medium
- [ ] T010 Complete independent review and `/speckit-converge` for `specs/046-task-usage-ledger`; run focused suites then full `scripts/check.sh`, record evidence and leave unmet capabilities unqualified. Executor: builder · Reviewer: thinker · Effort: high

**Checkpoint**: Verified only with qa.md exit evidence; task flags alone are insufficient.

## Backend

Follow Setup → Foundational → US1 → US2 → US3 → Polish above. These are the concrete tasks referenced by layer pointers; do not implement the pointer twice. Tests precede domain/CLI changes.

### US1: Register work before execution

- [x] T011 [US1] Add domain/contract tests for FR-001, FR-002, FR-003 to `rust/crates/ai-gent-core/src/usage/tests.rs` or existing module tests; assert authorization denials, invalid versions and idempotency. Executor: builder · Reviewer: thinker · Effort: medium
- [x] T012 [US1] Implement repository identity, TaskKey, attempt/run registration and immutable segment transitions in `rust/crates/ai-gent-core/src/usage/identity.rs` and adjacent domain types; satisfy FR-001, FR-002, FR-003. Executor: builder · Reviewer: thinker · Effort: high
- [x] T013 [US1] Wire the story's operations through `rust/crates/ai-gent/src/cli.rs` and `rust/crates/ai-gent/src/cmd/`; execute `rust/crates/ai-gent/tests/agent_046.rs` and legacy compatibility readers, recording positive/negative/replay results. Executor: builder · Reviewer: thinker · Effort: medium

**Checkpoint**: T011–T013 and the story acceptance test pass; then close its layer pointer.

### US2: Ingest evidence exactly once

- [x] T014 [US2] Add domain/contract tests for FR-004, FR-005, FR-006 to `rust/crates/ai-gent-core/src/usage/tests.rs` or existing module tests; assert authorization denials, invalid versions and idempotency. Executor: builder · Reviewer: thinker · Effort: medium
- [x] T015 [US2] Implement source semantics, authoritative contribution selection, subset/overflow validation and quality dimensions in `rust/crates/ai-gent-core/src/usage/normalize.rs` and adjacent domain types; satisfy FR-004, FR-005, FR-006. Executor: builder · Reviewer: thinker · Effort: high
- [x] T016 [US2] Wire the story's operations through `rust/crates/ai-gent/src/cli.rs` and `rust/crates/ai-gent/src/cmd/`; execute `rust/crates/ai-gent/tests/agent_046.rs` and legacy compatibility readers, recording positive/negative/replay results. Executor: builder · Reviewer: thinker · Effort: medium

**Checkpoint**: T014–T016 and the story acceptance test pass; then close its layer pointer.

### US3: Recover durable records

- [ ] T017 [US3] Add domain/contract tests for FR-007, FR-008, FR-009 to `rust/crates/ai-gent-core/src/usage/tests.rs` or existing module tests; assert authorization denials, invalid versions and idempotency. Executor: builder · Reviewer: thinker · Effort: medium
- [ ] T018 [US3] Implement lock/append/flush/replay, projection generation and content-addressed receipt envelope publication in `rust/crates/ai-gent-core/src/usage/journal.rs` and adjacent domain types; satisfy FR-007, FR-008, FR-009. Executor: builder · Reviewer: thinker · Effort: high
- [ ] T019 [US3] Wire the story's operations through `rust/crates/ai-gent/src/cli.rs` and `rust/crates/ai-gent/src/cmd/`; execute `rust/crates/ai-gent/tests/agent_046.rs` and legacy compatibility readers, recording positive/negative/replay results. Executor: builder · Reviewer: thinker · Effort: medium

**Checkpoint**: T017–T019 and the story acceptance test pass; then close its layer pointer.

## QA

- [ ] T020 Execute independent multi-persona CLI journeys and concurrency/fault scenarios from `specs/046-task-usage-ledger/qa.md` against `rust/crates/ai-gent/tests/agent_046.rs`; report uncovered FR/scenario cells. Executor: builder · Reviewer: thinker · Effort: high
- [ ] T021 Qualify relevant Claude/Codex and lower-tier adapter cells using the probe manifest in `docs/product/agent-parity-plan/capabilities.md`; store sanitized evidence under `specs/046-task-usage-ledger/evidence/` only after running the probe, never placeholder success. Executor: builder · Reviewer: thinker · Effort: medium
- [ ] T022 Integrate deterministic feature suites into `scripts/check.sh`/CI where not already discovered; record target workload results and limitations in `specs/046-task-usage-ledger/qa.md` and complete the story traceability review. Executor: builder · Reviewer: thinker · Effort: medium

**Checkpoint**: zero unaccounted scenario gaps, no open High defects and no false live capability claim.

## Dependencies and Execution Order

Feature prerequisites: 032 (existing workspace). Local dependency graph: T001 → T002 → story failing test → Backend tests → Backend implementation → CLI/test verification → layer pointer. Complete US1 before US2/US3 integration unless disjoint fixture-only work is explicitly reserved. T020–T022 and T009 precede T010's final review/check. Numerical IDs reflect layer extension order, not a naive total execution order; use this graph.

## Parallel Work and Delivery Strategy

No task is marked [P]: these stories intentionally share a domain module, CLI wiring and evidence store. Parallelize across features only after dependencies and the program ownership matrix permit it; independent fixtures may be split after reserving distinct files. Ship US1 inert/opt-in, then add US2/US3, preserving old readers. Never run two agents against the same shared CLI/registry file simultaneously. Each branch is published with a sanitized handoff for the agent on the other machine.

The exact local DAG is recorded in `task-graph.json`; it is planning metadata, not a claim that current delegate plan reads it. A build coordinator must translate it into the approved executable plan while preserving IDs and edges.

## Development evidence — 2026-11-18

Twelve tasks completed across registration and ingestion slices. See `evidence/ingestion-slice.md` for the ingestion contract, validation and remaining gaps. Seven tasks completed in the first registration slice. See `evidence/registration-slice.md` for actual tests, trust limits and pending work. T010 independent review/convergence remains unchecked; 046 is In progress, not Implemented/Verified.


## Bounded US3 progress — 2026-11-18

Recovery and local immutable snapshots are implemented on `feat/usage-recovery`; see `evidence/recovery-slice.md` and `contracts/recovery-and-snapshots.md`. A fresh reviewer found and verified correction of a snapshot retry evidence-verification bypass. Thirteen CLI recovery tests and five new core fault suites pass. Keep T007/T008/T017/T018/T019 unchecked: bulk versioned import/dry-run, actual process-kill/power-loss coverage and target workload qualification are not complete. Test-only failure checkpoints do not establish real crash durability or qualify new operating-system/host cells. T010/T020–T022 also remain open.


## Transfer and crash-test continuation — 2026-11-18

`feat/usage-transfer` adds bounded, versioned export/import with read-only dry-run validation. Imported archives stay separate from measured local runs and are classified imported/local-unverified/unclosed. See `contracts/evidence-transfer.md` and `evidence/transfer-slice.md` for exact bounds and trust limits. Actual process-kill tests extend earlier synthetic failure checkpoints; power-loss behavior remains unqualified. The measured replay baseline and correction plan are in `evidence/scale-baseline.md`. The 100k/16-writer target remains unqualified: v1's 64 MiB journal cannot fit it, and segmentation/projections are still required. Keep complete US3/convergence/QA task flags open until all their criteria pass.


## Indexed replay continuation — 2026-11-24

`perf/usage-ledger-indexes` removes redundant/quadratic contribution scans, reuses normalized quality within each summary, and verifies receipt raw prefixes in one incremental hash pass per journal. Compatibility cases pass before/after; see `evidence/indexed-replay-slice.md` for measured debug results, review, bounds and remaining work. This is a performance implementation slice, not completion of the projection/segmentation/load requirements in T018/T020–T022. Whole US3/convergence/QA flags remain open.
