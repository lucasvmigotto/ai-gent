# Feature Specification: Task identity and durable usage ledger

**Feature Branch**: `feat/task-usage-ledger`
**Created**: 2026-11-17
**Status**: In progress
**Input**: Owner requests full code-agent parity planning, mandatory task usage, multi-model/provider builder–reviewer metrics, and equal Claude + Codex Tier 1 with portable Tiers 2/3.
**Program**: [scope and decisions](../../docs/product/agent-parity-plan/README.md).
**Dependencies**: existing Rust workspace (032).

## User Scenarios & Testing

Tests are required for every story. Stories are independently testable on the foundational contract; later stories add capabilities without making earlier acceptance require a live paid model call.

### User Story 1 — Register work before execution (Priority: P1)

A coordinator registers a globally unambiguous task and execution scope before any model invocation.

**Why this priority**: Establish the minimum trustworthy workflow before adding projections or convenience.

**Independent Test**: `identity_and_segment_boundaries` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given two repositories each containing T001, when runs are registered, then their repository/feature/task keys and attempt IDs remain distinct.
2. Given an unknown task or a reused run identity with changed contents, when ingestion is attempted, then it fails without mutating the accepted record.

### User Story 2 — Ingest evidence exactly once (Priority: P1)

A collector records source usage without double counting overlapping counters or replayed events.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `usage_semantics_and_dedup` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given repeated incremental events and one cumulative terminal summary, when replayed, then one authoritative contribution is counted per request and each raw observation remains traceable.
2. Given a counter reset, null usage, an invalid subset or conflicting duplicate, when normalized, then quality degrades or ingestion rejects with a typed reason; unknown never becomes zero.

### User Story 3 — Recover durable records (Priority: P2)

A supervisor can restart after a crash without losing acknowledged usage or presenting a torn receipt.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `journal_crash_and_concurrency` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given a crash at each append/flush/rename boundary, when replay runs, then acknowledged records survive and an incomplete tail is isolated with a diagnostic.
2. Given a full disk or a competing writer, when persistence fails, then success is never emitted and task acceptance remains blocked.

### Edge Cases

Missing/null fields, unsupported versions, conflicting duplicates, wrong repository/task scope, interrupted writes, two concurrent actors and stale artifact/policy evidence must produce explicit diagnostics. The specific examples above and qa.md are required regression cases.

## Requirements

### Functional Requirements

- **FR-001** (US1): System MUST assign repository UUID, feature slug, task ID, attempt UUID, phase, run UUID, parent run, session/thread and execution-segment identity before dispatch; task prose and branch names MUST NOT be identity sources.
- **FR-002** (US1): System MUST represent builder, reviewer, repair, planner, coordinator, verification and recovery phases; retain cancelled and failed attempts and reject orphan parent references or parent cycles.
- **FR-003** (US1): System MUST persist agent application, requested and effective provider/model/effort, configuration digest, source version and explicit unknown identity separately; changes create a new segment.
- **FR-004** (US2): System MUST preserve immutable raw numeric fields and source reference/digest alongside normalized total input, cached input, cache creation, output and reasoning with declared subset/additive semantics and measurement scope.
- **FR-005** (US2): System MUST deduplicate by source namespace/session/request/event identity, distinguish observations from contributions and cumulative from delta counters, and reconcile terminal snapshots without summing both.
- **FR-006** (US2): System MUST support complete, partial, imported and missing evidence independently of exclusive/shared/unattributed attribution and local/reconciled attestation; reject negative counts, integer overflow and cross-run replay.
- **FR-007** (US3): System MUST serialize writes per repository ledger with bounded lock wait, durable acknowledgement, checksummed frames and monotonic sequence; retain immutable event IDs across retries.
- **FR-008** (US3): System MUST create immutable content-addressed receipts by same-filesystem atomic publication after their evidence is durable; record predecessor digest and schema version; detect corruption without claiming provider authentication.
- **FR-009** (US3): System MUST keep raw transcripts and prompts outside tracked receipts, restrict file permissions where supported, enforce size/path limits, and provide versioned export/import with dry-run validation.

### Key Entities

- **TaskKey**: repository UUID + feature slug + task ID; immutable, unique within registration namespace.
- **Attempt**: UUID, TaskKey, ordinal, parent attempt, purpose and terminal outcome.
- **Run**: UUID, attempt, role, parent run, host session and lifecycle.
- **Segment**: run, ordinal, effective provider/model/effort/config and start/end sequence.
- **UsageObservation**: event ID, source identity, scope, raw counters, normalized counters, provenance and quality.
- **ReceiptEnvelope**: schema, evidence digests, ledger sequence and predecessor; publication is immutable.

## Success Criteria

### Measurable Outcomes

- **SC-001**: Every positive acceptance fixture reaches its stated outcome with an auditable task/run/artifact link; no external model invocation is needed for deterministic validation.
- **SC-002**: Every negative acceptance fixture rejects or explicitly degrades the claim as specified; zero unsupported cases are presented as qualified/complete.
- **SC-003**: Replaying each successful operation twice changes neither its authoritative counts nor its outcome; interruption/concurrency cases preserve the documented boundary.

## Assumptions and Clarification Decisions

- [DECIDED: owner] Equal Claude + Codex Tier 1; other agents remain supported by declared Tier 2/3 capabilities, without pretending missing telemetry is measured.
- [DECIDED: planning] Extend existing Rust core/CLI and package contracts. No HTTP API or new UI; existing Applicability remains authoritative.
- [DECIDED: planning] Fixtures validate logic; a real host/version/OS claim additionally needs a recorded live probe. Paid model evals remain opt-in.
- [DECIDED: planning] Program scope is a follow-on to the current develop baseline; it does not expand 3.0.0 gates or revoke existing platform waivers.
- [ASSUMPTION] Host-local runtime evidence can be incomplete or forged by a user controlling the collector; local integrity checks are not provider billing attestation.

## Scope Boundary

Implement only this feature's requirements; dependent features consume its contracts. Live provider billing certification, mandatory remote services and automatic model-quality rankings are outside scope. Unimplemented capabilities remain Planned.
