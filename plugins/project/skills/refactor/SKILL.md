---
name: refactor
description: Plan and drive a major refactor of an existing project — keep its core invariants, analyze code, architecture, data, operations and business workflows that could work better, record each business change for the user's approval, and migrate in incremental strangler-style slices. Use for "plan a major refactor", "redesign this legacy system", "modernize the workflows".
---

# Refactor — redesign with the core kept

A deep, whole-project analysis that ends in a target design and an
incremental migration to it. What makes it different from
`project:retrofit` at `major`: retrofit renews the platform and changes no
behavior, while refactor may **improve the business logic itself** — only
the core invariants are fixed, and every business change is a decision the
user makes explicitly.

## Role

Act as a principal engineer who has led several legacy modernizations:
skeptical of rewrites, precise about what must not change, and practical
about sequencing. Big-bang rewrites fail; each slice of this refactor
ships on its own and can be rolled back.

## Before starting

1. Read `../../references/pipeline.md` and `references/migration.md`
   (patterns for incremental migration and data moves).
2. Require `project:introspec`'s artifacts; run it first if they're
   missing — a refactor that doesn't know what the system does will
   break it. Also read `retrofit.md` if one exists.
3. Clarify (pipeline rule 2): the goals ranked (maintainability, cost,
   performance, a new capability the old design blocks, team change),
   the constraints (freeze windows, compliance, parallel feature work),
   appetite for downtime, and who may approve business changes.

## Process

1. **Baseline and safety net.** Same as `project:retrofit`'s steps 1–2:
   the project builds and passes in its container, and characterization
   tests pin current behavior on the critical paths. Here they pin the
   **core invariants** strictly and everything else as "current
   behavior", so a business change later is a visible, reviewed diff.
2. **Classify the rules.** From the domain model and the evidence, sort
   every rule into:
   - **Core invariants** — what must stay true whatever the design:
     money and balances, legal and regulatory rules, security and
     authorization guarantees, data integrity, published contracts
     external systems depend on. They are immutable here.
   - **Business policies and workflows** — how the business chooses to
     operate today: approval steps, limits, notification rules, the order
     of a flow, manual workarounds. Improvable, with approval.
   - **Accidental behavior** — bugs, dead code, duplicated rules that
     disagree, behavior nobody can explain. Each needs a verdict (fix,
     keep, or ask).
   The table (rule · class · evidence · owner) goes in `refactor.md`, and
   the user confirms the core list before anything else.
3. **Analyze.** Every angle, each finding with evidence and cost:
   - code: coupling and dependency cycles, hotspots (churn × complexity
     from git history), duplication, dead code, test quality;
   - architecture: run `project:architecture` in review mode for the
     as-is → to-be, its ADRs and evolution triggers;
   - data: the model against the domain, constraints the database doesn't
     enforce, integrity problems (with the `db` plugin installed,
     `db:inspect` and `db:investigate`), migrations and growth;
   - operations: deploy, observability, incidents, cost;
   - security: authentication, authorization, secrets, dependency health
     (a `project:retrofit` findings pass, if none exists);
   - **business**: workflows the evidence says hurt (manual steps,
     frequent support fixes, rules that contradict each other, dead
     options) and what a better policy would look like.
4. **Business change records.** Each proposed change to a business policy
   or accidental behavior becomes `docs/product/bcr/NNNN-<change>.md`:
   current behavior (with evidence), proposed behavior, why, who and what
   it affects (users, data, integrations, reports), how existing data
   migrates, and the tests that will prove it. Status `proposed` until
   the user decides — `accepted` or `rejected`. Nothing unapproved is
   built. Accepted BCRs update the brief and domain model (through their
   owners, or as `[UPSTREAM GAP]` notes) and become spec changes via
   `project:spec`, with the affected features back to Planned on the
   changed stories.
5. **Target and migration plan — `docs/product/refactor.md`.** The target
   design (from the architecture review), then the route to it in
   **slices**, each independently shippable and reversible:
   - the seam used (an HTTP route in front of the old system, a module
     interface, a message topic) and the pattern (strangler fig, branch by
     abstraction, parallel run — `migration.md`);
   - data moves: expand → migrate → contract, backfills with counts and
     checksums, dual writes only with reconciliation;
   - verification: characterization tests, the slice's new tests, and a
     parallel run or shadow traffic comparing old and new outputs where
     the risk is high;
   - cutover and rollback per slice; decommissioning of the old path.
   Order slices by value and risk: an early, low-risk slice proves the
   seam and the pipeline.
6. **Execute through the chain.** Each slice goes through the usual
   stages (`backend:spec` / `frontend:spec` for the changed features,
   then `backend:build` / `frontend:build`, `qa:e2e`), with the
   characterization tests guarding everything the slice didn't intend to
   change. Update `refactor.md` as slices land.

## Coverage checklist

- [ ] baseline green; characterization tests cover the critical paths
- [ ] every rule classified; the core-invariant list confirmed by the user
- [ ] findings across code, architecture, data, operations, security and business, each with evidence
- [ ] every business change is a BCR with a decision; nothing unapproved in the plan
- [ ] target design with ADRs from `project:architecture`
- [ ] slices independently shippable and reversible, each with seam, verification, cutover and rollback
- [ ] every data move has a backfill check and a contract step
- [ ] core invariants tested at every slice; parallel runs where risk is high
- [ ] the old paths' decommissioning is planned, not left for later

## Handoff

The core list, accepted and rejected BCRs, the target in three lines, the
slice plan with the first slice to build, the biggest risks, and the next
stage (usually `project:spec` for the first slice's features). Commits
follow `git:workflow`.
