---
name: strategy
description: Define a risk-based test strategy for the project and each feature — which layer tests what, risk scores, test data and environments, gates per pipeline stage, story-to-test traceability, flaky-test policy and the exit criteria for Verified — in docs/product/test-strategy.md and specs/NNN-*/qa.md. Use for "define the test strategy", "what should we test".
---

# Test strategy

## Before starting

1. Read `../../references/qa-quality.md` (role, division of labor,
   anti-patterns) and `../../references/pipeline.md`.
2. Read: the brief (NFRs, compliance, audiences), `architecture.md`
   (topology, capacity model, SLOs), the constitution, every feature's
   `spec.md` (stories, acceptance scenarios, success criteria), and —
   when present — `ui.md`, `backend.md`, `contracts/openapi.yaml`,
   `ux-vision.md` (key flows), `domain-model.md` (invariants, lifecycles),
   `delivery.md`, and any existing tests.
3. Ask only what changes the strategy: supported browsers/devices,
   environments available for testing (is there a staging?), who does
   UAT, release cadence, tolerance for pipeline duration.

## Process

1. **Risk per feature** — score impact (money, data, safety, reputation,
   legal) × likelihood (complexity, change frequency, integrations,
   novelty) → High / Medium / Low. Risk decides depth: High gets
   negative, concurrency and failure-mode coverage at every layer; Low
   gets the happy path plus validation.
2. **Layers** — for the project, state what each layer covers and who
   owns it (build stages vs. QA), with the tools:
   unit · component · integration (real dependencies via
   Testcontainers/`devcontainer:infra` images) · contract (against
   `contracts/openapi.yaml`) · e2e (cross-stack journeys, `qa:e2e`) ·
   accessibility (axe in component tests + e2e key flows) · visual
   (screenshot comparison for the design system, if worth it) ·
   performance (`qa:load`) · security (from `devsecops:supply-chain`) ·
   exploratory (charters for High-risk features) · UAT.
3. **Test data** — factories/builders per aggregate, seed data for e2e
   (via API or migrations, not UI), personas matching the dev identity
   provider (admin, regular, no-roles — `devcontainer:infra` §4),
   synthetic data only (no real personal data), reset strategy.
4. **Environments** — local (the devcontainer stack), CI (ephemeral
   services), preview/staging; what runs where; the limits of each
   (e.g. load numbers from a laptop are relative only).
5. **Gates per pipeline stage** — PR, main, pre-production, scheduled:
   which suites must pass, thresholds (e.g. zero axe violations, p95 from
   SLOs), max allowed flake rate. `devsecops:pipeline` implements them.
6. **Exit criteria for Verified** — per feature: its e2e journeys pass
   in CI, load thresholds met where it has load targets, no open
   High-severity defects, accessibility checks on its flows pass.
7. **Flaky-test policy** — detection (reruns in CI reporting, history),
   quarantine with owner and deadline, never a silent retry-to-green.
8. **Metrics** — escaped defects, flake rate, suite durations, mutation
   score on critical modules; not raw coverage as a target.

## Output

- `docs/product/test-strategy.md` — risks, layers and owners, tools,
  data, environments, gates, exit criteria, policies, metrics.
- `specs/NNN-<feature>/qa.md` — risk score; traceability table
  (user story / acceptance scenario / FR / SC → test layer → test name or
  planned test); e2e journeys (steps, personas, data, expected results
  including negative paths); load profile if the feature has load targets
  (arrival rate, mix, thresholds); exploratory charters for High risk;
  exit criteria.
- A `## QA` section appended to each `tasks.md` (Spec Kit task format)
  for the QA-owned work: e2e journeys, load scripts, charters, gate
  wiring — with checkpoints.

## Coverage checklist

- [ ] every feature has a risk score and a `qa.md`
- [ ] every acceptance scenario maps to a test at a named layer
- [ ] every authorization rule has a negative test planned
- [ ] every High-risk feature has failure-mode and concurrency tests and a charter
- [ ] NFRs with numbers (latency, throughput, availability) map to load or monitoring checks
- [ ] test data is synthetic and personas match the dev IdP
- [ ] gates defined per pipeline stage with thresholds
- [ ] exit criteria for Verified stated per feature

## Handoff

Risk summary, the layer map, gates for `devsecops:pipeline`, what build
stages must include, and next stages (`qa:e2e`, `qa:load` after builds).
Commits follow `git:workflow`.
