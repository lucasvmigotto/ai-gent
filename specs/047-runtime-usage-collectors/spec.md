# Feature Specification: Cross-agent runtime usage collectors

**Feature Branch**: `docs/codex-parity-plan`
**Created**: 2026-11-17
**Status**: Planned
**Input**: Owner requests full code-agent parity planning, mandatory task usage, multi-model/provider builder–reviewer metrics, and equal Claude + Codex Tier 1 with portable Tiers 2/3.
**Program**: [scope and decisions](../../docs/product/agent-parity-plan/README.md).
**Dependencies**: 46.

## User Scenarios & Testing

Tests are required for every story. Stories are independently testable on the foundational contract; later stories add capabilities without making earlier acceptance require a live paid model call.

### User Story 1 — Collect both Tier 1 hosts (Priority: P1)

A Claude or Codex user gets measured task usage from the strongest available native evidence.

**Why this priority**: Establish the minimum trustworthy workflow before adding projections or convenience.

**Independent Test**: `tier1_native_and_detached` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given versioned Claude and Codex interactive/detached fixtures, when their collectors run, then identity and normalized counters match the fixture oracle and host diagnostics are preserved.
2. Given a native hook with null token usage or an unsupported host version, when the run stops, then it is partial/missing with a concrete capability diagnostic.

### User Story 2 — Extend to other agents (Priority: P1)

A Tier 2/3 adapter can supply the same evidence without imposing one host hook API on every agent.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `extension_protocol_and_unknowns` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given OpenCode or a synthetic lower-tier adapter with validated output, when registered through the adapter protocol, then the shared ledger and gate consume it identically.
2. Given an agent with no accessible counters, when collection closes, then the task remains usable but cannot claim complete measured usage under strict acceptance.

### User Story 3 — Close evidence after execution (Priority: P2)

A task stops once while final usage is collected without invoking more models.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `collector_terminal_drain` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given terminal usage arrives after process exit or Stop, when bounded drain completes, then the supervisor closes all runs before issuing accounting evidence.
2. Given missing tail events, collector failure or restart, when recovery runs, then durable cursors resume idempotently and pending accounting remains visible.

### Edge Cases

Missing/null fields, unsupported versions, conflicting duplicates, wrong repository/task scope, interrupted writes, two concurrent actors and stale artifact/policy evidence must produce explicit diagnostics. The specific examples above and qa.md are required regression cases.

## Requirements

### Functional Requirements

- **FR-001** (US1): System MUST ship first-class Claude Code and Codex collectors for detached execution and supported interactive/native child execution; qualify each host/version/surface separately.
- **FR-002** (US1): System MUST use documented terminal/result or transcript sources where available, per-request hooks as observations, and parent/child IDs to prevent charging child usage twice to the parent.
- **FR-003** (US1): System MUST capture effective model/provider changes and source counter semantics; distinguish local runtime claims from provider-reconciled evidence and do not infer model from the executable name.
- **FR-004** (US2): System MUST expose a versioned collector protocol for adapter-declared event sources, identity evidence, lifecycle closure and counter semantics; validate every adapter payload at the core boundary.
- **FR-005** (US2): System MUST inventory all AGENT_IDS, provide a lower-tier generic wrapped-command/import path, implement a qualified OpenCode adapter as the first extension, and publish per-host capability evidence rather than a blanket support claim.
- **FR-006** (US2): System MUST allow imported provider exports with matching identities and reconciliation windows; absent exports, identity joins or capabilities MUST remain explicitly unverified, never fabricated.
- **FR-007** (US3): System MUST treat Stop/SubagentStop/process-exit as lifecycle signals, not an accounting total or acceptance decision; collect final usage with bounded drain and mark incomplete evidence on deadline.
- **FR-008** (US3): System MUST resume collectors by durable source cursors across rotation/truncation; reject regressions and expose retryable transport failures separately from permanent unsupported schemas.
- **FR-009** (US3): System MUST recovery MUST NOT restart a builder, block Stop in a loop or invoke an LLM to recover telemetry; redact paths/secrets and bound source bytes before parsing.

### Key Entities

- **CollectorCapability**: host, version range, OS, surface, evidence sources, known gaps and qualification digest.
- **SourceCursor**: registered source, offset/sequence, file identity and last acknowledged event.
- **RunClosure**: terminal signal, last sequence, drain deadline, unresolved requests and completeness.
- **ProviderReconciliation**: provider export digest, matching window, matched and unmatched IDs; no fabricated match.

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
