# Remote implementation handoff

Prepared 2026-11-17. Planning branch: `docs/codex-parity-plan`, base `origin/develop` at 0a61fc4546491d6696fa46a1f2595958f4903ed0. Canonical remote: https://github.com/lucasvmigotto/ai-gent. Publication and exact plan commit are reported by the producing session; verify with `git ls-remote --heads origin docs/codex-parity-plan` rather than trusting a self-referential commit hash inside this file.

## User intent and standing constraints

The owner wants all agreed parity/accounting work specified completely before implementation. Latest tier instruction: “This feature should work in most of all Code Agents, following the idea Tier 1 (Claude + Codex) and others in Tiers 2 and 3”. Include actual multi-model/provider developer/reviewer usage, every attempt/repair/coordination run and mechanically enforced completion receipts. Another agent works at home on 3.0.0; do not collide with or silently expand that release.

Development update 2026-11-18: PR [#462](https://github.com/lucasvmigotto/ai-gent/pull/462) merged the plan and 046 US1 into `develop` at `e0c5ddb09b085ec4a3b366854553c4d4e5ce2b57`. The owner explicitly waived the macOS ARM job cancelled before steps twice and its skipped packaging dependency; those cells remain unqualified. Executed checks passed. No independent GitHub self-approval was submitted.

Ingestion PR [#463](https://github.com/lucasvmigotto/ai-gent/pull/463) merged into `develop` at `87d0861b4323656d24b0d8b05476ba45016f258e` after fresh independent review and all current-head CI passed, including all six Rust targets and packaging; no waiver was needed. Recovery implementation is published on `feat/usage-recovery` at `e0d2d9164a63e74c74021299d1dabb3a9a51c842`, based on that develop tree. Transfer continuation is published as `d924de0` on `feat/usage-transfer`. Current continuation branch: `perf/usage-ledger-indexes`, stacked on that published slice while integration remains separate. The old ingestion remote branch was deleted after merge. The ingestion slice adds 046 US2 ingestion, conservative normalization/deduplication and preregistered shared/unattributed execution scopes. Twelve of 22 feature tasks are complete; feature remains In progress. See `specs/046-task-usage-ledger/evidence/ingestion-slice.md`. Runtime skills/plugins/hooks/shared references remain unchanged. Before editing those, present the proposal and obtain approval under AGENTS.md. Prior PR waiver applies only to #462; it is not a waiver for future CI.

## Resume on another machine

```sh
git fetch origin
git switch -c perf/usage-ledger-indexes --track origin/perf/usage-ledger-indexes
python3 docs/product/agent-parity-plan/validate_plan.py
```

If the local branch already exists, switch to it and use `git pull --ff-only`; do not overwrite dirty work. The office `.worktrees/` folder is ignored, but every required artifact is committed on the remote branch. No dependency on local scratch paths or the private checkpoint.

Then fetch current develop and inspect changes since the pinned base. Reserve feature IDs 046–054 or rename consistently if the home agent has already occupied them. Follow roadmap.md dependencies and file ownership. Implementation branches start from current develop after the planning commit is available through reviewed integration; alternatively create a temporary build branch from the plan branch while explicitly retaining the develop landing target. Do not cherry-pick all docs blindly over a changed index/constitution.

## Prior work to reconcile

`origin/fix/delegate-dependencies` at 7e92b5c712bb7dbe96cd6bcd481f64e642aae004 contains the detailed audit and limited scheduler/completion fix. It was pushed and its full local check passed. It is not merged into this planning branch. 048 needs that code landed or equivalent verified behavior; 050 goes substantially beyond it. Keep the distinction in review.

## First build step

Fetch `origin/perf/usage-ledger-indexes` for the next slice; the old registration remote branch was deleted after merge. Review the ingestion contract/evidence and run tests before resuming. The bounded recovery/snapshot slice adds explicit torn-tail recovery, preserved originals, immutable local snapshots and synthetic failure checkpoints. See `specs/046-task-usage-ledger/evidence/recovery-slice.md`. This continuation adds bounded versioned export/import with read-only dry-run validation and an isolated imported archive; imported evidence does not alter current measured totals. See `contracts/evidence-transfer.md` and `evidence/transfer-slice.md` under feature 046. Next: finish remaining 046 US3 (T007/T017–T019/T008), especially segmented journals, disposable projections, actual crash/failure and scale qualification. The measured replay baseline and correction sequence are in `specs/046-task-usage-ledger/evidence/scale-baseline.md`; the current 64 MiB format cannot fit the planned 100k-event workload. Synthetic boundary failures and local snapshots do not qualify full US3 or 050 accepted completion. Independent review/convergence T010 and QA T020–T022 remain open. Then continue the dependency-ordered program; actual host collectors are 047 and enforced completion is 050. Set SPECIFY_FEATURE=046-task-usage-ledger and SPECIFY_FEATURE_DIRECTORY=specs/046-task-usage-ledger.

Spec Kit infrastructure/templates/constitution are committed; generated local .claude skills are not shipped. A different host can install its matching Spec Kit integration without changing these feature contracts. Do not rely on a checkout-local .specify/feature.json pointer; set the feature explicitly.

## Validation and tracker scope

Run the plan validator and recorded prerequisites, then full scripts/check.sh before each commit. Runtime-specific suites in quickstarts are future targets and do not exist yet. Live/paid host evals were not run by planning. New tracker items/board mutations were not created; program.json contains a ready feature/story batch for separately confirmed repo:sync. No invented issue IDs.

The final producing-session response records check results, commit and push state. Once fetched, use git log/status to establish actual state; this handoff deliberately avoids asserting unobserved future CI results.

Local task graphs deliberately differ from numerical file order because layer tasks are appended after story pointers. Use `task-graph.json`, not a naive T001→T022 loop. `.specify` core scripts/templates are sufficient for prerequisite checks; integration-state manifests are checkout-local.

Validation command evidence and limitations are recorded in `validation.md` after the producing session finishes checks.

Final continuation validation and publication are recorded in `specs/046-task-usage-ledger/evidence/transfer-slice.md`. Process-termination boundaries and limits are in `specs/046-task-usage-ledger/evidence/process-crash-slice.md`. Do not infer power-loss or full-scale qualification from these tests.


Performance continuation: contribution membership/cutoff indexing and single-pass in-memory receipt-prefix hashing are implemented without journal/receipt format changes. Matched debug replay samples and compatibility/review evidence are in `specs/046-task-usage-ledger/evidence/indexed-replay-slice.md`. The 30k sample improved from 17.180 to 9.296 seconds; do not extrapolate a 100k/reference-runner qualification. Next: segmented journals preserving old raw prefixes, bounded-memory decode and disposable verified projections/indexes, including imported archive request guards. Repeated whole-history normalization/cloning across operations still remains. The producing-session response records the pushed commit and final gate.


Current performance publication gate: two full checks failed in the unchanged database-backup fake-engine spawn fixture, despite isolated and workspace-selected target passes. The current work branch is reviewable but **must not be treated as full-gate green or merged** while that remains unresolved. See `specs/046-task-usage-ledger/evidence/indexed-replay-slice.md` for exact evidence and limits. No test policy was waived.


Backup fixture continuation: `test/backup-native-fixture` is stacked on published performance commit `89c49c7`. It replaces freshly written executable shell stand-ins with a prebuilt Rust example, without production changes. See `specs/046-task-usage-ledger/evidence/backup-native-fixture.md` for test prerequisites and limits. The historical failure cause remains unknown; earlier failed checks remain recorded. Its full uninstrumented `scripts/check.sh --quiet` passed on 2026-11-25; this descendant is locally green. Earlier failures remain historical evidence, not a waived gate. No remote CI or merge is claimed.
