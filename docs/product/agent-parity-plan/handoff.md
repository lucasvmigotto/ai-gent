# Remote implementation handoff

Prepared 2026-11-17. Planning branch: `docs/codex-parity-plan`, base `origin/develop` at 0a61fc4546491d6696fa46a1f2595958f4903ed0. Canonical remote: https://github.com/lucasvmigotto/ai-gent. Publication and exact plan commit are reported by the producing session; verify with `git ls-remote --heads origin docs/codex-parity-plan` rather than trusting a self-referential commit hash inside this file.

## User intent and standing constraints

The owner wants all agreed parity/accounting work specified completely before implementation. Latest tier instruction: “This feature should work in most of all Code Agents, following the idea Tier 1 (Claude + Codex) and others in Tiers 2 and 3”. Include actual multi-model/provider developer/reviewer usage, every attempt/repair/coordination run and mechanically enforced completion receipts. Another agent works at home on 3.0.0; do not collide with or silently expand that release.

Development update 2026-11-18: PR [#462](https://github.com/lucasvmigotto/ai-gent/pull/462) merged the plan and 046 US1 into `develop` at `e0c5ddb09b085ec4a3b366854553c4d4e5ce2b57`. The owner explicitly waived the macOS ARM job cancelled before steps twice and its skipped packaging dependency; those cells remain unqualified. Executed checks passed. No independent GitHub self-approval was submitted.

Current implementation branch: `feat/usage-normalization`, based on that develop commit. It adds 046 US2 ingestion, conservative normalization/deduplication and preregistered shared/unattributed execution scopes. Twelve of 22 feature tasks are complete; feature remains In progress. See `specs/046-task-usage-ledger/evidence/ingestion-slice.md`. Runtime skills/plugins/hooks/shared references remain unchanged. Before editing those, present the proposal and obtain approval under AGENTS.md. Prior PR waiver applies only to #462; it is not a waiver for future CI.

## Resume on another machine

```sh
git fetch origin
git switch -c feat/usage-normalization --track origin/feat/usage-normalization
python3 docs/product/agent-parity-plan/validate_plan.py
```

If the local branch already exists, switch to it and use `git pull --ff-only`; do not overwrite dirty work. The office `.worktrees/` folder is ignored, but every required artifact is committed on the remote branch. No dependency on local scratch paths or the private checkpoint.

Then fetch current develop and inspect changes since the pinned base. Reserve feature IDs 046–054 or rename consistently if the home agent has already occupied them. Follow roadmap.md dependencies and file ownership. Implementation branches start from current develop after the planning commit is available through reviewed integration; alternatively create a temporary build branch from the plan branch while explicitly retaining the develop landing target. Do not cherry-pick all docs blindly over a changed index/constitution.

## Prior work to reconcile

`origin/fix/delegate-dependencies` at 7e92b5c712bb7dbe96cd6bcd481f64e642aae004 contains the detailed audit and limited scheduler/completion fix. It was pushed and its full local check passed. It is not merged into this planning branch. 048 needs that code landed or equivalent verified behavior; 050 goes substantially beyond it. Keep the distinction in review.

## First build step

Fetch `origin/feat/usage-normalization` for the next slice; the old registration remote branch was deleted after merge. Review the ingestion contract/evidence and run tests before resuming. Next: 046 US3 (T007/T017–T019/T008), including fault injection, scale qualification and atomic receipt envelopes. Existing durable append behavior does not qualify those acceptance criteria. Independent review/convergence T010 and QA T020–T022 remain open. Then continue the dependency-ordered program; actual host collectors are 047 and enforced completion is 050. Set SPECIFY_FEATURE=046-task-usage-ledger and SPECIFY_FEATURE_DIRECTORY=specs/046-task-usage-ledger.

Spec Kit infrastructure/templates/constitution are committed; generated local .claude skills are not shipped. A different host can install its matching Spec Kit integration without changing these feature contracts. Do not rely on a checkout-local .specify/feature.json pointer; set the feature explicitly.

## Validation and tracker scope

Run the plan validator and recorded prerequisites, then full scripts/check.sh before each commit. Runtime-specific suites in quickstarts are future targets and do not exist yet. Live/paid host evals were not run by planning. New tracker items/board mutations were not created; program.json contains a ready feature/story batch for separately confirmed repo:sync. No invented issue IDs.

The final producing-session response records check results, commit and push state. Once fetched, use git log/status to establish actual state; this handoff deliberately avoids asserting unobserved future CI results.

Local task graphs deliberately differ from numerical file order because layer tasks are appended after story pointers. Use `task-graph.json`, not a naive T001→T022 loop. `.specify` core scripts/templates are sufficient for prerequisite checks; integration-state manifests are checkout-local.

Validation command evidence and limitations are recorded in `validation.md` after the producing session finishes checks.
