# Remote implementation handoff

Prepared 2026-11-17. Planning branch: `docs/codex-parity-plan`, base `origin/develop` at 0a61fc4546491d6696fa46a1f2595958f4903ed0. Canonical remote: https://github.com/lucasvmigotto/ai-gent. Publication and exact plan commit are reported by the producing session; verify with `git ls-remote --heads origin docs/codex-parity-plan` rather than trusting a self-referential commit hash inside this file.

## User intent and standing constraints

The owner wants all agreed parity/accounting work specified completely before implementation. Latest tier instruction: “This feature should work in most of all Code Agents, following the idea Tier 1 (Claude + Codex) and others in Tiers 2 and 3”. Include actual multi-model/provider developer/reviewer usage, every attempt/repair/coordination run and mechanically enforced completion receipts. Another agent works at home on 3.0.0; do not collide with or silently expand that release.

Development update 2026-11-18: 046 US1 is implemented on `feat/task-usage-ledger`, with seven tasks checked and the feature In progress. The registration journal is not a token collector or acceptance gate. See specs/046-task-usage-ledger/evidence/registration-slice.md. No runtime skills/plugins/hooks/shared references were edited by this planning branch. Before changing those files, present exact proposed changes and obtain approval under AGENTS.md. Do not merge/promote/tag/release from this planning authorization.

## Resume on another machine

```sh
git fetch origin
git switch -c docs/codex-parity-plan --track origin/docs/codex-parity-plan
python3 docs/product/agent-parity-plan/validate_plan.py
```

If the local branch already exists, switch to it and use `git pull --ff-only`; do not overwrite dirty work. The office `.worktrees/` folder is ignored, but every required artifact is committed on the remote branch. No dependency on local scratch paths or the private checkpoint.

Then fetch current develop and inspect changes since the pinned base. Reserve feature IDs 046–054 or rename consistently if the home agent has already occupied them. Follow roadmap.md dependencies and file ownership. Implementation branches start from current develop after the planning commit is available through reviewed integration; alternatively create a temporary build branch from the plan branch while explicitly retaining the develop landing target. Do not cherry-pick all docs blindly over a changed index/constitution.

## Prior work to reconcile

`origin/fix/delegate-dependencies` at 7e92b5c712bb7dbe96cd6bcd481f64e642aae004 contains the detailed audit and limited scheduler/completion fix. It was pushed and its full local check passed. It is not merged into this planning branch. 048 needs that code landed or equivalent verified behavior; 050 goes substantially beyond it. Keep the distinction in review.

## First build step

Fetch `origin/feat/task-usage-ledger` for the implementation slice. Review its contract and evidence first, then continue 046 US2 (T005/T014/T015/T016) with failing usage-normalization fixtures. T010 independent review remains pending; do not claim the feature converged. Set SPECIFY_FEATURE=046-task-usage-ledger and SPECIFY_FEATURE_DIRECTORY=specs/046-task-usage-ledger. Read the prospective constitution and program measurement/architecture decisions first. Use the installed Spec Kit skill files; implement and converge belong to the build phase.

Spec Kit infrastructure/templates/constitution are committed; generated local .claude skills are not shipped. A different host can install its matching Spec Kit integration without changing these feature contracts. Do not rely on a checkout-local .specify/feature.json pointer; set the feature explicitly.

## Validation and tracker scope

Run the plan validator and recorded prerequisites, then full scripts/check.sh before each commit. Runtime-specific suites in quickstarts are future targets and do not exist yet. Live/paid host evals were not run by planning. New tracker items/board mutations were not created; program.json contains a ready feature/story batch for separately confirmed repo:sync. No invented issue IDs.

The final producing-session response records check results, commit and push state. Once fetched, use git log/status to establish actual state; this handoff deliberately avoids asserting unobserved future CI results.

Local task graphs deliberately differ from numerical file order because layer tasks are appended after story pointers. Use `task-graph.json`, not a naive T001→T022 loop. `.specify` core scripts/templates are sufficient for prerequisite checks; integration-state manifests are checkout-local.

Validation command evidence and limitations are recorded in `validation.md` after the producing session finishes checks.
