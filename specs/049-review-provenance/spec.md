# Feature Specification: Independent review and repair provenance

**Feature Branch**: `docs/codex-parity-plan`
**Created**: 2026-11-17
**Status**: Planned
**Input**: Owner requests full code-agent parity planning, mandatory task usage, multi-model/provider builder–reviewer metrics, and equal Claude + Codex Tier 1 with portable Tiers 2/3.
**Program**: [scope and decisions](../../docs/product/agent-parity-plan/README.md).
**Dependencies**: 46, 48.

## User Scenarios & Testing

Tests are required for every story. Stories are independently testable on the foundational contract; later stories add capabilities without making earlier acceptance require a live paid model call.

### User Story 1 — Review the exact artifact independently (Priority: P1)

A reviewer assesses a precise revision in a fresh context, including Codex reviewing Codex.

**Why this priority**: Establish the minimum trustworthy workflow before adding projections or convenience.

**Independent Test**: `independent_exact_revision_review` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given a Codex build and fresh Codex review session on the same provider, when ordinary review policy allows it, then review is eligible with both session/model identities recorded.
2. Given the builder session reused as reviewer or the tree changes after review, when acceptance is requested, then review is ineligible and the reason is reported.

### User Story 2 — Track findings through repairs (Priority: P1)

The owner can follow every finding, repair and re-review without losing the original attempt cost.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `findings_repair_rereview` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given a reviewer requests changes and a repair produces a new tree, when re-review passes, then both reviews and the repair run remain linked with finding dispositions.
2. Given a finding is silently removed, a stale review reused or a review artifact hand-edited, when verification runs, then acceptance rejects the chain.

### User Story 3 — Verify review evidence mechanically (Priority: P2)

A completion consumer checks review provenance without trusting Markdown verdicts.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `review_verifier_and_round_budget` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given valid review evidence for a registered artifact, when verify runs, then it returns policy eligibility and actionable findings separately.
2. Given missing identity, an unavailable reviewer, corrupt evidence or a same-session review, when strict policy runs, then it fails without launching another model.

### Edge Cases

Missing/null fields, unsupported versions, conflicting duplicates, wrong repository/task scope, interrupted writes, two concurrent actors and stale artifact/policy evidence must produce explicit diagnostics. The specific examples above and qa.md are required regression cases.

## Requirements

### Functional Requirements

- **FR-001** (US1): System MUST allow same-agent and same-provider review with a fresh execution context; require different model/provider only when the approved policy asks for it.
- **FR-002** (US1): System MUST bind review to builder run, immutable tree/revision and diff digest, review policy, reviewer session/run, effective model/provider and evidence of fresh context.
- **FR-003** (US1): System MUST separate independence evidence, permission evidence and semantic review verdict; no reviewer role label or model name proves sandbox enforcement or independence by itself.
- **FR-004** (US2): System MUST assign stable finding IDs, severity, location, evidence and disposition; track fixed, accepted-risk and disproved dispositions with actor and rationale.
- **FR-005** (US2): System MUST link repair runs to their source review and parent attempt and require re-review of each changed tree; retain failed/aborted review rounds and all their usage.
- **FR-006** (US2): System MUST provide explicit owner-authorized review waivers with scope/reason/expiry and distinct waived status; a waiver never becomes an independently reviewed result.
- **FR-007** (US3): System MUST make review submission schema-validated, idempotent and durable before a supervisor can use it; workers cannot issue authoritative acceptance receipts.
- **FR-008** (US3): System MUST expose a deterministic verifier for task, attempt, builder run, review session, artifact digest, policy and finding closure, with typed stale/missing/conflicting errors.
- **FR-009** (US3): System MUST budget maximum review rounds in the approved plan, stop at that boundary and require a new owner decision rather than autonomous review loops.

### Key Entities

- **ReviewPolicy**: fresh-context rule, optional model/provider diversity, permissions and maximum rounds.
- **ReviewRecord**: review run, builder run, exact artifact, findings, verdict and provenance.
- **Finding**: stable ID, severity, evidence, disposition and authorization reference.
- **RepairLink**: review ID, repair run, resulting tree and required next review.
- **ReviewWaiver**: owner identity/reference, scope, reason, expiry and never-reviewed label.

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
