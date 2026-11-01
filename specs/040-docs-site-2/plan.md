# Implementation Plan: Docs site for 3.x

**Branch**: `docs/specs-rust-tools` (spec) | **Date**: 2026-11-01 | **Spec**: `spec.md`

## Summary

Rewrite the Install page from repository facts, add a generated tier compatibility page, replace the substring search with a weighted build-time index and an accessible combobox, record the hosting decision, and gate a BrowserRouter + prerender spike behind that decision. No redesign. Built after the Rust pieces so the page describes what is shipped.

## Technical Context

**Stack (as is)**: Bun, React 19.3.1, TypeScript 7, Vite 8.3.5, Tailwind 4.3, `react-router` 8.5.1 (HashRouter), Biome 2.6.4, vitest 5 + happy-dom + jest-axe 11, selenium-webdriver (containerized Chrome). **Generated**: `src/content/generated.json` by `scripts/generate-content.mjs`; llms files by `scripts/generate-llms.ts` and `scripts/llms-pages.ts`.
**Commands** (run in `docs/site/`): `bun run lint | typecheck | test | build`, `bun run e2e` (needs the Selenium container, see README).
**New data files**: `shared/tiers.json` (source), `src/content/search-index.json` (generated, gitignored like `generated.json`).

## Files to change (verified paths)

| Path | Change |
|---|---|
| `docs/site/src/content/sections.ts` | `installSections` (line 41): new order and commands; new `compatSections` |
| `docs/site/src/i18n.tsx` | `Messages.install` (line 68) gains keys for OS, pin, verify, rollback, doctor, git-clone migration; new `compat` group; `search` group keys for combobox labels |
| `docs/site/src/pages.tsx` | `InstallPage` (line 215) unchanged shell; new `CompatibilityPage` |
| `docs/site/src/App.tsx` | route `/use/compatibility` (before the `*` route) |
| `docs/site/src/content.ts` | `searchSkills` (line 124) replaced by `search(query)` over the index; keep a `searchSkills` wrapper until callers move |
| `docs/site/src/components.tsx` | `SearchBox` (line 382): combobox semantics, arrow keys, live region |
| `docs/site/scripts/generate-content.mjs` | read `shared/tiers.json`, `setup.sh` `AGENT_IDS`, build the search index |
| `docs/site/scripts/llms-pages.ts` | `staticPages` gains `compatibility`; install twin updated automatically from `installSections` |
| `docs/site/tests/pages.test.tsx` | Install tests (commands present), Compatibility test, axe tests |
| `docs/site/tests/content.test.ts` | search table test, tiers completeness |
| `docs/site/tests/llms.test.tsx` | assertions already generic; add compatibility slug |
| `docs/site/tests/e2e/smoke.mjs` | steps from FR-015 |
| `shared/tiers.json` | new, with `scripts/consistency.py` check (AGENT_IDS covered, reasons present) |
| `docs/site/README.md` | document the new data files and checks |
| `.github/workflows/docs-ci.yml` | only after the hosting sign-off |

## Constraint check

| Constraint | Result |
|---|---|
| D18 no full redesign | one page added, one rewritten, one component upgraded |
| D1, D2, D3, D8, D15 | the Install page text is derived from them; Tier 3 wording from D2; Windows from open question 1 |
| `docs/site/README.md` rules | content generated, never hand-edited; copy in `i18n.tsx`; completeness test; prefix-scoped deploy; Selenium container only; OG image unchanged |
| AGENTS.md | propose first; run `scripts/check.sh`; docs version bumped by the release script, not by hand |
| Naming | `3.0.0` or `3.x`, no `v` prefix (SC-007) |
| `shared/research.md` | claims labelled; hosting claims marked KNOWN/ASSUMPTION |
| Web research budget | 4 searches and 4 page fetches across 035-040 in total; this spec used 1 fetch (S2) |

## Install page content plan (facts and where they come from)

| Fact | Source |
|---|---|
| package download, checksum before unpacking, layout `~/.local/share/ai-gent/<version>/`, `current`, `previous`, runs `setup.sh`, needs curl, tar, bash, sha256sum or shasum | `README.md` lines 24-26 |
| `VERSION=x.y.z` pin and `pinned` marker; `VERSION=latest` clears | `README.md` line 30, `docs/product/release-3-notes.md` |
| `--rollback` | `README.md` line 32 |
| `--verify-attestation`; the checksum vs attestation explanation | `README.md` lines 34 and 43; `install.sh` lines 9 and 141-144 |
| `AI_GENT_RELEASE_BASE` (mirrors, `file://`) | `README.md` line 41 |
| move from the git clone: `--migrate` | `README.md` lines 45-49 (removed from the page when git mode is removed in 3.0.0) |
| `--target` list of agents | `setup.sh` `AGENT_IDS`; `README.md` line 81 |
| update notice, `AI_GENT_NO_UPDATE_CHECK`, `ai-gent update --check` | `README.md` lines 54-60, D15 |
| `ai-gent doctor` | D9 (the subcommand's output to be quoted from the binary once 032 exists) |

## Search design

Index entry: `{kind: skill|page|heading, id, title, command, plugin, text, url}`. Score = sum of field weights for each term (command/name 10, id 8, plugin 4, heading 4, description 1), prefix match 0.7x, one-edit match for terms of 4+ characters 0.4x; every term must hit some field; ties by kind (skill first) then id. Pure function in `src/search.ts` so it is unit-tested without React. The combobox follows the WAI-ARIA authoring practice (`role=combobox`, `aria-expanded`, `aria-controls`, `aria-activedescendant`) [KNOWN]. Bundle impact measured with `vite build` size output before and after (FR-007).

## BrowserRouter + prerender gate

Gate to start: hosting sign-off (spec contract) AND a decision among (a) Framework Mode migration, (b) custom prerender script, (c) stay. Recommendation (c) now. If (b) is chosen later: a build step after `vite build` that renders each static route and writes `dist/<path>/index.html` plus a copy as the fallback; the `skill/:id` routes are enumerated from `generated.json`; `basename` set from `VITE_BASE_PATH` (the config already reads it, `vite.config.ts`); `docs-ci.yml` uploads the per-route files; the e2e opens a deep link cold. Acceptance is the cold deep link and a `dist/` link check, not the framework.

## Risks

| Risk | Mitigation |
|---|---|
| The page describes a release that does not exist yet | FR-018: deploy after the release; docs PR merges with the cutover PR |
| Tier claims that nobody tested | the matrix has a source and a verified/unverified flag per cell; consistency check |
| Search weights feel wrong | the query table test is the contract; adjust the table, not the code |
| Host move breaks links | keep `docs.lucasvmigotto.me/ai-gent/` stable (custom domain) or add redirects; owner sign-off first |
| Windows text overpromises | FR-005 and open question 1 |
| i18n completeness test blocks a second locale later | additive by design; keys land with their English copy |

## Phased approach

1. **Phase 1 (US1, P1)**: Install page rewrite with drift guard. Can start as soon as 2.26's package facts are final; the 3.0.0-only parts (binary, git-mode removal) land with the cutover.
2. **Phase 2 (US2, P1)**: `shared/tiers.json`, consistency check, Compatibility page.
3. **Phase 3 (US3, P2)**: search module, index, combobox, tests.
4. **Phase 4 (US5, P2)**: hosting decision document signed; workflow change only if the answer is Pages.
5. **Phase 5 (US4, P3, gated)**: router/prerender spike.
