Reconstructed by project:introspec on 2026-10-18 from 384c771

# Feature Specification: Docs site shell and generated content

**Feature Branch**: `docs/introspec-site-runners` (reconstructed; the code already exists on `main`)

**Created**: 2026-10-18

**Status**: Draft

**Input**: Reverse-engineered from `docs/site/` — the layout every page shares, the content generated from the repository, the theme, the locale contract and the version footer.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Content generated from the repository (Priority: P1)

A maintainer runs any site command; the site's content (plugins, skills, versions, README, changelog) is regenerated from the repository it documents, so no page can drift from the skill files.

**Why this priority**: every other docs feature renders this content; truthfulness to code is the brief's first architecture driver.

**Independent Test**: run `bun run gen` and compare `src/content/generated.json` with `plugins/*/skills/*/SKILL.md`.

**Acceptance Scenarios**:

1. **Given** a plugin directory with `.claude-plugin/plugin.json`, **When** `bun run gen` runs, **Then** each `skills/<s>/SKILL.md` becomes one skill entry with id `<plugin>-<skill>`, its frontmatter `name` and `description`, its body and a source URL [OBSERVED: docs/site/scripts/generate-content.mjs:54-94].
2. **Given** a directory under `plugins/` with no manifest, **When** content is generated, **Then** it is skipped [OBSERVED: docs/site/scripts/generate-content.mjs:61].
3. **Given** the build runs on a release tag, **When** source links are generated, **Then** they point at that tag, else at the commit, else at `main` [OBSERVED: docs/site/scripts/generate-content.mjs:23; tests/content.test.ts:37].
4. **Given** `dev`, `build`, `test` or `typecheck` is run, **When** it starts, **Then** `gen` runs first [OBSERVED: docs/site/package.json scripts].

---

### User Story 2 - Version footer (Priority: P2)

A reader sees which toolkit and docs versions a page was built from, each linked to that tree.

**Why this priority**: readers and agents need to know whether the page matches the version they installed.

**Independent Test**: `tests/version.test.ts` and `tests/app.test.tsx` "shows each version in the footer".

**Acceptance Scenarios**:

1. **Given** HEAD is exactly tag `2.8.0`, **When** the site builds, **Then** the footer reads `ai-gent@v2.8.0 · docs@v<package.json version>` linked to `/tree/2.8.0` [OBSERVED: docs/site/scripts/version.mjs:37-48; tests/version.test.ts:13].
2. **Given** HEAD is past the last tag, **When** the site builds, **Then** both versions carry `+<short sha>` and link to the commit [OBSERVED: docs/site/scripts/version.mjs:41-43; tests/version.test.ts:25].
3. **Given** no tag and no git, **When** the site builds, **Then** the footer reads `ai-gent@dev` with no link [OBSERVED: tests/version.test.ts:37].

---

### User Story 3 - Theme choice (Priority: P2)

A reader switches between dark and light themes from the footer; the choice persists across visits. Dark is the default.

**Why this priority**: the UX vision's dark-first direction; accessibility of contrast in both themes.

**Independent Test**: `tests/app.test.tsx` "switches the theme from the footer's icon buttons".

**Acceptance Scenarios**:

1. **Given** no stored choice, **When** the page loads, **Then** `data-theme="dark"` is set before render [OBSERVED: docs/site/src/main.tsx:6-9].
2. **Given** the reader presses the sun button, **When** the theme changes, **Then** it is stored under `ai-gent-docs-theme` and the button is `aria-pressed` [OBSERVED: docs/site/src/components.tsx:36-67].

---

### User Story 4 - Locale contract (Priority: P3)

A contributor can add a locale as a content task: every registered locale must be a complete `Messages` set, and the locale switcher appears only once a second locale exists.

**Why this priority**: the brief keeps the i18n architecture although only en-US ships.

**Independent Test**: `tests/i18n.test.tsx`.

**Acceptance Scenarios**:

1. **Given** `LOCALES = ["en-US"]`, **When** the top bar renders, **Then** no locale switcher is shown [OBSERVED: docs/site/src/i18n.tsx:9; docs/site/src/components.tsx:106].
2. **Given** a stored or browser-preferred locale, **When** the app starts, **Then** a supported one is chosen, else the first locale, and the choice persists under `ai-gent-docs-locale` [OBSERVED: docs/site/src/i18n.tsx:403-445; tests/i18n.test.tsx:37].
3. **Given** a locale missing a key, **When** tests run, **Then** the completeness test fails [OBSERVED: tests/i18n.test.tsx:28].

---

### User Story 5 - Shared layout, navigation and metadata (Priority: P1)

Every page has a skip link, a top bar with the primary sections (desktop links, a mobile menu), a footer with versions, license and theme, and HTML metadata for previews and LLM discovery.

**Why this priority**: the frame of every page and every link preview.

**Independent Test**: `tests/app.test.tsx`, `tests/meta.test.ts`, axe checks in `tests/pages.test.tsx`.

**Acceptance Scenarios**:

1. **Given** any route, **When** it renders, **Then** the top bar links Install, Skills, Flow, Contribute, Reference and Changelog, with a mobile menu toggled by an `aria-expanded` button [OBSERVED: docs/site/src/components.tsx:125-200].
2. **Given** the built `index.html`, **When** it is served, **Then** it advertises `og:image` (1200×630), a Twitter large card, a canonical URL and `llms.txt` as an alternate [OBSERVED: docs/site/index.html:11-39; tests/meta.test.ts:11-27].
3. **Given** the footer, **When** it renders, **Then** it shows GPL v3+ linked to `LICENSE` at the build's ref [OBSERVED: tests/app.test.tsx:22].

### Edge Cases

- The skip link is `href="#main"` inside a `HashRouter`: activating it changes the hash route to `main`, which matches no route [INFERRED: docs/site/src/components.tsx:376 + docs/site/src/App.tsx:21-31 → the skip link likely renders the not-found page instead of moving focus; confirm with a keyboard test in a browser].
- A `SKILL.md` frontmatter value spanning several lines is read only up to the first line; the generator's parser is line-based [OBSERVED: docs/site/scripts/generate-content.mjs:36-45]. Descriptions are single-line by repository rule, so this holds today.
- Personal skills under `skills/` (`skill-docs`) aren't in the site; only plugin skills are generated [OBSERVED: docs/site/scripts/generate-content.mjs:49].
- The copy button announces "Copied" even when the clipboard write failed [OBSERVED: docs/site/src/components.tsx:232-240].

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The build MUST generate `src/content/generated.json` from `plugins/*/.claude-plugin/plugin.json` and `plugins/*/skills/*/SKILL.md`, plus `README.md` and `CHANGELOG.md` [OBSERVED: docs/site/scripts/generate-content.mjs:96-109].
- **FR-002**: The generated file MUST NOT be hand-edited; it is gitignored [OBSERVED: CLAUDE.md "Docs site"].
- **FR-003**: The footer MUST show the toolkit and docs versions per US2.
- **FR-004**: The theme MUST default to dark and persist the reader's choice.
- **FR-005**: Every locale in `LOCALES` MUST be a complete `Messages` set; the switcher MUST be hidden with one locale.
- **FR-006**: The site MUST be served under a path prefix taken from `VITE_BASE_PATH` (default `/ai-gent/`) and use hash routing [OBSERVED: docs/site/vite.config.ts:7; docs/site/src/App.tsx:19].
- **FR-007**: Pages MUST pass axe checks (landing, flow, skill page) [OBSERVED: tests/pages.test.tsx:40,70,88].

### Key Entities

- **Plugin**, **Skill**, **Version** — see `data-model.md`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every plugin skill in the repository has exactly one generated entry with a unique id [OBSERVED: tests/content.test.ts:13-23].
- **SC-002**: Zero axe violations on the tested pages [OBSERVED: tests/pages.test.tsx].
- **SC-003**: The footer version equals `git describe` at build time.

## Assumptions

- [ASSUMPTION: the skip link's behavior under hash routing was never checked in a browser; the e2e smoke doesn't exercise it.]
- [ASSUMPTION: personal skills (`skills/`) are deliberately left out of the site.]
