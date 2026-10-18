Reconstructed by project:introspec on 2026-10-18 from 384c771

# Feature Specification: Skill finder and skill reference

**Feature Branch**: `docs/introspec-site-runners` (reconstructed; the code already exists on `main`)

**Created**: 2026-10-18

**Status**: Draft

**Input**: Reverse-engineered from `docs/site/src/pages.tsx` (LandingPage, SkillsPage, SkillPage, NotFoundPage), `docs/site/src/components.tsx` (SearchBox, SkillIndex, SkillRow) and `docs/site/src/content.ts`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Open a skill's page and copy its invoke command (Priority: P1)

A toolkit user opens `#/use/skills/<plugin>-<skill>` and gets the skill's description, how to invoke it in Claude Code and in the other agents, its trigger phrases, the full SKILL.md body, its source link and the plugin's other skills.

**Why this priority**: the UX vision's first principle — "the skill page is the product".

**Independent Test**: `tests/pages.test.tsx` "renders a skill page with its invoke command"; the e2e smoke opens `#/use/skills/qa-load`.

**Acceptance Scenarios**:

1. **Given** skill `qa-load`, **When** its page opens, **Then** it shows `/qa:load` for Claude Code and `/qa-load` for opencode and Cursor (`$qa-load` in Codex) in a copyable block [OBSERVED: docs/site/src/pages.tsx:157-160; docs/site/tests/e2e/smoke.mjs:38-50].
2. **Given** a description with quoted phrases, **When** the page renders, **Then** up to six of them are listed as "when to use" [OBSERVED: docs/site/src/content.ts:86-90; pages.tsx:162-171; tests/content.test.ts:55].
3. **Given** the page, **When** it renders, **Then** it shows "generated from" with the plugin version and toolkit version, a link to the source SKILL.md at the build's ref, the body as Markdown and the plugin's other skills [OBSERVED: pages.tsx:173-200].
4. **Given** an unknown id, **When** the page opens, **Then** a not-found message with a link back to the index is shown [OBSERVED: pages.tsx:131-144].

---

### User Story 2 - Find a skill by the words of a task (Priority: P1)

A user types task words into the search field (landing or skills page) and gets the matching skills; `/` focuses the field from anywhere outside a text input.

**Why this priority**: the landing page's hero is the search field.

**Independent Test**: `tests/content.test.ts` "finds a skill by the words of a task" / "by its Claude Code command" / "returns nothing for an empty query".

**Acceptance Scenarios**:

1. **Given** the query "load test", **When** it is typed, **Then** only skills whose command, id, name, plugin or description contain every term are listed, ranked by id [OBSERVED: docs/site/src/content.ts:66-83].
2. **Given** a query with no match, **When** results update, **Then** "No skill matches “…”." and "Try fewer words, or browse the full index." are shown and the count is announced in a live region [OBSERVED: docs/site/src/components.tsx:324-333; docs/site/src/i18n.tsx:173-174].
3. **Given** focus outside any input, **When** the user presses `/`, **Then** the search field gets focus [OBSERVED: components.tsx:294-310].

---

### User Story 3 - Browse the index by plugin (Priority: P2)

A user browses every skill grouped by plugin, with each plugin's version, on the landing page or on `#/use/skills` with a plugin filter.

**Why this priority**: the alternative path to search in the vision's "find a skill" flow.

**Independent Test**: `tests/pages.test.tsx` "shows the landing hero and the skill index".

**Acceptance Scenarios**:

1. **Given** the landing page, **When** it renders, **Then** it shows the title, counts ("N skills · M plugins · en-US"), the search field, Install and Flow buttons, and the full index [OBSERVED: pages.tsx:50-92].
2. **Given** the skills page, **When** the user picks a plugin in the filter, **Then** only that plugin's group is shown [OBSERVED: pages.tsx:94-127; components.tsx:345-369].

---

### User Story 4 - Not-found route (Priority: P3)

Any unknown route renders a not-found page with a link to the landing page, never a blank page.

**Independent Test**: e2e smoke `#/does-not-exist` [OBSERVED: docs/site/tests/e2e/smoke.mjs:52-56].

**Acceptance Scenarios**:

1. **Given** `#/does-not-exist`, **When** it opens, **Then** the not-found heading renders [OBSERVED: pages.tsx:616-628].

### Edge Cases

- Search is over the description, not the SKILL.md body; the brief says "titles and content" [OBSERVED: content.ts:73] — drift, see `docs/product/introspec.md`.
- The empty state doesn't name the closest plugins or link the index, as the UX vision's flow 1 asks [OBSERVED: components.tsx:328-333].
- `Esc` doesn't clear or close search (the vision's keyboard path) [OBSERVED: no `Escape` handler in docs/site/src].
- The skill page leads with `/plugin:skill`; the vision's wireframe shows `[Copy /qa-load]` [OBSERVED: pages.tsx:157-160 vs docs/product/ux-vision.md:166].

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: One page per plugin skill at `#/use/skills/<plugin>-<skill>` MUST exist.
- **FR-002**: A skill page MUST show the Claude Code command `/<plugin>:<skill>` and the other agents' name `<plugin>-<skill>`.
- **FR-003**: Search MUST match all terms (AND), case-insensitive, over command, id, name, plugin and description; an empty query returns nothing.
- **FR-004**: `/` MUST focus search unless the user is typing in an input, textarea, select or editable element.
- **FR-005**: The index MUST group skills by plugin with the plugin's version.

### Key Entities

- **Skill**, **Plugin** — `../001-docs-site-shell/data-model.md`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every plugin skill reachable from the index in one click.
- **SC-002**: Zero axe violations on landing and skill pages [OBSERVED: tests/pages.test.tsx:40,88].

## Assumptions

- [ASSUMPTION: ranking by id among equal matches is intended; there is no relevance weighting.]
