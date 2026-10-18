Reconstructed by project:introspec on 2026-10-18 from 384c771

# Feature Specification: LLM-readable output

**Feature Branch**: `docs/introspec-site-runners` (reconstructed; the code already exists on `main`)

**Created**: 2026-10-18

**Status**: Draft

**Input**: Reverse-engineered from `docs/site/scripts/generate-llms.ts`, run as the last step of `bun run build`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An agent maps the toolkit from llms.txt (Priority: P1)

An agent fetching over HTTP, without JavaScript, reads `llms.txt`, follows a link to a page's `.md` and answers a toolkit question from plain Markdown.

**Why this priority**: the brief's journey 3 and architecture driver 4.

**Independent Test**: `bun run build`, then read `dist/llms.txt` and one linked `dist/docs/en-US/skills/<id>.md`.

**Acceptance Scenarios**:

1. **Given** a build, **When** `generate-llms.ts` runs, **Then** `dist/llms.txt` lists a "Use" section (install and every skill), a "Reference" section (flow, contribute, reference) and an "Optional" section (changelog), each linking `<VITE_SITE_URL>/docs/en-US/<slug>.md` with a one-sentence description [OBSERVED: docs/site/scripts/generate-llms.ts:166-201].
2. **Given** a skill, **When** its Markdown page is written, **Then** it holds the description, both invoke forms, the "generated from" line, the SKILL.md body and the source link [OBSERVED: generate-llms.ts:22-45].
3. **Given** the built `index.html`, **When** a crawler reads it, **Then** `<link rel="alternate" type="text/plain" href=".../llms.txt">` points at the entry file [OBSERVED: docs/site/index.html:13; tests/meta.test.ts:27].

---

### User Story 2 - The whole corpus in one file (Priority: P2)

An agent downloads `llms-full.txt`: every page's Markdown joined by `---` separators.

**Acceptance Scenarios**:

1. **Given** a build, **When** the script runs, **Then** `dist/llms-full.txt` and `dist/llms-full-en-US.txt` hold every page body in order [OBSERVED: generate-llms.ts:204-229].

---

### User Story 3 - Locale-keyed output (Priority: P3)

Each locale in `LOCALES` gets `llms-<locale>.txt`, `llms-full-<locale>.txt` and `docs/<locale>/…`; the unsuffixed files are en-US.

**Acceptance Scenarios**:

1. **Given** `LOCALES = ["en-US"]`, **When** the script runs, **Then** `llms-en-US.txt` and `llms.txt` carry the same list [OBSERVED: generate-llms.ts:204-224].

### Edge Cases

- The Markdown reference page lists 6 contracts; the HTML page lists 9 (secrets, agent roles and tracker sync are missing) [OBSERVED: generate-llms.ts:148-153 vs docs/site/src/pages.tsx:304-345].
- The Markdown install page omits the HTML page's Verify, Updates and first-skill sections; the Markdown contribute page omits Layout and the versions table [OBSERVED: generate-llms.ts:54-137 vs pages.tsx:206-299].
- A skill's one-line description is the text before the first `. `; descriptions with no period are used whole [OBSERVED: generate-llms.ts:27].
- Nothing tests the LLM output: no unit test reads `dist/llms*.txt` and CI only asserts the Open Graph files [OBSERVED: docs/site/tests/; .github/workflows/docs-ci.yml:67-84].

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The build MUST emit `llms.txt`, `llms-full.txt` and one `.md` per page and locale from the same content the pages render.
- **FR-002**: `.md`, `llms*.txt` MUST be served `no-cache` with `text/markdown` / `text/plain` (feature 005) [OBSERVED: .github/workflows/docs-ci.yml:250-263].
- **FR-003**: Links in `llms.txt` MUST be absolute, from `VITE_SITE_URL`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every page reachable in the HTML site has a Markdown twin with the same sections. *Not met today* (see Edge Cases).

## Assumptions

- [ASSUMPTION: the Markdown twins are meant to carry the same content as the HTML pages; the gaps above are drift, not intent.]
