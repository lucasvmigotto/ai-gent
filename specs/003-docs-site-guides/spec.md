Reconstructed by project:introspec on 2026-10-18 from 384c771

# Feature Specification: Install, flow, contribute, reference and changelog pages

**Feature Branch**: `docs/introspec-site-runners` (reconstructed; the code already exists on `main`)

**Created**: 2026-10-18

**Status**: Draft

**Input**: Reverse-engineered from `docs/site/src/pages.tsx` (InstallPage, FlowPage, ContributePage, ReferencePage, ChangelogPage) and their copy in `docs/site/src/i18n.tsx`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Install the toolkit (Priority: P1)

A developer opens `#/use/install`, copies the one-line installer (Claude Code by default, or `--target` for other agents), verifies, reads how updates arrive, and moves on to the skills.

**Why this priority**: the brief's journey 1 ("install and first skill").

**Independent Test**: render the page; compare its commands with `install.sh`'s header (feature 006).

**Acceptance Scenarios**:

1. **Given** the install page, **When** it renders, **Then** it shows `curl -fsSL …/install.sh | sh` and the `sh -s -- --target claude,opencode,codex,cursor` form, each copyable [OBSERVED: docs/site/src/pages.tsx:206-241].
2. **Given** the page, **When** it renders, **Then** it explains that the other eight agents are opt-in, five share `~/.agents/skills`, and only Claude Code and opencode enforce the guards [OBSERVED: docs/site/src/i18n.tsx:205-206] — consistent with `setup.sh` [OBSERVED: setup.sh:24-30].
3. **Given** the page, **When** it renders, **Then** it shows verify commands `claude plugin details project@skills-dir` and `opencode plugin list` [OBSERVED: pages.tsx:226-229] [INFERRED: neither command's existence or output is tested anywhere; confirm in a real Claude Code and opencode install].

---

### User Story 2 - See the autonomous flow (Priority: P1)

A reader opens `#/use/flow` and sees the delivery loop at a glance (one line of icons) and step by step — Start, Design, Specify, Track, Develop in parallel, Land, Watch CI, Verify, Close, Release, Document — each with the skills it runs, which steps ask the user, the autonomy rule and the supporting skills.

**Why this priority**: the landing page's second call to action; the e2e smoke covers it.

**Independent Test**: `tests/pages.test.tsx` "renders the autonomous flow with every step and a link per skill" and "names only skills that exist on the flow page"; e2e `#/use/flow`.

**Acceptance Scenarios**:

1. **Given** the flow page, **When** it renders, **Then** every step in `t.flow.steps` appears in order with chips linking each named skill [OBSERVED: pages.tsx:539-601; i18n.tsx:251-350; tests/pages.test.tsx:46].
2. **Given** a step naming a skill, **When** tests run, **Then** the skill must exist in the generated content [OBSERVED: tests/pages.test.tsx:59].
3. **Given** the steps Land and Release, **When** they render, **Then** they are marked as asking the user [OBSERVED: i18n.tsx:316,342].

---

### User Story 3 - Prepare a contribution (Priority: P2)

A contributor opens `#/contribute` for conventions, the check command, layout, the release flow and a table of every plugin's version.

**Independent Test**: render the page; the versions table equals the plugin manifests.

**Acceptance Scenarios**:

1. **Given** the contribute page, **When** it renders, **Then** it shows conventions, `./scripts/check.sh`, layout, releases and one row per plugin with its manifest version [OBSERVED: pages.tsx:243-299].

---

### User Story 4 - Reach the shared contracts (Priority: P2)

A reader opens `#/reference` and gets a list of the toolkit's contracts — pipeline, containers, guard hooks, databases, reading code, Spec Kit, secrets, agent roles, tracker sync — each linked to its source file at the build's ref.

**Acceptance Scenarios**:

1. **Given** the reference page, **When** it renders, **Then** nine entries appear, eight linking to a repository file and "Guard hooks" carrying a paragraph [OBSERVED: pages.tsx:301-363].

---

### User Story 5 - Read the changelog (Priority: P3)

A reader opens `#/changelog` and reads `CHANGELOG.md` rendered as Markdown.

**Acceptance Scenarios**:

1. **Given** the changelog page, **When** it renders, **Then** the repository's `CHANGELOG.md` at build time is shown [OBSERVED: pages.tsx:603-614; docs/site/scripts/generate-content.mjs:104].

### Edge Cases

- The copy key `install.troubleshooting` ("If it does not work") exists but the page never renders a troubleshooting section [OBSERVED: i18n.tsx:214 vs pages.tsx:206-241] — the brief's journey 1 and the UX vision promise one.
- The UX vision's per-tool tabs on the install page are two stacked code blocks [OBSERVED: pages.tsx:213-223].
- Reference entries link out to GitHub instead of rendering pages (`/reference/pipeline` etc. in the vision's IA) [OBSERVED: pages.tsx:351-358].
- The contribute page has no section sidebar (vision's navigation model) [OBSERVED: pages.tsx:243-299].

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The install page MUST show the same installer commands as `install.sh`'s usage header.
- **FR-002**: The flow page MUST name only skills that exist (test-enforced).
- **FR-003**: The contribute page's versions table MUST come from the plugin manifests, never hand-written.
- **FR-004**: Reference links MUST point at the build's ref (tag, else commit).
- **FR-005**: The changelog page MUST render `CHANGELOG.md` as of the build.

### Key Entities

- **Flow step** — `key`, `title`, `skills[]`, `body`, optional `asks`, `track[]`, `loop` (copy in `i18n.tsx`, type `FlowStep`) [OBSERVED: i18n.tsx:251-350].

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Zero axe violations on the flow page [OBSERVED: tests/pages.test.tsx:70].
- **SC-002**: Every skill chip on the flow page resolves to a skill page.

## Assumptions

- [ASSUMPTION: linking reference entries to GitHub, rather than rendering them, is a deliberate simplification of the UX vision's IA.]
- [NEEDS CLARIFICATION: is the install troubleshooting section still wanted (brief journey 1), or should the dead `install.troubleshooting` key be removed?]
