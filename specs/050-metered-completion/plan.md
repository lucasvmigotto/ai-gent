# Implementation Plan: Required metered task completion

**Branch**: `docs/codex-parity-plan` | **Date**: 2026-11-17 | **Spec**: [spec.md](spec.md)
**Status**: Planned; specify → clarify → plan → tasks completed as planning artifacts, implement not started.

## Summary

Add a central completion verifier in ai-gent-core consumed by CLI and read models. Two-phase finish writes pending state, drains evidence without model calls, then atomically publishes the acceptance receipt. Use content/tree digests to avoid a receipt-containing commit self-reference; a later integration attestation links the actual landing commit. CI compares changed task fingerprints to a protected baseline and validates receipts before tracker writes.

## Technical Context

- Language/version: Rust edition 2024, workspace MSRV 1.92; locked dependencies in rust/Cargo.lock. Extend existing Python/sh only at current integration boundaries.
- Dependencies: current clap 4, serde 1, serde_json 1, thiserror 2; assert_cmd/insta/tempfile for tests. Do not upgrade the workspace as part of this feature.
- Storage: local journal/immutable receipt contracts from 046; projections disposable. Host-private sources remain outside Git.
- Platform/type: existing cross-platform CLI, Linux/macOS/Windows; each host surface is qualified separately. No HTTP/client-server seam or new frontend.
- Testing: Rust unit + named CLI integration suite `agent_050` and existing script fixtures; real-host probes are separate evidence.
- Proposed budgets: per record maximum 1 MiB, source file read maximum 64 MiB per pass, collector drain default 10 s/max 60 s, external helper default 30 s/max 120 s, lock wait max 5 s. These are design limits, not measured results.
- Qualification workload: 100k usage events and 16 concurrent writers, replay ≤10 s and peak RSS ≤256 MiB on a recorded 4-core/8 GiB reference runner. Establish the baseline before claiming the target passes; exclude host/model latency.

## Constitution Check

| Principle | Pre-research gate | Post-design evidence | Result at planning |
|---|---|---|---|
| I | Interface and migration defined | contracts/cli.md; data-model.md | Pass for plan |
| II | Tests per story | qa.md and tests-first Backend tasks | Pass for plan |
| III | No false completion/usage claim | spec.md FRs and shared completion contract | Pass for plan |
| IV | No host-exclusive core contract | Tier 1 and extension fixtures in qa.md | Pass for plan |
| V | Authorization/data bounds explicit | backend.md matrix and retention | Pass for plan |
| VI | Faults/recovery bounded | backend.md and negative acceptance cases | Pass for plan |
| VII | Work branch, release isolation | program roadmap and handoff | Pass for plan |

These passes evaluate design coverage only; implementation and live qualification remain unchecked.

## Project Structure

- Domain/adapter changes: `rust/crates/ai-gent-core/src/completion/` (a proposed module if absent; existing files remain where already implemented).
- CLI integration: `rust/crates/ai-gent/src/cli.rs` and `rust/crates/ai-gent/src/cmd/`.
- Contract/e2e fixtures: `rust/crates/ai-gent/tests/agent_050.rs` and `rust/crates/ai-gent/tests/fixtures/agent_050/`.
- Artifacts: spec.md, research.md, data-model.md, contracts/cli.md, quickstart.md, backend.md, qa.md, tasks.md, checklists/requirements.md and analysis.md in this directory.
- Cross-host instruction/packaging updates are owned by 053; coordinate changes to shared files through the program serialization matrix.

## Phases and Rollout

Phase 0 research resolves evidence semantics and records limitations in research.md. Phase 1 contracts/model freeze precedes failing story fixtures. Phase 2 implements story slices, then independent review and fault checks. Default rollout is observe, then warn, then strict at the 050 cutover. Each feature may ship inert/opt-in until dependencies qualify.

## Compatibility and Migration

Keep existing 035 token output and 041 delegation readers compatible through explicit v1 adapters. Unknown/new schema majors fail with a supported-version diagnostic. Preserve old artifacts, import as unverified, and regenerate projections; never backfill measured counters from estimates. See [migration plan](../../docs/product/agent-parity-plan/migration.md).

## Complexity Tracking

No constitutional violation is proposed. A local journal is chosen over adding a database/service dependency; role/provider and evidence dimensions are separate because one enum would erase relevant guarantees. Any later departure requires a recorded ADR and impacted contract tests.
