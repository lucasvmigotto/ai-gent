# Feature Specification: Multi-provider and builder–reviewer usage reporting

**Feature Branch**: `docs/codex-parity-plan`
**Created**: 2026-11-17
**Status**: Planned
**Input**: Owner requests full code-agent parity planning, mandatory task usage, multi-model/provider builder–reviewer metrics, and equal Claude + Codex Tier 1 with portable Tiers 2/3.
**Program**: [scope and decisions](../../docs/product/agent-parity-plan/README.md).
**Dependencies**: 46, 47, 49, 50.

## User Scenarios & Testing

Tests are required for every story. Stories are independently testable on the foundational contract; later stories add capabilities without making earlier acceptance require a live paid model call.

### User Story 1 — Explain total task cost (Priority: P1)

A user sees all attempts and role contributions, including unsuccessful work and coordination overhead.

**Why this priority**: Establish the minimum trustworthy workflow before adding projections or convenience.

**Independent Test**: `all_attempts_and_counter_totals` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given a task with failed build, repair, two reviews and shared planning, when reported, then exclusive totals reconcile and shared overhead is separate with its allocation method.
2. Given cached/reasoning subset counters, missing telemetry or historical heuristics, when totals render, then there is no double counting, invented zero or misleading certified total.

### User Story 2 — Inspect builder–reviewer combinations (Priority: P1)

A maintainer compares observed execution combinations without pretending they were a controlled quality experiment.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `pairs_segments_and_cohorts` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given builder provider A/model X and reviewer provider B/model Y across repair rounds, when grouped, then pair counts, tokens, elapsed time and finding dispositions retain the exact run links.
2. Given a fallback changes model mid-run or one task is sampled twice, when grouped, then segment changes remain visible and paired totals do not duplicate shared runs.

### User Story 3 — Estimate money without claiming bills or quotas (Priority: P2)

A user can apply dated prices to usage and export a reproducible view.

**Why this priority**: Close a concrete bypass or compatibility gap without weakening the first story.

**Independent Test**: `pricing_legacy_and_exports` in the feature CLI integration suite, using synthetic evidence and fake host processes.

**Acceptance Scenarios**:

1. Given a model and matching price snapshot with cache categories, when cost is estimated, then currency, rates, date, rounding and pricing source are included.
2. Given unknown prices, subscription use or unmatched provider exports, when reported, then cost stays unknown/estimated and subscription quota is not derived from token counts.

### Edge Cases

Missing/null fields, unsupported versions, conflicting duplicates, wrong repository/task scope, interrupted writes, two concurrent actors and stale artifact/policy evidence must produce explicit diagnostics. The specific examples above and qa.md are required regression cases.

## Requirements

### Functional Requirements

- **FR-001** (US1): System MUST group by task/attempt/phase/run/segment, host, effective provider/model/effort and outcome; show successful and failed/cancelled work, completeness and attributable share.
- **FR-002** (US1): System MUST sum input-total plus output-total only across disjoint authoritative contributions; show cached/reasoning as breakdowns, preserve raw fields, and display unknown totals or explicit known lower bounds.
- **FR-003** (US1): System MUST support exclusive, shared and unattributed reporting; allocations include method/version/weights and cannot be labelled certified per-task measurements.
- **FR-004** (US2): System MUST record and report builder–reviewer provider/model/session pairs, review rounds, time-to-accepted and verified finding dispositions with task complexity/cohort labels where available.
- **FR-005** (US2): System MUST keep requested and actual model identities distinct, list unresolved aliases as unknown, and show sample size and missingness for every comparison.
- **FR-006** (US2): System MUST never turn raw finding counts into a quality leaderboard; explain that workload, reviewer behavior, cache warmth and repairs confound cross-model comparisons.
- **FR-007** (US3): System MUST keep raw usage, estimated API cost and subscription quota in distinct fields; version pricing snapshots, include currency and cache TTL/service-tier distinctions or report unsupported.
- **FR-008** (US3): System MUST preserve legacy token-tools JSON through an explicit compatibility mode; replace misleading billable terminology only in versioned output and label historical heuristic allocation.
- **FR-009** (US3): System MUST generate Markdown/JSON/CSV views from evidence without editing authoritative counters; redact source paths and exclude full transcripts from tracked exports.

### Key Entities

- **UsageAggregate**: grouping dimensions, disjoint contribution IDs, known totals and missing dimensions.
- **PairMetric**: builder segment, reviewer segment, artifact, rounds, timings and finding dispositions.
- **Allocation**: shared run, beneficiary keys, method/version/weights and non-certified label.
- **PriceSnapshot**: model/provider, service class, category rates, currency, effective dates and source.
- **LegacyProjection**: schema version, old field mapping, heuristic provenance and deprecation notice.

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
