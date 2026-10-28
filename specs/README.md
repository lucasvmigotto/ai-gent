Reconstructed by project:introspec on 2026-10-18 from 384c771

# Features — ai-gent docs site and runners

Status: Draft

The parts of ai-gent that are software with behavior: the documentation site (`docs/site/`) and the scripts that run things (installer, runners, planners). The skills themselves (`plugins/*/skills/**/SKILL.md` and their references) are their own specification and are not reconstructed here. Every feature was reverse-engineered from code at commit 384c771; statuses come from evidence only (see `docs/product/introspec.md`).

Sides: the docs site has only a Frontend; the runners are command-line tools with no UI, so their status is in the Backend column (the code path) and Frontend is N/A. QA is the end-to-end evidence: a suite that drives the real thing and runs in CI.

| # | Feature | Scope | Priority | Depends on | Frontend | Backend | QA | Notes |
|---|---|---|---|---|---|---|---|---|
| 001 | docs-site-shell | layout, generated content, theme, locale contract, version footer, metadata | P1 | — | Implemented | N/A | — | unit + axe tests; skip link under hash routing unverified |
| 002 | docs-site-skill-reference | landing finder, search, index, skill pages, not-found | P1 | 001 | Implemented | N/A | — | e2e smoke covers US1 and US4 only |
| 003 | docs-site-guides | install, flow, contribute, reference, changelog pages | P2 | 001 | Implemented | N/A | — | flow page tested; install/contribute/reference/changelog untested; troubleshooting Planned |
| 004 | docs-site-llm-output | llms.txt, llms-full.txt, per-page Markdown | P1 | 001, 002, 003 | Implemented | N/A | — | no tests; Markdown twins drift from the HTML pages |
| 005 | docs-site-delivery | Docs CI checks, e2e smoke, R2 deploy, OG image | P1 | 001 | N/A | Implemented | — | CI green 2026-10-17; actions pinned by tag only |
| 006 | installer | install.sh clone/update/pin, setup.sh per-agent install, opencode config | P1 | — | N/A | Implemented | — | test-install.sh (70 checks) runs in CI and passed; interactive selector untested |
| 007 | pass-store | shared passstore module: check, has, read, write | P1 | — | N/A | Implemented | — | tested only through dbrun and reporun |
| 008 | db-runner | dbrun: profiles, classification, read-only queries, plans/apply/restore, scripts, log, secrets | P1 | 007 | N/A | Verified | — | 51 unit tests run in CI; the real-engine suite runs weekly in CI (scheduled.yml, green 2026-10-18) |
| 009 | repo-runner | reporun: detect, cli, api, batch, chain | P1 | 007 | N/A | Implemented | — | 27 test methods pass; write misclassification found |
| 010 | spec-sync | specsync: plan, record, features | P2 | 009 | N/A | Implemented | — | parses this file |
| 011 | worktrees | wt.py: plan, add, list, remove | P3 | — | N/A | Implemented | — | parses this file |
| 012 | checkpoint-band | project mod: checkpoint holds band, ask before push/merge/tag/release under a hold | P1 | — | Implemented | N/A | — | first Claude Code mod; `claude plugin test` in check.sh |
| 013 | pipeline-pane | project:status as a live pane: artifacts, feature statuses, open markers, next stage | P2 | 012 | Planned | N/A | — | mod in `project`; read-only, $.fs plus git log |
| 014 | token-status | status line and toasts: cards in flight, projected vs used, budget verdict | P3 | 012 | Planned | Planned | — | mod in `agents`; Backend is `token_report.py budget --json` |
| 015 | guard-ux | richer deny/ask presentation for the git, db, secrets, repo and devcontainer guards | P2 | 012 | Planned | Planned | — | guard.sh stays the decider; importing rules.mjs rejected (research.md R3) |
| 016 | secret-mask | mask pasted credentials at prompt.submit, command.run and skill.prompt | P1 | 012 | N/A | Planned | — | mod in `secrets`; patterns shared with sessions.py |
| 017 | repo-hygiene | band for unpulled dev/main and release commits, session-start toasts, /release-preview | P2 | 012 | Planned | N/A | — | mods in `git` and `devcontainer`; never fetches |

Verified needs suites covering every story, running and passing in CI; for tooling without a UI the integration suites stand in for e2e journeys (pipeline rule 6). 008 is Verified: its unit tests run on every pull request and the real-engine suite weekly. The closest others are 006 (all stories but the interactive selector) and 002 (two of four stories).

013–017 are forward specs (not reconstructed): optional Claude Code mods — hooks modules in the plugin they serve — that surface or enforce what the skills, scripts and guard hooks already guarantee, and are never part of a contract. For them, Frontend is what the person sees (pane, band, status line, toast, dialog, command output) and Backend is hook logic that gates or rewrites (tool calls, prompts) plus any script change. All depend on 012 (the checkpoint mod, specified and built on its own branch), which settles the module layout, `check.sh` wiring, versioning and install they reuse.

## Contract

`contracts/openapi.yaml`: **N/A** — there is no HTTP API. The site is static and the runners are local CLIs; their contracts are `specs/NNN-*/contracts/routes.md` and `cli.md`.

## Not reconstructed

- `scripts/check.sh`, `scripts/release.py`, `scripts/update-check.sh`, `scripts/opencode-config.py`, `opencode/guard/` and the `plugins/*/hooks/guard.sh` guard hooks — outside the requested scope; listed as entry points in `docs/product/introspec.md`.
- `plugins/compact/` and `plugins/project/scripts/repo_state.py` — skill helpers, outside scope.
