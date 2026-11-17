# Feature Specification: Source, inventory and status evidence integrity

**Feature Branch**: `docs/codex-parity-plan`
**Created**: 2026-11-17
**Status**: Planned
**Input**: Owner requests full code-agent parity planning, mandatory task usage, multi-model/provider builder–reviewer metrics, and equal Claude + Codex Tier 1 with portable Tiers 2/3.
**Program**: [scope and decisions](../../docs/product/agent-parity-plan/README.md).
**Dependencies**: 46.

## User Scenarios & Testing

Tests are required for every story. Stories are independently testable on the foundational contract; later stories add capabilities without making earlier acceptance require a live paid model call.

### User Story 1 — Describe source checks truthfully (Priority: P1)

A reviewer knows whether a source merely contains terms or actually supports a claim.

**Why this priority**: Establish the minimum trustworthy workflow before adding projections or convenience.

**Independent Test**: `lexical_negation_and_source_bounds` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given a source says a package is not archived and a claim says archived, when lexical verification runs, then the output is lexical_match/needs_review rather than confirmed.
2. Given inaccessible, oversized, binary, symlink-escaped or non-regular source input, when checked, then bounded validation reports unsupported/error and does not read beyond scope.

### User Story 2 — Generate inventories and status counts (Priority: P1)

A maintainer sees current feature/task/skill counts with reproducible evidence.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `inventory_drift_and_status_provenance` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given specs 044/045 exist but are absent from the index, when inventory check runs, then it reports exact missing rows and does not invent implementation statuses.
2. Given task flags disagree with receipts or an owner release waiver, when status renders, then it shows the discrepancy and preserves the waiver provenance.

### User Story 3 — Publish a reproducible capability proof (Priority: P2)

A contributor can reproduce parity claims and see untested combinations.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `probe_manifest_and_honest_claims` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given a compatibility report, when the underlying fixtures/probes run, then every qualified cell points to versioned evidence and every unsupported cell names a limit.
2. Given a heuristic review score or stale probe, when a release claim is assembled, then it cannot be promoted to verified solely by its label.

### Edge Cases

Missing/null fields, unsupported versions, conflicting duplicates, wrong repository/task scope, interrupted writes, two concurrent actors and stale artifact/policy evidence must produce explicit diagnostics. The specific examples above and qa.md are required regression cases.

## Requirements

### Functional Requirements

- **FR-001** (US1): System MUST replace lexical confirmed claims with explicitly lexical outcomes; preserve claim/source/digest and require distinct human or semantic review evidence before reporting supported/refuted.
- **FR-002** (US1): System MUST accept only bounded regular files or explicitly allowed fetched sources, prevent traversal/symlink escape, and record source time/version/digest and unavailable status.
- **FR-003** (US1): System MUST version verifier output and migrate consumers without silently treating old lexical confirmed as semantic certification.
- **FR-004** (US2): System MUST generate or verify feature/skill/plugin/task inventories from their canonical files; detect missing/duplicate IDs, broken links and stale aggregate counts without rewriting owner decisions.
- **FR-005** (US2): System MUST distinguish observed, inferred, assumed, planned, implemented and verified claims with source revision/test evidence; a passing prose or structural check does not prove live host behavior.
- **FR-006** (US2): System MUST document amendments to existing 035/041 contracts and cross-links to 033/036/037/039/040 without silently changing the 3.0.0 required gate set or historical waiver status.
- **FR-007** (US3): System MUST store sanitized probe manifests and deterministic fixture results with tool version, OS, command, exit status, revision and scope; separate live interactive probes from fake executables.
- **FR-008** (US3): System MUST allow evidence-based overrides for heuristic findings with actor/reason/source and expiry, while preserving the original signal and blocking unresolved critical gate failures.
- **FR-009** (US3): System MUST publish a follow-on migration and operations feedback checklist covering rollback, escaped defects and source/capability drift; do not claim billing certification from local hashes.

### Key Entities

- **SourceObservation**: claim ID, source digest/time, lexical terms and lexical outcome.
- **SemanticAssessment**: claim/source link, evaluator identity, reasoning reference and supported/refuted/uncertain.
- **InventorySnapshot**: revision, canonical entries, calculated counts and discrepancies.
- **ProbeManifest**: tool/version/OS/surface, revision, command, result, evidence digest and scope.
- **HeuristicOverride**: signal ID, actor, reason, supporting evidence, scope and expiry.

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
