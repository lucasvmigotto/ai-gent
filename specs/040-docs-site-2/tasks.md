# Tasks: Docs site for 3.x

Format: `- [ ] Tnnn [P] [USn] description. Files. Verify. Requirement.` Run commands in `docs/site/` unless noted. Executor Sonnet builder, Reviewer Opus inline. Every task ends with `bun run lint && bun run typecheck && bun run test` green; the full `scripts/check.sh` runs before each commit.

## Phase 1: Install page (US1)

- [ ] T001 [US1] Extract the install facts into a typed constant list (commands with their source line): `docs/site/src/content/install-facts.ts`. Files: that file. Verify: `bun run typecheck`. Requirement: FR-001.
- [ ] T002 [US1] Rewrite `installSections` with the new order (choose, per OS, pin, verify, update and roll back, after install, migrate from clone, first skill, troubleshooting). Files: `src/content/sections.ts`, `src/i18n.tsx` (`Messages.install` keys). Verify: `bun run test` (completeness test passes). Requirement: FR-002, FR-013.
- [ ] T003 [P] [US1] Drift guard: a test that every command in the Install Markdown twin appears in `README.md` or the header of `install.sh`. Files: `tests/install-drift.test.ts`. Verify: `bun run test -- install-drift`. Requirement: FR-003.
- [ ] T004 [P] [US1] Troubleshooting entries: missing binary (D8 text), `gh` missing for attestation, pinned install silent, unpack refused beside an old clone. Files: `src/i18n.tsx`, `tests/pages.test.tsx` (the "one entry per problem" test at line 52). Verify: `bun run test -- pages`. Requirement: FR-002.
- [ ] T005 [US1] Axe test and Markdown twin assertion for the new page. Files: `tests/pages.test.tsx`, `tests/llms.test.tsx`. Verify: `bun run test`. Requirement: FR-014, FR-016.
- [ ] T006 [US1] Windows section per owner's answer to open question 1 (until then, the WSL/Git Bash wording marked in the PR as pending). Files: `src/i18n.tsx`. Verify: `bun run test`. Requirement: FR-005. Blocked on the owner.
- [ ] T007 [US1] At the 3.0.0 cutover only: remove git mode text and the clone-migration section; update `README.md` line 52 wording with the owner's approval. Files: `src/content/sections.ts`, `src/i18n.tsx`, `README.md`. Verify: `git grep -n 'AI_GENT_MODE' docs/site README.md` empty. Requirement: FR-004.

## Phase 2: Compatibility matrix (US2)

- [ ] T010 [US2] Define `shared/tiers.json` (tiers, agents, rows, cell state, reason, source, verified flag) from `setup.sh` `AGENT_IDS` and README lines 114-124 and 383-396. Files: `shared/tiers.json`. Verify: `python3 -c "import json;json.load(open('shared/tiers.json'))"`. Requirement: FR-006.
- [ ] T011 [US2] `scripts/consistency.py` check: every `AGENT_IDS` agent in exactly one tier, every cell has a state and a reason; test in `scripts/test_consistency.py`. Files: those two. Verify: `python3 -m unittest scripts/test_consistency.py`; `python3 scripts/consistency.py`. Requirement: FR-006, SC-003.
- [ ] T012 [US2] Generate the matrix in `generate-content.mjs` and render `CompatibilityPage` (table with `th scope`, symbol plus text, no colour-only state); route `/use/compatibility`; nav link. Files: `scripts/generate-content.mjs`, `src/pages.tsx`, `src/App.tsx`, `src/content/sections.ts`, `src/i18n.tsx`. Verify: `bun run test`. Requirement: FR-006, FR-014.
- [ ] T013 [P] [US2] llms page and axe test for compatibility. Files: `scripts/llms-pages.ts`, `tests/pages.test.tsx`, `tests/llms.test.tsx`. Verify: `bun run test`. Requirement: FR-016.
- [ ] T014 [US2] Fill Codex/Tier 2 cells according to the owner's answer to open question 3; unverified cells say so. Files: `shared/tiers.json`. Verify: T011 check. Requirement: spec US2 scenario 3. Blocked on the owner.

## Phase 3: Search (US3)

- [ ] T020 [US3] Pure `search.ts` with weights, prefix and one-edit tolerance; a table test of at least 15 queries (`commit`, `instal`, `worktree`, `wt`, `token`, `secret`, `database`, `release`, `pr`, `spec`, `compact`, `orchestrate`, `checkpoint`, `tier`, `rollback`). Files: `src/search.ts`, `tests/search.test.ts`. Verify: `bun run test -- search`. Requirement: FR-007, SC-004.
- [ ] T021 [US3] Build-time index including static pages and headings. Files: `scripts/generate-content.mjs`, `.gitignore` entry for `src/content/search-index.json`. Verify: `bun run gen && test -s src/content/search-index.json`. Requirement: FR-007.
- [ ] T022 [US3] Combobox semantics and keyboard in `SearchBox`; keep `/`; live region count; keep `searchSkills` as a wrapper for existing callers. Files: `src/components.tsx`, `src/content.ts`, `src/i18n.tsx`. Verify: `bun run test` incl. axe and a keyboard test. Requirement: FR-008.
- [ ] T023 [P] [US3] Bundle check: record gzipped size before and after in the PR (`bun run build`). Files: none. Verify: increase under the FR-007 threshold or the owner accepts. Requirement: FR-007.
- [ ] T024 [US3] e2e steps (install heading, copy button, compatibility table, keyboard search, skip link on a deep link). Files: `tests/e2e/smoke.mjs`. Verify: `bun run build`, serve, `SELENIUM_URL=... bun run e2e` per `docs/site/README.md`. Requirement: FR-015, SC-005.

## Phase 4: Hosting (US5)

- [ ] T030 [US5] Owner decision on `contracts/hosting-decision.md`; record the sign-off in the file. Files: that file. Verify: sign-off fields filled. Requirement: FR-012. Blocked on the owner.
- [ ] T031 [US5] If Pages: new deploy job (Pages actions, OIDC), remove the R2 steps and the prefix guard, keep the no-surprise checks (build only after release); custom domain. Files: `.github/workflows/docs-ci.yml`, `docs/site/README.md`. Verify: a dry deploy to a branch environment; `scripts/check.sh`. Requirement: FR-019. Only after T030.
- [ ] T032 [US5] If R2: add the install.sh checksum to the release notes flow (`scripts/release.py` notes) and document key rotation. Files: `scripts/release.py` (propose first), `docs/site/README.md`. Verify: `scripts/release.py --dry-run`. Requirement: mitigations 1 and 4.

## Phase 5: Router spike (US4, gated on T030)

- [ ] T040 [US4] Spike: custom prerender script (option b) on a branch; record whether it works with `react-router` 8.5.1, base path `/ai-gent/`, and the chosen host's deep links; verdict in `specs/040-docs-site-2/spike-router.md`. Files: that file, `scripts/prerender.ts` (throwaway). Verify: a cold deep link opens and `dist/use/install/index.html` exists. Requirement: FR-009, FR-011.
- [ ] T041 [US4] If the verdict is yes: switch to `BrowserRouter` with `basename`, uploads of per-route HTML, link checker, e2e deep link. Files: `src/main.tsx`/`App.tsx`, `vite.config.ts`, workflow. Verify: `bun run build` and e2e. Requirement: FR-010, FR-011.

## Phase 6: Generated reference (optional)

- [ ] T050 CLI reference page generated from the binary (`ai-gent docs --json` or equivalent) when 032 exposes it. Files: `scripts/generate-content.mjs`, `src/pages.tsx`, `src/content/sections.ts`. Verify: `bun run test`; page lists every subcommand of 035-038. Requirement: FR-017. Blocked on 032.
- [ ] T051 SC-007 check and final pass: `git grep -n -E '\bv[0-9]+\.?[0-9]*\b' docs/site/src docs/site/scripts` shows no release written with a `v` prefix; `scripts/check.sh` green. Verify: both commands. Requirement: SC-007.
