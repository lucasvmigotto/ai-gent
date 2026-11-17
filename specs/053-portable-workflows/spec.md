# Feature Specification: Portable skills, installation and workflow profiles

**Feature Branch**: `docs/codex-parity-plan`
**Created**: 2026-11-17
**Status**: Planned
**Input**: Owner requests full code-agent parity planning, mandatory task usage, multi-model/provider builder–reviewer metrics, and equal Claude + Codex Tier 1 with portable Tiers 2/3.
**Program**: [scope and decisions](../../docs/product/agent-parity-plan/README.md).
**Dependencies**: 48, 50, 52.

## User Scenarios & Testing

Tests are required for every story. Stories are independently testable on the foundational contract; later stories add capabilities without making earlier acceptance require a live paid model call.

### User Story 1 — Use equal Tier 1 entry points (Priority: P1)

A Claude or Codex user invokes the same lifecycle skills without provider-specific role restrictions or host assumptions.

**Why this priority**: Establish the minimum trustworthy workflow before adding projections or convenience.

**Independent Test**: `tier1_entry_points_and_dedup` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given the same task on each Tier 1 host, when the workflow is installed and invoked, then it reaches the same executable plan/admission/review/completion contract.
2. Given duplicate skills from shared and native-plugin installation, when setup/doctor runs, then discovery is deduplicated and the effective source is explicit.

### User Story 2 — Offer capability-based lower tiers (Priority: P1)

A supported lower-tier user can run the common toolbox with honest limits.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `lower_tier_capability_matrix` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given an installed AGENT_IDS host with only CLI access, when executing a registered task, then generic registration/collector import/finish commands remain available with declared evidence gaps.
2. Given no qualifying live test for a capability, when compatibility docs render, then the row stays unqualified rather than inheriting the Tier 1 guarantee.

### User Story 3 — Scale lifecycle rigor to the project (Priority: P2)

The owner selects a workflow profile without weakening required task evidence or silently granting permission.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `profiles_permissions_and_portable_evals` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given a CLI-only lite project, when planning runs, then irrelevant HTTP/frontend artifacts are explicitly N/A and the same task evidence requirements remain.
2. Given no user answer, a protected path or a high-assurance policy, when the workflow advances, then silence is not approval and profile changes cannot waive enforcement.

### Edge Cases

Missing/null fields, unsupported versions, conflicting duplicates, wrong repository/task scope, interrupted writes, two concurrent actors and stale artifact/policy evidence must produce explicit diagnostics. The specific examples above and qa.md are required regression cases.

## Requirements

### Functional Requirements

- **FR-001** (US1): System MUST make Claude and Codex equal Tier 1 targets for the new program, using one source of skill truth and generated host entry points; remove Claude-only role restrictions through reviewed instruction changes.
- **FR-002** (US1): System MUST choose Spec Kit integration from the active/detected host with explicit override; provide portable fallbacks for tool/Canvas/monitor assumptions and supported non-web project shapes.
- **FR-003** (US1): System MUST qualify optional native plugin packaging against current host manifests/trust APIs; retain shared skill discovery when native support is absent and prevent duplicate registration.
- **FR-004** (US2): System MUST define Tier 2 as adapter-qualified integration for declared capabilities and Tier 3 as best-effort portable skills/CLI; both consume the same identity, receipt and verifier contracts.
- **FR-005** (US2): System MUST track every registered agent across skills, CLI launch, telemetry, model identity, hooks, approvals, review isolation, continuation and packaging with evidence/version/OS dimensions.
- **FR-006** (US2): System MUST keep all existing agents installed; promote capability rows through evidence, not brand assumptions, and publish a scoped migration note superseding old Codex-tier-2 wording for this program only.
- **FR-007** (US3): System MUST define lite, standard and high-assurance profiles for artifact breadth, reviewer diversity and operational feedback; preserve receipt truthfulness, owner authorization and required checks in every profile.
- **FR-008** (US3): System MUST align QA tooling descriptions/body, source-validation semantics, commit/review sequencing and precise owner authorization scope without blanket unsupported exceptions.
- **FR-009** (US3): System MUST add portable trigger/behavior evals that assert artifacts and executable effects rather than Claude-specific tool names; keep paid live evals opt-in and separate from deterministic CI.

### Key Entities

- **WorkflowProfile**: artifact breadth, required checks, review diversity and policy version.
- **CapabilityRow**: AGENT_IDS key, capability, tier, version/OS/surface, evidence and status.
- **InstallationSource**: shared skill/native plugin origin, digest, precedence and conflicts.
- **ArtifactApplicability**: project shape, artifact path, applicable/N/A and recorded reason.

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
