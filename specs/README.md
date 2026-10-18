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
| 008 | db-runner | dbrun: profiles, classification, read-only queries, plans/apply/restore, scripts, log, secrets | P1 | 007 | N/A | Implemented | — | 51 unit tests pass; real-engine suite manual, not run here |
| 009 | repo-runner | reporun: detect, cli, api, batch, chain | P1 | 007 | N/A | Implemented | — | 27 test methods pass; write misclassification found |
| 010 | spec-sync | specsync: plan, record, features | P2 | 009 | N/A | Implemented | — | parses this file |
| 011 | worktrees | wt.py: plan, add, list, remove | P3 | — | N/A | Implemented | — | parses this file |

No feature is Verified: Verified needs end-to-end suites covering every story, running and passing in CI. The closest are 006 (all stories but the interactive selector) and 002 (two of four stories).

## Contract

`contracts/openapi.yaml`: **N/A** — there is no HTTP API. The site is static and the runners are local CLIs; their contracts are `specs/NNN-*/contracts/routes.md` and `cli.md`.

## Not reconstructed

- `scripts/check.sh`, `scripts/release.py`, `scripts/update-check.sh`, `scripts/opencode-config.py`, `opencode/guard/` and the `plugins/*/hooks/guard.sh` guard hooks — outside the requested scope; listed as entry points in `docs/product/introspec.md`.
- `plugins/compact/` and `plugins/project/scripts/repo_state.py` — skill helpers, outside scope.
