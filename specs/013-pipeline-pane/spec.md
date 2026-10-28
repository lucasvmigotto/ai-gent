# Feature Specification: Pipeline pane (project:status, live)

**Feature Branch**: `docs/mod-specs` (specification only; built on its own branch)

**Created**: 2026-10-28

**Status**: Draft

**Input**: A Claude Code mod that keeps `project:status`'s snapshot on screen: the artifacts and feature statuses `shared/pipeline.md` defines, read with `$.fs`, redrawn when they change, ending in what to run next. Grounded in `plugins/project/skills/status/SKILL.md` and `shared/pipeline.md`.

**Depends on**: 012 (checkpoint mod) — it settles the module layout, `check.sh` wiring, versioning and install that this feature reuses unchanged. 013 adds its code to the same `project` plugin module 012 creates.

## Context and ground rules

- A mod is an optional, Claude Code-only layer. `project:status` stays the report of record: this pane shows a deterministic subset of it and never writes, fixes or decides anything. It is not part of `shared/pipeline.md`; no stage reads what it shows.
- Owner: the `project` plugin. Its module is `plugins/project/hooks/register.tsx` (shared with 012), named in `plugins/project/hooks/hooks.json` under `"modules"`; the pane's code sits beside it as `plugins/project/hooks/pipeline-pane.ts(x)`.
- Other agents installed by `setup.sh` never load a hooks module, so nothing changes for them.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See where the pipeline stands without asking (Priority: P1)

The user opens a pane (`/pipeline`, or a docked sidebar opened automatically on a wide fullscreen terminal) that lists each artifact of the pipeline's table as present, missing or stale, and one row per feature from `specs/README.md` with its frontend, backend and QA status.

**Why this priority**: it is the whole point; the user stops spending a turn on "where are we".

**Independent Test**: a `claude plugin test` case mounts the pane on `terminal` and `desktop` against a fixture tree (`docs/product/brief.md`, `architecture.md`, `specs/README.md` with three rows) and finds the expected rows by key.

**Acceptance Scenarios**:

1. **Given** a repository with `docs/product/brief.md` and `specs/README.md` but no `architecture.md`, **When** the pane opens, **Then** brief is ✓, architecture is ✗ and flagged as a problem (a later stage ran), and each README row is listed.
2. **Given** `brief.md` committed after `architecture.md` (`git log -1 --format=%cs`), **When** the pane draws, **Then** architecture is marked stale with the brief's date.
3. **Given** a README row whose status cell reads `Implemented` with a note after it, **When** parsed, **Then** the status is Implemented (the same reading `specsync` and `wt.py` apply).
4. **Given** no `docs/product/` and no `specs/`, **When** the pane opens, **Then** it says the project has no pipeline artifacts and suggests `/project:init`, as the skill does.

---

### User Story 2 - The pane follows the work (Priority: P1)

When a stage writes an artifact or flips a status, the pane redraws without the user asking.

**Why this priority**: a stale pane is worse than none.

**Independent Test**: a test changes a fixture file's content through the kit's `$.fs`, advances the mocked clock past the poll interval, and asserts the redrawn row.

**Acceptance Scenarios**:

1. **Given** the pane is open, **When** an Edit or Write tool call on a path under `docs/product/` or `specs/` completes, **Then** the pane re-reads that path and redraws.
2. **Given** a file changed outside the session (another terminal, a `git pull`), **When** the poll interval elapses, **Then** the change shows (polling `$.fs.stat` `mtimeMs`; the API has no file watcher).
3. **Given** the pane is closed, **When** files change, **Then** nothing is polled or read.

---

### User Story 3 - Know what to run next (Priority: P2)

The pane ends in one recommended stage and the reason, following `project:status` §4 rules 1–3: an open `[NEEDS CLARIFICATION]` first, then the earliest missing artifact later stages depend on, then the highest-priority feature whose dependencies are done, at its least advanced side.

**Independent Test**: fixture trees for each rule, one test each.

**Acceptance Scenarios**:

1. **Given** an open `[NEEDS CLARIFICATION: …]` in `specs/004-x/spec.md`, **When** the pane draws, **Then** Next names it with `file:line` and says it blocks `plan`.
2. **Given** no blocker and feature 002 (P1, depends on 001 Implemented) Planned on the backend, **When** the pane draws, **Then** Next is the backend step for 002.
3. **Given** the recommendation, **When** the user presses it, **Then** the pane fills the prompt with `/project:status` (the full report, with CI and contradictions) — it never submits on its own.

---

### User Story 4 - Count the open items (Priority: P3)

A line counts `[UPSTREAM GAP: …]`, `[NEEDS CLARIFICATION: …]` and `[ASSUMPTION: …]` markers under `docs/product/` and `specs/`, expandable to `file:line`.

**Acceptance Scenarios**:

1. **Given** two clarifications and five assumptions, **When** drawn, **Then** the line reads `2 clarifications · 5 assumptions · 0 gaps`, and pressing it lists them.

### Edge Cases

- **No git** (or not a checkout): staleness is skipped and the artifacts row says "staleness needs git"; presence and statuses still show.
- **Missing or malformed `specs/README.md`**: the features block says so; a row without a 3-digit first cell is ignored, as `wt.py` ignores it.
- **Large trees**: marker counting reads only `*.md` under `docs/product/` and `specs/`, with a cap (files and bytes) after which the line says "more not counted".
- **Narrow terminal**: below the width `$.ui.open` needs to place a pane unasked, nothing opens by itself; the command still opens it inline. The tree is sized to `e.props.bodyColumns`; under 40 columns the features table collapses to `NNN name F/B/Q` letters.
- **Non-terminal surfaces** (`desktop`, `vscode`, `mobile`): the same tree, drawn with that surface's elements; nothing depends on terminal paint. `mobile` gets the command only, never an unasked pane.
- **Hot reload**: module variables reset, so what is drawn (open state, last snapshot, expanded sections) lives in `$.state` atoms; the snapshot is recomputed after a reload.
- **Headless (`claude -p`)**: no surface; the module registers nothing that draws, and the command answers with the text snapshot.
- **Clock**: the poll uses `$.clock.every`, stopped when the pane closes and dropped on reload.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The pane MUST read only through `$.fs` (and `$.process.run` for `git log -1 --format=%cs -- <path>`), and MUST NOT write any file, run a stage, or fetch.
- **FR-002**: The artifact list MUST be the paths of `shared/pipeline.md`'s artifact table that `project:status` §1 checks; it is read from a constant in the module that a test compares with the table's paths, so a contract change fails the test.
- **FR-003**: README parsing MUST agree with the two scripts that already parse it: each feature's overall state equals `plugins/repo/scripts/specsync.py features --json`'s `state` (Planned → todo … Verified → done, least advanced side wins), and the ready / blocked / done split equals `plugins/agents/scripts/wt.py plan --json` (a parity test on fixtures).
- **FR-004**: The pane MUST redraw within one poll interval (default 5 s, a `userConfig` number) of a change to a watched path, and immediately after an Edit/Write tool call on one.
- **FR-005**: Next MUST follow `project:status` §4 rules 1–3 and MUST label itself a hint, pointing to `/project:status` for the full report.
- **FR-006**: The module MUST register a `/pipeline` command that opens (or focuses) the pane; with no surface it answers the snapshot as text.
- **FR-007**: A `userConfig` boolean (`pipelinePane`, default true) MUST turn the whole feature off without touching 012's parts of the module.
- **FR-008**: State that drawing reads MUST be in `$.state` atoms declared in `plugins/project/types/index.d.ts`, never in module variables.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/project` passes, with each user story covered on `terminal` and `desktop`.
- **SC-002**: On this repository, the pane's per-feature state matches `specsync features --json` and its ready / blocked / done split matches `wt.py plan --json`, for all rows.
- **SC-003**: With the pane open for 10 minutes and no changes, the module spawns no process after the first draw (only `stat` calls).
- **SC-004**: `claude plugin validate plugins/project` reports no refusal.

## Out of scope

- CI results, tracker drift, token figures and contradiction checks (evidence vs. claimed status): they need network or judgment and stay in `project:status`.
- Editing statuses or artifacts from the pane.
- Any other agent (opencode, Codex, …).
- Changing `shared/pipeline.md` or `plugins/project/skills/status/SKILL.md`.

## Decisions (2026-10-28, with the maintainer)

- Parsing: the module parses `specs/README.md` and the artifacts itself with `$.fs` (no Python at draw time); FR-003's parity test keeps it equal to `specsync features --json` and `wt.py plan --json`.
- Opening: unasked at session start only when `specs/README.md` exists and the surface docks panes; otherwise through `/pipeline`.

## Assumptions

- [ASSUMPTION: `project` hosts both 012 and 013 in one `register.tsx`, which composes per-feature files; one module per plugin is the engine's rule.]
- [ASSUMPTION: Claude Code 2.1.312's `$.fs.stat`, `$.clock.every` and `Pane` stay as declared; the API is early access.]
