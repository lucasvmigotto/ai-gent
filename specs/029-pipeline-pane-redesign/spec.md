# Feature Specification: Pipeline pane, dashboard layout (amends 013)

**Feature Branch**: `docs/mod-specs-2` (specification only; built on its own branch)

**Created**: 2026-10-31

**Status**: Accepted

**Input**: The maintainer's five images and written proposal (`~/Downloads/claude-mods-pipeline-panel-redesign.md`). Image 1, the target: a `PIPELINE` header with a green `LIVE` badge and "Updated automatically", three large stat tiles (artifacts ready, features, clarifications), an Artifacts table (Artifact, Stage, Status pill), a segmented Feature progress bar with count pills, and a Next action block with a `Needs attention` badge. Images 2–5 and the proposal: the artifacts table with a subtle left-edge status indicator; an aligned feature matrix (fixed-width ID, right-aligned priority, fixed-width status columns, the full status vocabulary, expandable rows, All/Ready/Blocked/Done filters); a secondary stage progression bar; a semantic colour table and glyphs with ASCII fallbacks; a structured view model rendered through `$.ui.resolve(e)` only; three width modes; five phases.

**Depends on**: 013 (pipeline pane). This spec changes how 013's snapshot is drawn and what the pane lets the person expand or filter. It does not change what is read or decided.

## Context and ground rules

- **What stays (013, unchanged)**: the `Snapshot` type (`plugins/project/types/index.d.ts`), `compute` and `recommend` (`pipeline-model.ts`, `pipeline.ts`), the reads through `$.fs` and `git log -1 --format=%cs`, the poll and the Edit/Write refresh, the `/pipeline-pane` command, the parity with `specsync.py features --json` and `wt.py plan --json`, and the rule that the pane never writes, fixes or decides. `project:status` stays the report of record. The next-stage recommendation stays `recommend`'s.
- **What changes**: the view. `pipeline-view.ts` moves from `Line { text, tone }` to a structured model (sections, columns, rows, cells, a summary); the model is drawn through `$.ui.resolve(e)` (`Box`, `Text`, `Button`) only; 013's line view stays as the compact layout and as the headless text.
- **Not a band**: the "at most two bands above the prompt" rule (decided 2026-10-31 for 023, 025 and 028) does not apply to a pane.
- **Surfaces**: no DOM, HTML table, CSS or browser component enters the module; no ANSI escape is required for styling; interaction is `Button` presses, which every surface has. Claude Code-only, like every mod; other agents are unaffected.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Read the pipeline at a glance (Priority: P1)

Opening `/pipeline-pane` draws, in order: the header (title, badge, caption), the summary metrics, the Artifacts table, the Feature progress bar, the Features matrix, the open items and the Next action block, all from the snapshot 013 computes.

**Why this priority**: it is what was asked for, and the data already exists.

**Independent Test**: a `claude plugin test` case mounts the pane on `terminal` and `desktop` against 013's fixture tree and finds by key each region.

**Acceptance Scenarios**:

1. **Given** 5 of 8 artifacts present and not stale, 28 README rows, 10 open `[NEEDS CLARIFICATION]`, 5 assumptions and 2 gaps, **When** drawn full width, **Then** the summary reads `5 Artifacts ready`, `28 Features`, `10 Clarifications` as the three large metrics and `5 Assumptions`, `2 Gaps` as two small ones.
2. **Given** the summary, **When** drawn, **Then** it is flat text: no card background, no border, no gradient.
3. **Given** the recommendation is rule 1 (an open clarification), **When** drawn, **Then** the Next action block carries `Needs attention`; rules 2 and 3 carry `Suggested`; no recommendation (`rule: null`) carries `Up to date`.
4. **Given** the Next action block, **When** the person presses `/project:status`, **Then** the prompt is filled and never submitted (013 US3 AC3), and the block ends with `Hint only · Full report: /project:status`.

---

### User Story 2 - A LIVE badge that tells the truth (Priority: P1)

The header badge reads `LIVE` only while the pane is really kept current, and says so when it is not.

**Why this priority**: a badge that always says LIVE is decoration, and 013's reason to exist is that a stale pane is worse than none.

**Independent Test**: with `$.clock.every` mocked, a test advances the clock and asserts the badge from the model.

**Acceptance Scenarios**:

1. **Given** the poll is running and the last check was within three poll intervals, **When** drawn, **Then** the badge is `LIVE` with the caption `Checked 14:02:11 · every 5 s`.
2. **Given** no check for more than three intervals (the poll stopped, the machine slept), **When** the pane next redraws, **Then** the badge is `STALE` (amber) with `Last checked 14:02:11`.
3. **Given** a poll tick that finds nothing changed, **When** it runs, **Then** the checked time still advances though the snapshot does not.
4. **Given** headless or the compact layout, **When** the text prints, **Then** there is no badge, only `as of 14:02:11`.
5. **Given** an interaction (a filter, an expansion), **When** it redraws, **Then** it starts no poll and reads no file (FR-013).

---

### User Story 3 - Artifacts as a stage-aware table (Priority: P1)

The Artifacts section is a table of Artifact, Stage and Status, one row per entry of 013's artifact list, with a subtle left-edge indicator in the status colour; the full path, originating command and staleness are one press away.

**Independent Test**: model tests on the four states from 013's fixtures; a tree test for the row keys and the expansion.

**Acceptance Scenarios**:

1. **Given** the four situations 013 distinguishes, **When** drawn, **Then** the status words are `Ready` (present, not stale), `Stale` (present, an upstream artifact committed later; the row names it and its date), `Missing` (absent although a later stage ran, `isProblem`), `Pending` (absent, nothing later needs it). These are 013's states renamed for display; none is new.
2. **Given** a row, **When** drawn, **Then** it shows the short label, the stage name (`Project init`, `Architecture`, `UI/UX`, `Specification`, `Backend`, `API specification`, `QA strategy`, `DevOps`) and the status word with its glyph, and a one-cell left-edge mark in the status's colour (`success`, `warning`, `error`, `inactive`).
3. **Given** the person presses a row, **When** it expands, **Then** it shows the full path, the owning command (`/project:architecture`) and, for `Stale`, the upstream path and date; pressing again hides them.
4. **Given** no git, **When** drawn, **Then** `Stale` cannot be told, no row is `Stale`, and a line says `Staleness needs git.`

---

### User Story 4 - A real feature matrix (Priority: P1)

The Features section is an aligned matrix: ID, Feature, Priority (right-aligned), a **Ready** column (readiness) and one fixed-width column per status side the README has, each cell showing the status as a glyph and its word. Readiness and implementation status are never drawn the same way.

**Why this priority**: the matrix is where the proposal says a grid pays most, and the existing line list does not align.

**Independent Test**: model tests for the columns on three README shapes; a golden text for the matrix at 100 columns; a long-name test.

**Acceptance Scenarios**:

1. **Given** 013's fixture README (Frontend, Backend, QA), **When** drawn above 90 columns, **Then** the columns are `ID`, `FEATURE`, `PRI`, `READY`, `FRONTEND`, `BACKEND`, `QA`; ID is 3 cells, PRI is 3 cells right-aligned, READY 7, each status column 13 (a glyph, a space, the word), and FEATURE takes the remainder.
2. **Given** a README with a single `Status` column (the format `pipeline-model.ts`'s `SIDES` also reads), **When** drawn, **Then** the one column is headed `STATUS`; there is no `S` column when the README has none (see Decisions).
3. **Given** the full vocabulary, **When** drawn, **Then** Planned, In progress, Partial, Implemented, Verified, N/A and an empty or `—` cell each get a distinct word, glyph and tone (the table is in `data-model.md`), and `Implemented (tests untested)` shows `Implemented` with the full cell text on expansion.
4. **Given** a feature that is `ready` to start and whose status is Planned, **When** drawn, **Then** the READY cell says `ready` (in the active tone) and the status cells say `Planned` (amber): two columns, two vocabularies, no shared glyph.
5. **Given** a feature name longer than its column, **When** drawn, **Then** it is cut with `…` and the full name appears on expansion; no row wraps or overflows.
6. **Given** a retired feature, **When** drawn, **Then** its row is dim, READY says `done` and its cells say `Retired`.
7. **Given** the image 3 rendering glitch (the QA cell showing `secondary>-`), **When** a cell has no status, **Then** it draws a plain `—` (ASCII `-`), never markup text.

---

### User Story 5 - Filter and expand features (Priority: P2)

Four buttons, `All`, `Ready`, `Blocked`, `Done`, filter the matrix by readiness; pressing a row expands it with its dependencies, notes and files.

**Independent Test**: model tests on the filter for 013's fixture (the buckets equal `wt.py plan --json`'s split); a tree test that pressing a row key writes the atom and expands it.

**Acceptance Scenarios**:

1. **Given** the filter `Ready`, **When** drawn, **Then** only features whose `bucket` is `ready` show; `Blocked` shows `blocked`; `Done` shows `done`; `All` shows every row. The buckets are 013's (`wt.py plan`), unchanged, and the active button is marked with a glyph as well as a tone.
2. **Given** a filter with no match, **When** drawn, **Then** the section says `No ready features.` and the buttons stay.
3. **Given** a row pressed open, **When** it expands, **Then** it shows `Depends on` (each dependency as `↳ 002 name`, ASCII `- 002 name`, with `waits for` for the ones not built), the README `Notes`, and `spec.md`, `plan.md`, `tasks.md` as present or absent (`Feature.files`).
4. **Given** the filter or an expansion set, **When** the pane redraws on a poll tick, **Then** both are kept (they are atoms).
5. **Given** no sorting, search or nested level (Out of scope), **When** drawn, **Then** none is offered.

---

### User Story 6 - Feature progress and stage progression (Priority: P2)

One segmented bar shows how many features are Implemented, Planned and Other, with a count pill each and the total at its right. Below the Artifacts table a six-step strip (`Init`, `Architecture`, `Spec`, `Build`, `QA`, `Delivery`) shows done, current and pending, as a secondary aid.

**Independent Test**: model tests for the bucket counts and the stage states on 013's fixtures; a tree test for the segments' `flexGrow`.

**Acceptance Scenarios**:

1. **Given** 17 features whose least advanced side is Implemented or Verified, 7 Planned and 4 doing or retired, **When** drawn, **Then** the bar has segments with `flexGrow` 17, 7 and 4 and the pills read `17 Implemented`, `7 Planned`, `4 Other`; the counts sum to the README rows.
2. **Given** a bucket with zero features, **When** drawn, **Then** its segment is omitted and its pill reads `0 …` dim; a non-zero segment keeps `minWidth` 1.
3. **Given** the progress bar and the READY column, **When** drawn, **Then** the bar is labelled `Implementation` and nothing in it uses readiness words, so the two are not conflated.
4. **Given** the stage strip, **When** drawn, **Then** each step is `done`, `current` or `pending` from the existing snapshot (the rule is in `data-model.md`), and the strip is labelled as an aid; `Next action` still comes from `recommend` and never from the strip.
5. **Given** no `specs/README.md`, **When** drawn, **Then** the section says `No specs/README.md yet.` and draws no bar.

---

### User Story 7 - Open items, each expandable (Priority: P2)

Clarifications, assumptions and gaps each have a count in the summary and each expands on its own to up to 20 `file:line text` entries with `+N more` after (013's cap).

**Acceptance Scenarios**:

1. **Given** 10 clarifications, 5 assumptions and 2 gaps, **When** the person presses `Clarifications`, **Then** only that list opens; the others stay closed.
2. **Given** zero clarifications, **When** drawn, **Then** the number is dim, not amber.
3. **Given** more than 20 markers of a kind or the scan hit 013's cap, **When** drawn, **Then** `+N more` or `(more not counted)` shows.

---

### User Story 8 - Choose the layout and the glyph set (Priority: P3)

`pipelineLayout` (`dashboard` or `compact`) selects the drawing; `pipelineAscii` (boolean) forces the ASCII glyph set.

**Acceptance Scenarios**:

1. **Given** neither set, **When** drawn, **Then** it is the dashboard with the Unicode glyphs.
2. **Given** `pipelineLayout: compact`, **When** drawn, **Then** it is 013's `view` output, byte for byte on 013's fixtures.
3. **Given** `pipelineAscii: true`, **When** drawn, **Then** every glyph is its fallback and no non-ASCII glyph appears.
4. **Given** any other `pipelineLayout` value, **When** drawn, **Then** the dashboard is used.

### Edge Cases

- **Width modes**: under 40 columns the compact text (013's narrow rule); 40 to 90 inclusive the condensed grid (the summary on one line, the artifacts table without the Stage column, the matrix with glyph-only status cells under a one-line legend, no Priority text longer than three cells); above 90 the full matrix. The width is `e.props.bodyColumns`; nothing assumes a fixed width.
- **No colour**: every colour has a word or glyph beside it. The only colour-only element is the bar's fill, and its labelled counts sit directly beneath it.
- **Empty pipeline** (no artifacts): the dashboard is not drawn; the pane says `No pipeline artifacts yet. Start with /project:init.` as 013 does.
- **Empty README**: the Features section says so (013's text).
- **Large lists**: the matrix draws at most 60 rows (a `userConfig` number, `pipelineMaxRows`) and then `+N more; filter to see them`; a hundred rows must not slow a redraw.
- **Hot reload**: module variables reset; the filter, expansions and the check time are atoms.
- **Headless (`claude -p`)**: `/pipeline-pane` answers 013's text snapshot, unchanged.
- **Pane closed**: the poll is stopped (013); closing is untested for the reason 013 recorded (the kit cannot raise `ui.close`).
- **Surfaces**: `desktop`, `vscode` and `mobile` draw the same model with their own `Box` and `Text`; `Text size` may map the large metrics there and is ignored on the terminal.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The redesign MUST NOT alter `compute`, `recommend`, `refresh`, the files read, the processes spawned, the poll interval or the `Snapshot` type. The view model is a pure function of the snapshot and a small context (width, expanded set, filter, check time, glyph set).
- **FR-002**: `pipeline-view.ts` MUST export a structured model (`PipelineView`: sections, columns, rows, cells, the summary with artifacts, features, clarifications, assumptions and gaps) built by a pure `build`, and MUST keep `view` and `text` so compact and headless output is unchanged.
- **FR-003**: The model MUST be drawn only through `$.ui.resolve(e)` (`Box`, `Text`, `Button`); no HTML, CSS or ANSI escape is used for styling.
- **FR-004**: Artifact status MUST come from `isPresent`, `staleSince` and `isProblem` as 013 sets them (FR-003 of 013 parity unchanged); the display words are `Ready`, `Stale`, `Missing`, `Pending`.
- **FR-005**: Each artifact row MUST show a stage name from a constant in `pipeline-model.ts` keyed by `ArtifactState.stage`; `scripts/consistency.py` fails when a stage the table uses has no name.
- **FR-006**: The matrix columns MUST be derived from the status sides present in `Feature.sides` (`frontend`, `backend`, `status`, `qa`); no fixed `S` column is invented.
- **FR-007**: A status cell MUST show the glyph and the normalised word (`word()` of 013); the full vocabulary (Planned, In progress, Partial, Implemented, Verified, N/A, retired, empty) MUST be kept and distinct.
- **FR-008**: Readiness MUST be `Feature.bucket` (`ready`, `blocked`, `done`), shown in its own column with words, and MUST NOT share a glyph or column with implementation status.
- **FR-009**: The filter buttons MUST filter by `Feature.bucket` only; nothing is added to the model for them.
- **FR-010**: Colours MUST be theme keys (`success`, `warning`, `error`, `inactive`, `subtle`, `suggestion` for active, `text`); every colour MUST have a glyph or word; no raw hex and no ANSI true-colour as primary styling.
- **FR-011**: Glyphs MUST come from one table with a Unicode and an ASCII entry each (`data-model.md`); `pipelineAscii` selects the ASCII set.
- **FR-012**: The badge MUST be derived from state: `LIVE` only when the poll is running and the last check is no older than three intervals, else `STALE`; the check time MUST advance on every tick.
- **FR-013**: Interaction (expanding, filtering) MUST write `$.state` atoms only: no file read, no spawn, no timer. Rendering MUST NOT read files.
- **FR-014**: State read while drawing MUST be in `$.state` atoms declared in `plugins/project/types/index.d.ts`, read in `pipeline.ts` (an atom is read only where it is declared) and passed to the drawing as values.
- **FR-015**: `userConfig` MUST add `pipelineLayout` and `pipelineAscii`, without touching `pipelinePane` or `pipelinePollSeconds`.
- **FR-016**: A render that throws or is refused MUST fall back alone (the engine draws its own); the compact layout MUST stay reachable by the option.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/project` and `claude plugin validate plugins/project` pass; 013's tests pass unchanged for `compact` and headless.
- **SC-002**: On this repository the bucket counts sum to the README's row count, the per-feature states still equal `specsync features --json` and the readiness split still equals `wt.py plan --json` (013's parity test untouched).
- **SC-003**: With the pane open for 10 minutes and no change, the module spawns no process after the first draw; the only new work is one atom write per interval.
- **SC-004**: With the poll stopped in a test, the badge is `STALE` within three intervals.
- **SC-005**: The golden text of the dashboard for 013's fixtures at 120, 80 and 39 columns equals the stored strings, in Unicode and in ASCII.
- **SC-006**: The compatibility table in `plan.md` passes row by row.

## Out of scope

- Any change to what the pane reads or decides, to `shared/pipeline.md`, to `plugins/project/skills/status/SKILL.md`, or to `specsync.py` and `wt.py`.
- Sorting, search and nested navigation; charts of history; editing from the pane; CI and token figures (they stay in `project:status`).
- Gradients, animations, oversized cards, and any ANSI true-colour styling.
- A band version of the dashboard.
- Any other agent.

## Decisions (decided 2026-10-31 (the maintainer accepted the default))

Every item here is my choice where the images and the proposal leave a gap or two sources differ; none has been put to the maintainer.

- **The `S` column is dropped as a fixed column.** The proposal's matrix has `F B S Q`, but this repository's README has Frontend, Backend and QA (`specs/README.md`), and `S` is undefined. In `pipeline-model.ts`, `status` is a side only for a README that has a single `Status` column (`SIDES`, read by `specsync.py` and `wt.py` too). So the columns are the sides the README has: `F B Q` here, or one `STATUS` for a single-status README.
- **`Blocked` and `Ready` are not new.** They are `Feature.bucket`, from `wt.py plan`: ready = no dependency unbuilt and some side below Implemented; blocked = a dependency not Implemented in every side it builds; done = no side below Implemented.
- **The first image's three large metrics plus the proposal's assumptions and gaps**: the three stay large, flat text; assumptions and gaps are two small metrics beside them.
- **Colour-alone is not allowed**, so the bar (which is) always has its labelled counts under it.
- **Active colour**: the theme key `suggestion` stands for the proposal's blue/cyan.
- **Glyphs the proposal does not give** (Implemented, Partial, Retired, N/A): in `data-model.md`, Planned; the proposal's eight keep its Unicode and ASCII pairs.
- **Stage progression**: derived from existing artifacts and buckets by a pure rule (Planned, `data-model.md`), labelled an aid, never read by `recommend`.
- **Stage name for `domain-model`** is `Backend` (as in image 1); `Domain model` is more exact.
- **Image 3's `secondary>-` glitch** is a rendering bug in the mockup; a cell with no status draws `—` (ASCII `-`).
- **LIVE honesty**: `LIVE` means a tick ran within three intervals; the check time advances each tick through a new atom, which redraws once per interval. Writing only on change would leave the caption stale on a quiet repository, so it is rejected. The caption `Checked 14:02:11 · every 5 s` replaces "Updated automatically", which a stopped poll cannot back.
- **Dashboard is the default** (`pipelineLayout`), `compact` is 013's view; ASCII is opt-in (`pipelineAscii`, default false).
- **Row cap**: 60 rows in the matrix (`pipelineMaxRows`), then a line `+N more; filter to see them`.
- **Hotkeys** on the filter buttons are an enhancement where the surface holds the keyboard; nothing depends on them.
- **File placement**: the drawing goes in a new `pipeline-draw.tsx` that `register.tsx` imports, so `register.tsx` does not grow (the proposal puts it in `register.tsx`).

### Built so far (groups 1 to 3 of the plan; recorded 2026-10-31)

- **Done**: the structured model (`build`, `dashboardText`, `statusCell` in `pipeline-view.ts`; `STAGE_NAME`, `progress`, `stageStrip` in `pipeline-model.ts`), the native renderer (`pipeline-draw.tsx`), the options `pipelineLayout`, `pipelineAscii` and `pipelineMaxRows`, the atoms `pipelineFilter`, `pipelineCheckedAt` and `pipelinePollMs`, the LIVE badge, and the golden strings at 120 and 80 columns in Unicode and ASCII. 013's `view` and `text` are pinned by goldens and unchanged; 013's pane tests run under `pipelineLayout: compact`, the layout they describe, because the default changed.
- **Remains (group 4 and 5)**: expandable artifact and feature rows, the All/Ready/Blocked/Done buttons (the filter is in the model and its atom exists, but nothing writes it; the dashboard shows the counts as text), the per-kind open-item buttons (one Show/Hide button opens all three kinds for now), the `scripts/consistency.py` check that an artifact stage has a `STAGE_NAME` (a test in `pipeline-view.test.tsx` checks it meanwhile), the compatibility table run, the manual runs, and the docs-site, `AGENTS.md` and `CHANGELOG.md` pages.
- **LIVE is the poll's verdict, not a clock comparison in the render.** The ticker writes `pipelineCheckedAt` on every check and `pipelinePollMs` (the interval; 0 when the poll stops or a check fails); the pane passes the last check as `now`, so it reads LIVE while the poll keeps a positive interval and has checked, and STALE at once when it stops or a check fails. `build` still implements the three-interval rule (tested with explicit times), but a machine that slept between ticks cannot show STALE until the next check fails, since nothing reads the clock while drawing.
- **READY has no glyph**, only the words `ready`, `blocked`, `done`, so the column stays 7 cells wide as the spec says; `data-model.md`'s `▶` and `⏸` appear in 013's compact lines only.
- **Condensed status cells** are the glyph alone, so `N/A` and `Retired` share `—` there; the legend line names them and the full mode keeps the words.
- **The progress bar** uses three block characters (`█ ▒ ░`, ASCII `# = -`) so the segments differ without colour; its labelled counts sit under it.
- **ASCII covers the dashboard only**: the compact width (under 40 columns) and `pipelineLayout: compact` print 013's lines, which use Unicode.
- **The mock clock**: the engine's mock clock starts at 0, which the pane reads as "Not checked yet"; the LIVE test advances it first.
- **Not verified (needs an interactive session)**: whether a coloured, bold run reads as a pill on every surface (no `backgroundColor` is used anywhere, so no background painting was assumed and none was probed); whether `Text size="title"` maps on desktop and vscode; whether `Box key` regions, `flexShrink` and `truncate-end` cells align as the plain-text goldens do (the mounted tests check the tree, not the pixels); whether `Date` local-time formatting is allowed in a render hook (it ran in `claude plugin test`); whether a `Button` can wrap a row (each Button holds one cell, so the question did not arise).

## Assumptions

- [ASSUMPTION: an empty `Box` with `height` 1 and a `backgroundColor` paints a solid run on every surface. Unverified; the plan's first probe checks it. Fallback: `Text` runs of `█` sized by the same counts with the largest-remainder rule.]
- [ASSUMPTION: `$.clock.now()` may be called in a render hook (a read). If not, the ticker writes the badge verdict into the atom instead.]
- [ASSUMPTION: `userConfig` takes a string with a default. If only booleans and numbers are allowed, `pipelineLayout` becomes the boolean `pipelineCompact`.]
- [ASSUMPTION: a `Button` can carry a `Box` row as its press target, or each cell is its own button. If neither, the row's first cell is the button.]
- [NEEDS VERIFICATION: whether the kit says if a terminal paints background colours or Unicode glyphs; absent that, the word-and-glyph rule and `pipelineAscii` are the guarantee.]
- Found 2026-10-31: `claude plugin validate` on 2.1.312 accepts `userConfig` options of `type: string` and a module calling `$.clock.now`; the rendering questions (empty `Box` background, `Button` around a row) still need the plan's probe in an interactive session.
