# Feature Specification: Tier 1 lifecycle, guard and recovery integration

**Feature Branch**: `docs/codex-parity-plan`
**Created**: 2026-11-17
**Status**: Planned
**Input**: Owner requests full code-agent parity planning, mandatory task usage, multi-model/provider builder–reviewer metrics, and equal Claude + Codex Tier 1 with portable Tiers 2/3.
**Program**: [scope and decisions](../../docs/product/agent-parity-plan/README.md).
**Dependencies**: 46, 47, 48, 50.

## User Scenarios & Testing

Tests are required for every story. Stories are independently testable on the foundational contract; later stories add capabilities without making earlier acceptance require a live paid model call.

### User Story 1 — Qualify host enforcement honestly (Priority: P1)

A Claude or Codex user can inspect whether collection, tool normalization and guard decisions actually operate on their host.

**Why this priority**: Establish the minimum trustworthy workflow before adding projections or convenience.

**Independent Test**: `tool_normalization_and_hook_truth` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given equivalent shell/Edit/apply_patch credential-target fixtures, when the host normalizer runs, then canonical targets and guard decisions agree or the capability is explicitly unsupported.
2. Given an ask-only decision on a host lacking ask, an untrusted hook or a hook timeout/error, when doctor probes it, then enforcement is degraded/blocked and never declared effective.

### User Story 2 — Recover a supervisor across hosts (Priority: P1)

A stopped coordinator can replay workers and pending acceptance on either Tier 1 host without a Claude-only wakeup.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `recovery_replay_and_takeover` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given workers finish while the coordinator is offline, when it resumes via status/recover, then durable events and acknowledgements reveal exactly which actions remain.
2. Given duplicate wakeups, PID reuse or an expired lease, when replay occurs, then no duplicate worker or acceptance is created.

### User Story 3 — Recap and hand off without host bias (Priority: P2)

An agent at another machine can find the plan and pending work without an ignored worktree or Claude-stamped identity.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `history_handoff_and_identity` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given Codex, Claude and supported lower-tier history fixtures, when recap runs, then it identifies the actual host/session and unfinished requests with redacted evidence.
2. Given unsupported private history or unavailable local paths, when handoff is generated, then it names the limitation and uses committed branch/revision links rather than claiming continuity.

### Edge Cases

Missing/null fields, unsupported versions, conflicting duplicates, wrong repository/task scope, interrupted writes, two concurrent actors and stale artifact/policy evidence must produce explicit diagnostics. The specific examples above and qa.md are required regression cases.

## Requirements

### Functional Requirements

- **FR-001** (US1): System MUST normalize host tool payloads to the existing evaluator contract, including Codex patch text and paths, with shared parity fixtures; uncertain parse or unsupported decision cannot silently authorize protected operations.
- **FR-002** (US1): System MUST provide Tier 1 installation/trust/doctor probes and feature capability evidence per OS/version/surface; resolve host ask limitations through a supported native approval path or explicit block/degraded status.
- **FR-003** (US1): System MUST treat hooks as host-local integration rather than complete OS isolation; document direct-shell/filesystem bypass boundaries and require external CI verification for acceptance.
- **FR-004** (US2): System MUST replace mandatory Claude Monitor assumptions with durable event replay, acknowledgements, polling/status and optional host wakeups; work must progress using the CLI alone.
- **FR-005** (US2): System MUST persist phase/run state before events become visible, reject ignored append failures, and support explicit takeover generation so two supervisors cannot control the same lease.
- **FR-006** (US2): System MUST bound event retention and helper timeouts; compact only acknowledged projections while keeping immutable usage/receipt evidence required for verification.
- **FR-007** (US3): System MUST add explicit host selection and versioned Codex history adapters to recap/checkpoint; never stamp Claude by default when identity is unknown.
- **FR-008** (US3): System MUST keep local checkpoints private but publish sanitized project handoffs on work branches, recording remote/ref/revision, pending work and independently replayable commands.
- **FR-009** (US3): System MUST provide generic lower-tier handoff/import support without reading credentials, unbounded sessions or private transcripts into tracked documents.

### Key Entities

- **HostCapability**: host/version/OS/surface, observed probe results and evidence timestamp.
- **NormalizedTool**: canonical verb, targets, patch paths and parse certainty.
- **SupervisorCursor**: owner generation, acknowledged event sequence and lease.
- **Wakeup**: optional notification ID, durable event range and acknowledgement.
- **Handoff**: remote/ref/revision, sanitized requests, state, pending steps and host identity.

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
