# Feature Specification: Provider-neutral planning and admitted execution

**Feature Branch**: `docs/codex-parity-plan`
**Created**: 2026-11-17
**Status**: Planned
**Input**: Owner requests full code-agent parity planning, mandatory task usage, multi-model/provider builder–reviewer metrics, and equal Claude + Codex Tier 1 with portable Tiers 2/3.
**Program**: [scope and decisions](../../docs/product/agent-parity-plan/README.md).
**Dependencies**: 46.

## User Scenarios & Testing

Tests are required for every story. Stories are independently testable on the foundational contract; later stories add capabilities without making earlier acceptance require a live paid model call.

### User Story 1 — Route roles by capability (Priority: P1)

The owner can choose Claude or Codex as coordinator and route builder/reviewer work by capability and budget.

**Why this priority**: Establish the minimum trustworthy workflow before adding projections or convenience.

**Independent Test**: `role_routing_and_admission` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given the same task graph and equivalent capabilities, when either Tier 1 host coordinates, then the approved dependency/ownership plan is identical except explicitly configured routing.
2. Given a requested provider/model/profile unavailable on a selected adapter, when planning runs, then it explains the missing capability without silently substituting.

### User Story 2 — Schedule dependencies and ownership separately (Priority: P1)

A coordinator runs independent tasks concurrently while respecting shared files and accepted dependencies.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `dag_ownership_and_slot_races` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given independent ownership groups linked by a dependency edge, when scheduling runs, then ready groups may execute concurrently while the dependent group waits for accepted prerequisite evidence.
2. Given a dependency cycle, a duplicate scoped task or a stale prerequisite receipt, when admission runs, then no worker starts and the error identifies the graph fault.

### User Story 3 — Launch constrained workers reproducibly (Priority: P2)

A worker receives the approved configuration without ambient Git state or command-line prompt leakage.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `launch_isolation_and_limits` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given poisoned GIT_DIR/GIT_WORK_TREE, a long brief and two launches in one millisecond, when workers start, then each has unique IDs, correct worktree, private scratch and bounded stdin input.
2. Given a permission/profile mismatch, failed preflight or oversize input, when dispatch is requested, then it fails before process creation with no slot leak.

### Edge Cases

Missing/null fields, unsupported versions, conflicting duplicates, wrong repository/task scope, interrupted writes, two concurrent actors and stale artifact/policy evidence must produce explicit diagnostics. The specific examples above and qa.md are required regression cases.

## Requirements

### Functional Requirements

- **FR-001** (US1): System MUST replace hard-coded Claude judgment/coordinator policy with named planner/builder/reviewer/reader roles; make provider/model diversity selectable independently of agent application.
- **FR-002** (US1): System MUST parse the repository delegation policy, retain explicit opt-in, require an approved immutable plan digest and capability preflight before dispatch, and reject direct-run bypass.
- **FR-003** (US1): System MUST record requested versus effective model, provider, effort, profile, sandbox, network and approval configuration in registration and launch evidence; unknown capabilities cannot satisfy strict requirements.
- **FR-004** (US2): System MUST separate dependency DAG edges from declared file/resource ownership conflicts; preserve deterministic stable track identity and reject cycles, unknown references and self-dependencies.
- **FR-005** (US2): System MUST reserve worker slots atomically under one supervisor admission transaction, enforce configured global/per-provider caps and release abandoned leases through bounded recovery.
- **FR-006** (US2): System MUST make dependencies consume revision-bound accepted/integrated evidence rather than text markers or last worker exit; external prerequisites use explicit scoped references.
- **FR-007** (US3): System MUST use collision-resistant run IDs, allowlisted child environments, explicit Git/worktree context and private run scratch; never share a broad temporary directory as an authorization boundary.
- **FR-008** (US3): System MUST pass briefs through bounded stdin or a restrictive file with a documented lifecycle, never shell interpolation or unbounded argv; cap output, event bytes, process time and Git-helper time.
- **FR-009** (US3): System MUST report effective restrictions separately from role labels; native child inheritance cannot be presented as an independent read-only sandbox, and unsupported security requests fail closed.

### Key Entities

- **RoleRoute**: role, agent application, provider/model constraints, effort and fallback policy.
- **ExecutionPlan**: immutable digest, scoped tasks, DAG, ownership graph, policy/capability digests.
- **Admission**: plan digest, owner authorization reference and preflight result.
- **SlotLease**: run ID, reservation generation, capacity namespace, expiry and renewal.
- **LaunchEvidence**: requested/effective configuration, worktree/tree and bounded input digest.

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
