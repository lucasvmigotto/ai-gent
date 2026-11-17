# Feature Specification: Required metered task completion

**Feature Branch**: `docs/codex-parity-plan`
**Created**: 2026-11-17
**Status**: Planned
**Input**: Owner requests full code-agent parity planning, mandatory task usage, multi-model/provider builder–reviewer metrics, and equal Claude + Codex Tier 1 with portable Tiers 2/3.
**Program**: [scope and decisions](../../docs/product/agent-parity-plan/README.md).
**Dependencies**: 46, 47, 48, 49.

## User Scenarios & Testing

Tests are required for every story. Stories are independently testable on the foundational contract; later stories add capabilities without making earlier acceptance require a live paid model call.

### User Story 1 — Accept only after evidence closes (Priority: P1)

A task owner gets a durable receipt for a tested, independently reviewed artifact with closed accounting.

**Why this priority**: Establish the minimum trustworthy workflow before adding projections or convenience.

**Independent Test**: `finish_pending_and_atomic_acceptance` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given a terminal builder, final collector drain, passing checks and eligible exact-tree review, when finish is requested, then one receipt is published before accepted status is visible.
2. Given usage is missing, a required run remains open, checks fail, or review refers to another tree, when finish runs, then work is preserved in accounting_pending or review_pending with typed reasons.

### User Story 2 — Enforce every completion consumer (Priority: P1)

A hand-edited task checkbox or worker done event cannot unblock dependent work or tracker synchronization.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `all_consumers_and_ci_bypass` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given a manually ticked task without a valid receipt, when check, dependency planning, tracker sync or CI evaluates it, then it reports unverified completion and blocks downstream acceptance.
2. Given two finish commands race or the log append fails after worker exit, when status is read, then at most one accepted receipt exists and no in-memory success leaks.

### User Story 3 — Migrate without inventing history (Priority: P2)

An existing project adopts strict accounting for new work while retaining honest legacy history.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `baseline_waiver_and_rollback` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given an explicit baseline commit and enforcement cutover, when legacy tasks are imported, then they remain imported/unknown and new registered tasks require receipts.
2. Given an unsupported host or deliberate exception, when a human waiver is issued, then it is separately visible and never counted as complete measured evidence.

### Edge Cases

Missing/null fields, unsupported versions, conflicting duplicates, wrong repository/task scope, interrupted writes, two concurrent actors and stale artifact/policy evidence must produce explicit diagnostics. The specific examples above and qa.md are required regression cases.

## Requirements

### Functional Requirements

- **FR-001** (US1): System MUST implement registered -> running -> execution_finished -> accounting_pending/review_pending -> accepted -> integrated transitions; failed/cancelled/exempt are explicit outcomes, not successful measured completion.
- **FR-002** (US1): System MUST require closed usage for every registered builder/reviewer/repair/coordinator run in task scope, complete evidence, eligible review, passed required checks and exact accepted tree under the stored policy.
- **FR-003** (US1): System MUST publish one immutable receipt atomically with compare-and-swap on attempt generation; emit accepted only after durable publication and use separate integration evidence for the landed revision.
- **FR-004** (US2): System MUST route delegate mark/check/status, worktree finish, spec status, dependency readiness, generated Markdown and tracker sync through the same verifier; worker done remains an execution signal.
- **FR-005** (US2): System MUST add a required CI command validating newly completed scoped tasks against sanitized receipts, exact diff/revision, evidence closure, deduplication, sums and reviewed policy; baseline changes cannot exempt newly registered work.
- **FR-006** (US2): System MUST reject stale task fingerprints, receipt replay across repositories/features/attempts, mutable policy downgrades and manifest omission; report local hooks as optional collection assistance rather than the sole enforcement boundary.
- **FR-007** (US3): System MUST stage observe -> warn -> strict rollout with a versioned repository policy and explicit baseline commit; strict is the target default for registered work after cutover, not an immediate rewrite of old statuses.
- **FR-008** (US3): System MUST support bounded human-authorized exemptions recording actor/reference/reason/scope/expiry; count exempt separately and preserve work when evidence is unavailable.
- **FR-009** (US3): System MUST keep v1 receipt verification readable during upgrades, make migration dry-run and repeatable, and reject rollback that silently relabels accepted/exempt/imported evidence.

### Key Entities

- **CompletionPolicy**: schema, baseline commit, cutover, strictness and required evidence dimensions.
- **CompletionIntent**: TaskKey, attempt generation, artifact digest, check/review/run references.
- **CompletionReceipt**: immutable links, quality, attribution, verification results and accepted artifact.
- **IntegrationAttestation**: receipt digest, landed revision/tree, merge/check evidence and actor.
- **Exemption**: scope, actor, reason, expiry and separate non-measured outcome.

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
