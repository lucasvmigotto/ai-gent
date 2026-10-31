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
| 013 | pipeline-pane | project:status as a live pane: artifacts, feature statuses, open markers, next stage | P2 | 012 | Implemented | N/A | — | mod in `project`; `/pipeline-pane`; read-only, $.fs plus git log; parity-tested against specsync and wt; closing the pane is untested (the kit can't raise ui.close) |
| 014 | token-status | status line and toasts: cards in flight, projected vs used, budget verdict | P3 | 012 | Implemented | Implemented | — | mod in `agents`; `token_report.py budget --json`; the orchestrate ledger's State words are fixed; `/tokens --live` (story 5) is projected, not built |
| 015 | guard-ux | richer deny/ask presentation for the git, db, secrets, repo and devcontainer guards | P2 | 012 | Implemented | N/A | — | guard.sh stays the decider; importing rules.mjs rejected (research.md R3); copies, not symlinks (R4); asks tested by `decorate` only, the kit can't show their text |
| 016 | secret-mask | mask pasted credentials at prompt.submit, command.run and skill.prompt | P1 | 012 | N/A | Implemented | — | mod in `secrets`; patterns shared with sessions.py; `claude plugin test` 11 cases; checked live in a headless run |
| 017 | repo-hygiene | band for unpulled dev/main and release commits, session-start toasts, /release-preview | P2 | 012 | Implemented | N/A | — | mods in `git` and `devcontainer`; never fetches; SessionStart lines come from the stored hook row (classic.SessionStart returns {}) |
| 018 | token-burn-rate | burn rate and ETA in the token status line, toast when a cap's ETA is near | P2 | 014 | Implemented | Implemented | — | `token_report.py budget` gains `rate` and `caps[].eta_minutes`; mod only formats |
| 019 | card-cost-on-landing | toast with a card's used vs projected tokens when it becomes `landed` | P3 | 014 | Implemented | N/A | — | reads the ledger; writes nothing; hint in token-economy.md |
| 020 | card-timeline-pane | `/cards-pane`: ledger rows, observed state changes, spans and peak parallelism | P3 | 014, 019 | Implemented | Implemented | — | needs `token_report.py tasks --dry-run --json` (shared with 014 story 5) |
| 021 | handoff-nudge | toast when a usage cap runs low: checkpoint is N commits stale, run `project:checkpoint` | P2 | 012, 014 | Implemented | N/A | — | `session.compact` and `$.session.usage()` give the trigger; cap check optional |
| 022 | worktree-board | `/worktree-board` pane: one row per worktree, ahead/behind, dirty, PR and CI | P3 | 011 | Implemented | N/A | — | mod in `agents`; `gh` columns only when signed in |
| 023 | ci-watch-band | band with the pushed commit's CI runs, toast on failure | P2 | 009 | Implemented | N/A | — | mod in `repo`; through reporun; bounded back-off; no secrets read |
| 024 | guard-log | `/guards` and a pane listing recent guard denies and asks | P3 | 015 | Implemented | N/A | — | records the verb only, never the command; guard still decides. Not built: story 3 (`guard_log.py`, a cross-session summary); interactive R1/R2 probes on Claude Code 2.1.312 not run |
| 025 | container-disk-band | persistent band and `/containers` for leftovers and low disk | P3 | 017 | Implemented | N/A | — | reads `leftovers.sh`; never removes; by ID or label only |
| 026 | output-secret-warning | toast when a tool result holds a credential: treat as exposed, rotate, `pass insert` | P1 | 016 | Implemented | Implemented | — | adds `skipIn: result` to two patterns in `shared/secret-patterns.json`; redacts the stored result via `session.append` |
| 027 | next-stage-prompt | toast naming the next pipeline stage after an artifact is written | P3 | 013 | Implemented | N/A | — | reuses 013's `compute` and `recommend` |
| 028 | release-readiness-band | band with what `release.py --dry-run` would bump | P3 | 017 | Implemented | N/A | — | extends `/release-preview`; never releases |
| 029 | pipeline-pane-redesign | dashboard layout for the pipeline pane: summary tiles, artifacts table, aligned feature matrix, stage strip, glyphs with ASCII fallbacks, filters | P2 | 013 | Implemented | N/A | — | view-only amendment to 013; headless text unchanged; `S` column dropped; structured view model first. Not done: the compatibility table run and the manual runs; interactive R1/R2 probes on Claude Code 2.1.312 not run |
| 030 | band-priority | at most two bands above the prompt by declared priority, `/bands` for the rest | P2 | 012, 017 | Implemented | N/A | — | shared `band-priority.ts` copied into each hooks module; read by 023, 025, 028; cross-plugin visibility probed first |
| 031 | session-chrome | a Workspace line folding the quiet bands above the prompt, and a context meter (on by default) on the prompt's hint line | P2 | 021, 030 | Implemented | N/A | — | `Band.summary` in the shared `band-priority.ts`; agent activity, the auto-mode row restyle and meter colour are not possible (engine API limits); interactive R1/R2 probes on 2.1.312 not run |

Verified needs suites covering every story, running and passing in CI; for tooling without a UI the integration suites stand in for e2e journeys (pipeline rule 6). 008 is Verified: its unit tests run on every pull request and the real-engine suite weekly. The closest others are 006 (all stories but the interactive selector) and 002 (two of four stories).

013–017 are forward specs (not reconstructed): optional Claude Code mods — hooks modules in the plugin they serve — that surface or enforce what the skills, scripts and guard hooks already guarantee, and are never part of a contract. For them, Frontend is what the person sees (pane, band, status line, toast, dialog, command output) and Backend is hook logic that gates or rewrites (tool calls, prompts) plus any script change. All depend on 012 (the checkpoint mod, specified and built on its own branch), which settles the module layout, `check.sh` wiring, versioning and install they reuse.

018–031 are the second set of forward mod specs (written 2026-10-31, now Implemented on `dev`; 024 story 3 and the remaining 029 runs are not built): each is read-only and optional like 013–017, and each lists its open questions as `[NEEDS CLARIFICATION]`. Decisions of 2026-10-31 settled every open question (see each spec's Decisions section). Prerequisite changes land first, each on its own branch: `token_report.py tasks --dry-run --json` (020, 014 story 5), `leftovers.sh --json` (025), `release.py` and `shared/release-single.py` `--json` (028), and the `skipIn: result` pattern flag with regenerated `secret-data.ts` (026). Also decided 2026-10-31: bands above the prompt obey a priority cap of two (specified in 030); releases are batched from `dev` in themed promotions; the work is built with `agents:orchestrate` in parallel worktrees after the specs are reviewed and accepted. Suggested build order, by what they unblock: 014 story 5 (`/tokens --live`, with `tasks --dry-run --json`) → 018 → 019 → 020; 026 (the only P1) with its shared-pattern change; 021 and 023; 024 and 027; then 022, 025 and 028. One branch per spec, landed on `dev` one at a time.

## Contract

`contracts/openapi.yaml`: **N/A** — there is no HTTP API. The site is static and the runners are local CLIs; their contracts are `specs/NNN-*/contracts/routes.md` and `cli.md`.

## Not reconstructed

- `scripts/check.sh`, `scripts/release.py`, `scripts/update-check.sh`, `scripts/opencode-config.py`, `opencode/guard/` and the `plugins/*/hooks/guard.sh` guard hooks — outside the requested scope; listed as entry points in `docs/product/introspec.md`.
- `plugins/compact/` and `plugins/project/scripts/repo_state.py` — skill helpers, outside scope.
