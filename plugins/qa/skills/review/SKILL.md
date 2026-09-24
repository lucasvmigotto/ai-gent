---
name: review
description: Audit an existing test suite for gaps against stories and risky code, weak assertions, over-mocking, flakiness, slowness and pyramid shape, using mutation testing with coverage as a signal only; ranked findings with fixes, and improved tests (never production code) on request. Use for "review our tests", "why is CI flaky", "mutation testing".
---

# Test-suite review

## Before starting

1. Read `../../references/qa-quality.md` (anti-patterns and quality
   floor) and `../../references/pipeline.md`.
2. Read what defines "enough": `specs/*/spec.md` and `qa.md`
   (traceability), `test-strategy.md` if present, `domain-model.md`
   invariants, `backend.md` authz matrices, `ui.md` state matrices.
3. Inventory the suites: layers, frameworks, counts, durations, how they
   run locally and in CI.

## Analysis

1. **Traceability gaps** — acceptance scenarios, invariants, authz rows
   and UI states with no test; tests that trace to nothing (dead or
   redundant).
2. **Mutation testing** on the highest-risk modules (domain logic,
   authorization, money/time calculations, state machines) — surviving
   mutants point at assertions that don't check what matters. Run it
   scoped (changed files or a module), not on the whole codebase at
   once.
3. **Coverage** (branch, not line) to find untested risky branches — as
   a signal for where to look, not a score to hit.
4. **Assertion quality** — tests asserting only status codes, no
   assertion at all, snapshot-only, or internal calls instead of
   behavior.
5. **Mocking** — mocks of code the project owns, mocks that make the
   test pass whatever the implementation does, integration tests that
   mock the database.
6. **Flakiness** — rerun the suite N times (e.g. `--repeat-each`,
   `--count`, or a loop) and read CI history; find the cause (timing,
   shared data, order, external calls, time zones, randomness without
   seeds).
7. **Speed** — the slowest tests and setup costs; tests at a higher
   layer than needed.
8. **Data** — shared fixtures, real personal data, order dependence,
   missing cleanup.
9. **Shape** — proportion per layer against the strategy; an inverted
   pyramid is a finding.

## Findings

| Field | Content |
|---|---|
| Severity | High (a real defect could ship undetected) · Medium · Low |
| Location | test file:line, or the untested code/story |
| Evidence | surviving mutant, rerun result, missing trace row, snippet |
| Fix | the concrete test to add or change |
| Effort | S · M · L |

Write `docs/product/test-review-<date>.md`: summary, metrics (counts,
durations, flake rate, mutation score per module), findings ranked,
quick wins, plan.

## Fixing (only when the user agrees)

Improve **tests only** — add missing tests, strengthen assertions,
replace sleeps, isolate data, split slow tests to the right layer. If a
new test exposes a production defect, stop and report it to the owning
build stage instead of changing production code. Re-run mutation testing
and the flakiness loop afterwards to show the improvement.

## Handoff

Top gaps, metrics before/after, defects found, and the plan. Commits
follow `git:workflow`.
