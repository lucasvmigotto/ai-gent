# Feature Specification: Docs site for 3.x (docs-site v2)

**Feature Branch**: `docs/specs-rust-tools` (specification only; built after the Rust pieces)

**Created**: 2026-11-01

**Status**: Draft

**Input**: `docs/product/rust-decisions.md` (D1, D2, D3, D8, D15, D18: "a docs-site v2 spec (3.x Install page, tier compatibility matrix, search; no full redesign) is part of the planning set, built after the Rust pieces"), `docs/product/release-3-notes.md`, `docs/site/README.md`, `docs/site/src/` (read: `App.tsx`, `pages.tsx` `InstallPage` line 215, `content/sections.ts` `installSections` line 41, `content.ts` `searchSkills` line 124, `components.tsx` `SearchBox` line 382, `i18n.tsx`), `docs/site/scripts/{generate-content.mjs, generate-llms.ts, llms-pages.ts}`, `docs/site/tests/`, `.github/workflows/docs-ci.yml`, and specs 001-005 (the baseline).

**Depends on**: 001-005 (Implemented), 006 (installer), and the facts of 032-039 (the binary, `ai-gent update`, `doctor`). Built last; does not block any Rust piece.

## Context and ground rules

- **No full redesign** (D18). The shell, theme, landing finder, skill pages, flow, contribute, reference and changelog pages stay. This spec adds one page (compatibility), rewrites the Install page, improves search, and decides two infrastructure questions. It follows `docs/site/README.md`.
- The current site: Bun, React 19, TypeScript, Vite 8, Tailwind, **HashRouter** (`react-router` 8.5.1 in `package.json`), Biome, vitest + jest-axe + happy-dom, a Selenium e2e smoke, deployed to Cloudflare R2 at `https://docs.lucasvmigotto.me/ai-gent/` by `.github/workflows/docs-ci.yml` (prefix-scoped S3 sync of `assets/`, `index.html`, `favicon.svg`, `og-image.png`, `llms*.txt` and the Markdown pages). `LOCALES` is `["en-US"]` only; the README says "English-only" while `i18n.tsx` supports more by design (a completeness test fails the build on a gap).
- The Install page today (`installSections`) shows the one-line install, the target-list form, a verify command, an updates paragraph, the first skill and troubleshooting. It does **not** show the package-install facts shipped in 2.26 (checksum before unpacking, `VERSION=` pin, `--rollback`, `--verify-attestation`, `--migrate`, moving from the git clone) which today live only in the README (lines 24-52).
- Content that is **generated from the repository** (never hand-edited): `src/content/generated.json` from every `SKILL.md` and `plugin.json` (`scripts/generate-content.mjs`), the changelog, version footer, `llms.txt`, `llms-full.txt` and a Markdown twin per page and locale (`scripts/generate-llms.ts`). New content in this spec follows the same rule: facts come from the repo, not from prose typed into the site.
- Terminology: the version is written `3.0.0` or `3.x`, with no `v` prefix (tags have none); the README line 52 still writes the version with a `v` prefix ("going away in ...") and `docs/site` will need the same wording check; the README is outside this spec's edit list but is listed under open question 6.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Install page for the package era (Priority: P1)

A new user opens the Install page and gets, per operating system, the command to install the package, how to pin a version, how to roll back, how to verify the download (checksum, and `gh attestation verify`), and how to move from an old git-clone install. After 3.0.0 the page no longer mentions git mode.

**Why this priority**: it is the first thing every user reads, and 2.26/3.0.0 change what is true.

**Independent Test**: vitest on `InstallPage` (sections present, commands exact), axe test, llms Markdown twin test, e2e step that the page renders and its code blocks are copyable.

**Acceptance Scenarios**:
1. **Given** Linux or macOS, **Then** the page shows `curl -fsSL https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.sh | sh` and says what it does in order: downloads the package and `SHA256SUMS`, verifies the checksum **before** unpacking, unpacks into `~/.local/share/ai-gent/<version>/`, points `current` at it keeping `previous`, runs `setup.sh` (README lines 24-26).
2. **Given** a pin, **Then** `VERSION=3.0.0 sh` is shown, with the note that a pinned install silences update notices until `VERSION=latest` (README line 30, D15).
3. **Given** a rollback, **Then** `sh ~/.local/share/ai-gent/current/install.sh --rollback` is shown (`current` and `previous` swap).
4. **Given** verification, **Then** the page explains that the checksum guards corruption (same host as the tarball) and the attestation guards a tampered asset, shows `--verify-attestation` (needs `gh`), and shows the manual `gh attestation verify <file> -R lucasvmigotto/ai-gent` form [from `install.sh` line 141: the repository slug is a variable `SLUG`; confirm its value at build time].
5. **Given** Windows, **Then** the page states what is true on the day (see FR-005 and open question 1): not a promise.
6. **Given** the binary, **Then** a section "After install" shows `ai-gent doctor` and `ai-gent update --check` (D15: notify-only; `ai-gent update` installs through the same verified path; `--rollback` goes back; `AI_GENT_NO_UPDATE_CHECK` disables the notice).
7. **Given** a missing binary, **Then** the troubleshooting entry says what D8 says: the tool fails with the install command, there is no Python fallback.

### User Story 2 - Tier compatibility matrix (Priority: P1)

A user of Codex, Cursor or another agent sees what they get: a table with a row per feature and a column per tier (Claude Code; Codex; other agents), each cell one of `yes`, `instructions only`, `no`, `partial` with a short reason.

**Why this priority**: D2 introduces tiers in force at 3.0.0; users must see the policy.

**Independent Test**: unit test that the table is generated from one data source and that every agent in `setup.sh` `AGENT_IDS` (claude, opencode, codex, cursor, antigravity, copilot, amp, kiro, windsurf) appears under exactly one tier; axe test on the table (header cells, scope); llms twin.

**Acceptance Scenarios**:
1. **Given** the matrix, **Then** rows cover at least: skills install (`<plugin>-<skill>` entries), guard hooks (enforced), the OpenCode guard plugin, mods (bands, panes, status line), token tracking (`ai-gent tokens`), worktree tool, repo/spec/compact tools, installer, update notice, native Windows.
2. **Given** a Tier 3 agent, **Then** the page says best-effort and frozen, out of the required CI, nothing removed in 3.x (D2), and links the list of agents.
3. **Given** a cell whose truth cannot be checked from the repo, **Then** it says "unverified" rather than a guess (see open question 3 about Codex).

### User Story 3 - Better search (Priority: P2)

Search finds the right skill and the right page faster: matches ranked by where they hit (command and name above description), tolerates a small typo or a prefix, covers pages and reference headings (Install, Flow, Compatibility) as well as skills, and is fully keyboard operable.

**Why this priority**: the landing finder is the main navigation; today it only ranks by id after an all-terms substring match (`searchSkills`, `content.ts` lines 124-142).

**Independent Test**: unit tests on a pure `search(query)` function with a table of queries and expected first results; axe test; e2e step typing in the landing search.

**Acceptance Scenarios**:
1. **Given** the query `commit`, **Then** `git:workflow` ranks above a skill that only mentions commits in its description.
2. **Given** the query `instal`, **Then** the Install page is a result (prefix match).
3. **Given** the query `wt` or `worktree`, **Then** `agents:parallel` and the Compatibility row appear.
4. **Given** results, **Then** arrow keys move through them, Enter opens, Escape clears; the live region announces the result count (`t.search.results(count)` exists).
5. **Given** no network, **Then** search works (index generated at build time, no external service).

### User Story 4 - Real URLs and static HTML (Priority: P3, gated)

If the research and the host decision support it, the site moves from HashRouter to BrowserRouter with a prerendered HTML file per route, so links are plain paths and the first paint has content.

**Independent Test**: build output contains `use/install/index.html` etc.; e2e opens a deep link directly; a link checker over `dist/`.

**Acceptance Scenarios**: see FR-009 to FR-011. This story is built only if the gate in the plan passes (host supports it, prerender approach chosen, base path `/ai-gent/` handled).

### User Story 5 - Know who hosts the docs and why (Priority: P2)

The owner decides where the site lives (R2 or GitHub Pages) with both options listed and a recommendation; the Install page's trust statement matches the decision.

**Independent Test**: `contracts/hosting-decision.md` exists with both options and the owner's sign-off field; if the host changes, `docs-ci.yml` passes its own guards.

## Requirements *(mandatory)*

- **FR-001**: The Install page content is generated from sources in the repo where they exist: the commands from `install.sh`'s header (`--rollback`, `--verify-attestation`, `--migrate`, `VERSION=`, `AI_GENT_RELEASE_BASE`), the supported agents from `setup.sh` `AGENT_IDS`, the package facts from `shared/releasing.md`. Prose lives in `i18n.tsx` `Messages.install`; commands live in `content/sections.ts` (as today).
- **FR-002**: The page order: Choose how (package default), Per operating system, Pin a version, Verify the download (checksum, attestation), Update and roll back, After install (`ai-gent doctor`), Move from the git clone (until git mode is removed, then deleted), First skill, Troubleshooting.
- **FR-003**: Every command shown is copyable (the existing code block component) and exact; a test extracts each fenced command from the Markdown twin and checks it appears in the README or `install.sh` header (drift guard).
- **FR-004**: After the 3.0.0 cutover the page and the README no longer document `AI_GENT_MODE=git`, `AI_GENT_BRANCH`, `AI_GENT_REPO`, `AI_GENT_DIR` (`docs/product/release-3-notes.md`, "What 3.0.0 would change"); contributors are pointed to the repository checkout and `./setup.sh`.
- **FR-005**: Per-OS: Linux and macOS (x86_64, aarch64) use the one-liner (needs `curl`, `tar`, `bash`, `sha256sum` or `shasum`; no git, no sudo, README line 24). Windows: the page states the supported route on release day: either a native installer (if 3.0.0 ships one) or WSL/Git Bash with the same POSIX script. Which is true is open question 1; the page must not claim native support that is not shipped (D3 is a goal, D14 may leave binaries unsigned: SmartScreen and Gatekeeper behaviour must be described honestly if unsigned).
- **FR-006**: The compatibility matrix has a single data source `shared/tiers.json` (rows, tier columns, cell state, reason, source file), read by `generate-content.mjs`; `scripts/consistency.py` fails if an `AGENT_IDS` agent is missing from a tier or if a cell lacks a reason. The README's tier table (when 3.0.0 adds it) is generated from or checked against the same file.
- **FR-007**: Search: a build-time index (`src/content/search-index.json`, generated) of skills (id, command, name, plugin, description, triggers) and static pages (title, headings); a pure scoring function with field weights (command and name > id > plugin > headings > description), prefix and one-edit tolerance for terms of 4+ characters, all terms must hit (as today). No new runtime dependency unless the measured bundle cost is under 5 kB gzipped [ASSUMPTION: threshold chosen here, owner may change it].
- **FR-008**: Search UI: combobox/listbox semantics with `aria-activedescendant`, arrow/Enter/Escape, the `/` shortcut kept, result count announced in a polite live region, focus visible, works at 200% zoom and 320 px width (WCAG 2.2 AA obligations from `docs/product/ux-vision.md` and the existing axe tests).
- **FR-009**: BrowserRouter + prerender (gated): React Router's built-in prerender exists only in Framework Mode, not in Declarative Mode with `BrowserRouter` plus plain Vite, which is what the site uses [RESEARCHED: S2]. So the options are (a) migrate the site to Framework Mode with `ssr: false` and `prerender` (large: routes file, root module, build tool), or (b) keep Vite and add a small build script that server-renders each route to `dist/<route>/index.html` [ASSUMPTION: feasible with `react-dom/server` and a static router from `react-router`; not verified for react-router 8.5.1], or (c) stay on HashRouter. This spec recommends (c) for 3.0.0 and a time-boxed spike of (b) afterwards.
- **FR-010**: Whichever router is used, every static route has a Markdown twin and a place in `llms.txt` (already true: `staticPages`), so non-JS readers and LLMs do not depend on prerendering.
- **FR-011**: A deep link must work on the chosen host: R2 serves objects by key and has no single-page-app fallback by itself, and the deploy job uploads only a fixed set of files [OBSERVED: `docs-ci.yml` lines 245-275], so BrowserRouter on R2 needs one HTML object per route (prerender) or an edge rule outside this repository [ASSUMPTION: R2 custom-domain behaviour and any edge rule not researched]; React Router's own doc says the host must serve the fallback for paths that would otherwise 404 [RESEARCHED: S2].
- **FR-012**: Hosting decision document `contracts/hosting-decision.md` (both options, trade-offs, recommendation, owner sign-off); no workflow change before sign-off.
- **FR-013**: i18n: every new string is a key of `Messages` in `src/i18n.tsx`; the completeness test passes; if more locales exist by then, each gets the new keys (the test fails otherwise); command text is not translated.
- **FR-014**: Accessibility: axe tests for the new Install page, the Compatibility page and the search component; tables have `th scope`; no colour-only state in the matrix (text plus symbol); skip link and landmarks unchanged (001 flagged "skip link under hash routing unverified": the e2e adds that check).
- **FR-015**: e2e (Selenium smoke, containerized Chrome only, never a host driver): add steps for the Install page heading and a copy button, the Compatibility table, and a search query that lands on a result with the keyboard.
- **FR-016**: llms output: `staticPages` gains `compatibility`; the Install twin carries the new sections; `tests/llms.test.tsx` already asserts "every section of the page is in its Markdown" and "a page for every static route".
- **FR-017**: A CLI reference page (the `ai-gent` subcommands, flags, exit codes) is generated from the binary itself (`ai-gent docs --json` or the clap definitions), not typed (open question 4). It lands with 3.0.0 if the binary exposes the data; otherwise it is out of this spec.
- **FR-018**: Release: docs changes bump `docs/site/package.json` automatically through `scripts/release.py`; nothing is bumped by hand. The site ships in the same release as the binary it describes, or after, never before (the Install page must not describe a release that is not published).
- **FR-019**: The deploy keeps its fitness functions: prefix-scoped S3 targets only (`R2_PREFIX`), hashed assets first and never deleted, no-cache entry files. A Pages move replaces them with the Pages guards.

## Success Criteria *(mandatory)*

- **SC-001**: `bun run lint`, `typecheck`, `test`, `build` pass; the new tests cover US1-US3 and every axe test passes with zero violations.
- **SC-002**: The Markdown twin of the Install page contains the install, pin, rollback and verify commands, and the drift guard (FR-003) passes.
- **SC-003**: The matrix lists all nine agents of `AGENT_IDS` once each, with no cell lacking a reason; `consistency.py` enforces it.
- **SC-004**: A search table test of at least 15 queries has the expected first result for each; typing `instal` finds Install.
- **SC-005**: e2e smoke passes in CI against the built site, including keyboard search.
- **SC-006**: The hosting decision is signed or explicitly pending; the Install page's trust text matches it.
- **SC-007**: no release version is written with a `v` prefix under `docs/site/src` or `docs/site/scripts` (the T051 grep).

## Edge Cases

- A pinned install shows no update notice; the page must say how to leave the pin (`VERSION=latest`).
- `gh` missing: `--verify-attestation` degrades to "checksum only" with a message (install.sh line 144); the page says so.
- A visitor on an unsupported OS or CPU: the page lists the targets from D3 (x86_64 and aarch64 for Linux, macOS, Windows).
- Search with an empty query shows nothing (as today); with only stop-words shows the "no results" state with a way out.
- Reduced motion, high contrast, forced colours: unchanged from 001; the new table must not rely on colour.
- A release that has no package yet: the Install page's version note is generated from the latest tag; the site is deployed only after the release (FR-018).
- Skill descriptions change: the search index regenerates on every build; no hand-kept list.

## Out of Scope

- A visual redesign, new themes, dark-mode work, a blog, analytics, comments.
- Translating the site (LOCALES stays as is; additive later).
- Versioned docs (one site for the latest release; the changelog page holds history).
- Replacing Tailwind, Vite or the test stack.
- Documentation for Tier 3 agents beyond the matrix.

## Open Questions

1. **Windows install route for 3.0.0**: `install.sh` is POSIX; D3 makes native Windows a goal but D16 keeps the bootstrap POSIX. Does 3.0.0 ship a PowerShell installer (`install.ps1`) or document WSL/Git Bash? Recommended: document WSL/Git Bash on the Install page for 3.0.0, state native Windows as "binary available, installer in progress" only if a Windows binary is actually published, and add `install.ps1` as its own spec.
2. **Hosting**: R2 or GitHub Pages (see `contracts/hosting-decision.md`). Recommended: move to GitHub Pages deployed by OIDC if the shared docs-hub bucket is not needed for other sites; otherwise keep R2 and publish `install.sh`'s checksum in the GitHub Release notes.
3. **Codex in the matrix**: D2 puts Codex in Tier 2, above the others, but this repository shows no mechanism that enforces guards for Codex (README line 124: "guard hooks are instructions only, not enforced" for Codex and the others) [OBSERVED]. What does Tier 2 add concretely at 3.0.0 (an enforced guard via the binary, a tested install, CI coverage)? Recommended: the matrix says exactly what is tested; Tier 2 means "in the required CI install test", nothing more, until the owner states otherwise.
4. **CLI reference generated from the binary**: does the binary expose `docs --json`? Recommended: yes, a hidden subcommand emitting clap metadata; 032 to confirm.
5. **BrowserRouter**: skip for 3.0.0 (recommended) and spike the custom prerender later, or do it now? Recommended: skip now.
6. **README wording**: README line 52 writes the version with a `v` prefix. Fix it to "3.0.0" in the README as part of the cutover edits (owner approval needed; outside this spec's file list).
7. **Search dependency**: allow a small library (for example a few kB gzipped) or require a hand-written scorer? Recommended: hand-written first; FR-007 sets the bundle threshold.

## Sources (web research, retrieved 2026-11-01)

| # | Claim | URL | Confidence | Label |
|---|---|---|---|---|
| S2 | React Router pre-rendering is available only in Framework Mode (Data and Declarative modes: not available); config `ssr: false` with `prerender`; each path yields an HTML file and a `.data` file; a SPA fallback file (`index.html` or `__spa-fallback.html`) must be served by the host for paths that would otherwise 404; the page does not mention `basename` | https://reactrouter.com/how-to/pre-rendering | Medium (vendor doc, current page, `basename` unaddressed) | [RESEARCHED: S2] |

Not researched, labelled where used: GitHub Pages and R2 hosting behaviours (custom headers, SPA fallback, bandwidth, OIDC deploy) are [KNOWN] or [ASSUMPTION] in `contracts/hosting-decision.md`. Source numbers S1-S4 are shared across specs 035-040.
