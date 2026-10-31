# Feature Specification: Card timeline pane

**Feature Branch**: `docs/mod-specs-2` (specification only; built on its own branch)

**Created**: 2026-10-31

**Status**: Implemented

**Input**: A Claude Code pane for an `agents:orchestrate` run: one row per card of `.ai-gent/orchestrate-status.md` with its state, branch (when the ledger has that optional column), projected and used tokens, a log of the state changes the mod saw, and — on demand — each card's span on a shared time axis, so which cards ran in parallel is visible. Read-only: it reads the ledger, and runs `token_report.py` in its read-only mode only when asked.

**Depends on**: 014 (token status) — the ledger reader (`parseLedger` in `plugins/agents/hooks/token-model.ts`); 013 (pipeline pane) — the pane pattern (`Pane` through `ui.render`, `$.ui.open`, an `mtimeMs` poll, a `command.run` command, `ui.close`; `plugins/project/hooks/register.tsx`).

## Context and ground rules

- The ledger is hand-kept by the coordinator except its Tokens column (`token_report.py tasks`); it has no timestamps, and it lives in `.ai-gent/`, which is git-excluded, so there is no history to read. State changes are therefore only what the mod itself observes while it runs, kept in `$.state` (survives a hot reload, not a new session) — the pane says "since HH:MM".
- Spans come from the transcripts: `token_report.py`'s attribution groups already carry each card's `start` and `end` (`build_groups`, `token_report.py:747-748`), but `tasks` prints only a text table. A read-only `tasks --dry-run --json` (**Planned**, FR-002) is the one script change, a **prerequisite change landed before this feature** (its own branch and commit, not a task of 020), and 014's User Story 5 (`/tokens --live`), still unbuilt, is to reuse it. Reading transcripts is slow, so it runs when the pane opens and when the user presses *Measure*, never on the poll.
- The mod computes no token figure: figures are the ledger's cells and the script's JSON. The only things it derives are time arithmetic (an interval overlap and the axis scale) and counts.
- Owner: the `agents` plugin, the same module as 014 (a second feature in it; a failing 020 hook must not take 014 down, as 013 does for 012). Claude Code-only.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See every card and its state in one place (Priority: P1)

`/cards-pane` opens a pane with a header (`5 cards · 2 running · 1 review · 1 queued · 1 landed`) and one row per ledger card: id, state, branch (and worktree) if the ledger has such columns (optional: `agents:orchestrate`'s ledger mandates only `Card` and `State`, `SKILL.md` "Ledger"; the Tokens column is written by the script), and the Tokens cell as written.

**Why this priority**: the ledger is a Markdown file the user reads by opening it; the coordinator's wave is the thing they are waiting on.

**Independent Test**: a `claude plugin test` case mounts the pane on `terminal` and `desktop` against a fixture ledger of five cards and finds the rows by key.

**Acceptance Scenarios**:

1. **Given** a ledger with the mandatory `Card` and `State` columns and the optional `Branch` and `Tokens` columns, **When** the pane opens, **Then** each card is a row with its state, branch and `~420k / 391k`, and the header counts the states.
2. **Given** a ledger with no `Branch` column, **When** the pane opens, **Then** rows omit the branch.
3. **Given** a state outside `queued`, `running`, `review`, `landed`, **When** read, **Then** the row shows it dimmed and the header counts it as `other` (the skill's vocabulary is fixed; anything else is a ledger problem).
4. **Given** no ledger, **When** `/cards-pane` runs, **Then** it says there is no orchestration run here (`.ai-gent/orchestrate-status.md` missing) and opens nothing.

---

### User Story 2 - The pane follows the run and logs the changes (Priority: P1)

While open, the pane re-reads the ledger when its `mtimeMs` changes (a poll, default 5 s), redraws, and appends each state change it sees to a log shown under the rows: `15:02 004-us1-core queued → running`.

**Why this priority**: a static list is the ledger again; the changes are what a person waiting on a wave wants.

**Independent Test**: a test rewrites the fixture ledger through `$.fs`, advances the mocked clock past the poll, and asserts the new row state and the log line.

**Acceptance Scenarios**:

1. **Given** the pane is open, **When** the ledger file changes, **Then** within one poll the rows redraw and each card whose state differs gets a log line with the time it was seen.
2. **Given** the pane is closed, **When** the ledger changes, **Then** nothing is read or polled by this feature (014's own refresh is separate).
3. **Given** the log passed 200 entries, **When** a new one is added, **Then** the oldest drops.
4. **Given** the first read after opening, **When** cards are already `running`, **Then** no log line is invented; the log says `since 15:00 (changes seen in this session)`.

---

### User Story 3 - See which cards ran in parallel (Priority: P2)

A *Measure* button in the pane (and `/cards-pane --measure`) runs `token_report.py tasks --dry-run --json` and draws, for each card with transcripts, a bar on a shared axis from the earliest start to the latest end, the peak concurrency in the header (`peak 3 in parallel`), and the card's `used` from the script beside the ledger's.

**Why this priority**: orchestration's point is parallel work under one limit; the overlap shows whether the wave was really parallel and where it queued.

**Independent Test**: a mocked `$.process.run` answering fixture JSON with overlapping and disjoint spans; assert the bars' columns and the peak.

**Acceptance Scenarios**:

1. **Given** cards A (14:00–14:40) and B (14:10–14:50) and C (15:00–15:20), **When** measured, **Then** A and B overlap, C is alone, and the header says `peak 2 in parallel`.
2. **Given** a card with no subagent transcripts yet (queued), **When** measured, **Then** it has no bar and its row says `no run yet`.
3. **Given** the script fails or times out (60 s), **When** measured, **Then** the pane says so and keeps the rows and the log; nothing else changes.
4. **Given** the pane just opened, **When** it draws, **Then** it did not run the script: spans show `press Measure` until asked.
5. **Given** `tasks --json` is run without `--dry-run`, **When** the script parses its arguments, **Then** it exits with an error (the mod can never cause a write; FR-002).

---

### User Story 4 - Read it when the surface has no pane (Priority: P3)

Where the kit cannot dock a pane (headless, a narrow or mobile surface), `/cards-pane` prints the same content as text in the transcript: the header, one line per card, the log, and the spans as a plain list.

**Independent Test**: the command's text on a fixture, with and without a measurement.

**Acceptance Scenarios**:

1. **Given** a surface that cannot open the pane, **When** `/cards-pane` runs, **Then** the reply is the text form.
2. **Given** a pane narrower than 60 columns, **When** it draws, **Then** the bars are dropped and the rows keep id, state and the Tokens cell.

### Edge Cases

- **Observed log only**: transitions before the session, or while the session was closed, are unknown; the pane never infers them from the file.
- **Card ids in `tasks --json` but not in the ledger** (older runs in the same transcripts): ignored; the ledger decides which cards exist.
- **Card in the ledger with two attribution groups** (a fix round): the span covers the earliest start to the latest end of the card's groups, as the script reports one record per card id.
- **A very long run**: the axis is scaled to the pane's `bodyColumns` (013 does the same for its rows); under one column per minute the bar shows a minimum one cell so a short card is visible.
- **Clock and zones**: transcript times are UTC; the axis is labelled in the viewer's local time.
- **Cost**: `--measure` reads every transcript of the project (slow on large runs); it is never on the poll and never on a timer, and runs at most one at a time.
- **Pane lifecycle**: closing it clears its poll and reads; reopening starts the log from the saved `$.state` entries.
- **Headless (`claude -p`)**: no pane and no command output beyond the text form.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Every token figure shown MUST be a ledger Tokens cell as written or a field of `token_report.py`'s JSON; the mod MUST derive only counts, interval overlap and the axis scale, and MUST write nothing (ledger, `cards.md`, history, `$.fs` writes).
- **FR-002**: `token_report.py tasks` MUST gain `--json`, valid only with `--dry-run` (rejected through the argument parser's error, exit 2, otherwise), printing instead of the text table one object: `cards` (each `card`, `feature`, `model`, `projected`, `used`, `start`, `end`, `agents`, `calls`) and `unattributed` (count). `start` and `end` are UTC with an explicit `Z` (the script's own timestamps are UTC without a suffix, which JavaScript would read as local time). The text output and every write path are unchanged. This is a prerequisite change landed before this feature and reused by 014's `/tokens --live`; tested in `test_token_report.py` against a fake `HOME`.
- **FR-003**: The mod MUST read the ledger with 014's `parseLedger`, extended to return `Branch` and `Worktree` cells when those columns exist, without changing what 014 reads.
- **FR-004**: The state log MUST record only transitions the mod observed (card, from, to, local time), capped at 200 entries, kept in `$.state`.
- **FR-005**: The mod MUST find the script relative to its plugin root (never on `PATH`), run it only on the *Measure* press or `--measure`, with a 60-second timeout, one run at a time. `tasks` requires `--project`, so the mod passes `--project=<slug>` (the `=` form, since the slug starts with `-`), the slug being the session's cwd with every character outside `A-Za-z0-9-` replaced by `-`, as `cmd_budget` derives it (`re.sub(r"[^A-Za-z0-9-]", "-", os.getcwd())`); it passes no `--session` (the script then discovers the project's sessions). The mod runs it with `--dry-run --json` and never `--rollup-out`.
- **FR-006**: The pane MUST register as `Pane` with a `requestId` of its own (`cards`), poll `$.fs.stat` on the ledger's `mtimeMs` only while open, and register `/cards-pane` through `$.command.register`.
- **FR-007**: `userConfig`: `cardsPane` (boolean, default true) and `cardsPollSeconds` (number, default 5). The pane never opens by itself. `register.tsx` is restructured so each feature's composer checks its own option; `tokenStatus` false (014's switch) gates only 014, 018 and 019 and does not turn this pane off, which reads the ledger on its own.
- **FR-008**: A hook failure in this feature MUST NOT disable 014's status line (each feature registers its own hooks with its own `.catch`).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/agents` passes stories 1–4 on `terminal` and `desktop`.
- **SC-002**: `test_token_report.py` passes the `tasks --dry-run --json` cases (shape, overlap fixture, `--json` without `--dry-run` rejected, no file written), never reading real transcripts.
- **SC-003**: With the pane closed, the feature reads no file and spawns nothing; with it open and idle, it spawns nothing (a poll is one `stat`).

## Out of scope

- Timestamps or a history of the ledger in the repository (the ledger stays a hand-kept file).
- Live per-card usage while a subagent runs (014's out of scope; 014 US5 refreshes figures on demand only).
- Editing the ledger, dispatching or stopping a card from the pane.
- A budget chart, the projection verdict or the caps (014's status line and `/tokens`).
- Worktree health (git state, containers); that is `wt.py list` and 022's board. The ledger's `Branch` and `Worktree` columns are optional and unspecified by the skill; the pane shows them when present and the skill is not changed for them.
- Other agents' UIs (the pane is Claude Code-only; the ledger stays the cross-agent record).

## Assumptions

- [ASSUMPTION: The attribution groups' `start` and `end` (`token_report.py:747-748`) are the earliest and latest message timestamps of a card's subagents, so a span is when the card had a builder working, not when it was dispatched or merged.]
- [ASSUMPTION: The kit's `Pane` API and `$.ui.open` placement are as 013 uses them (Claude Code 2.1.312 pin).]

## Decisions (2026-10-31, with the maintainer)

- The pane opens by `/cards-pane` only, never by itself.
- `tasks --dry-run --json` is a prerequisite change in `token_report.py`, landed on its own branch before 020 and before 014's `/tokens --live`; 020's plan has no task for it. Its `--project` handling is specified in FR-005 and its timestamps carry a `Z` (FR-002).
- Timestamps are the pane's own observed log; no `Since` column in the ledger, and the skill is not changed.
- decided 2026-10-31 (the maintainer accepted the default): `Branch` and `Worktree` are optional ledger columns the pane shows when present; the skill does not define them.
- decided 2026-10-31 (the maintainer accepted the default): `tokenStatus` off does not turn the pane off (FR-007).
- built 2026-10-31: the pane's placement through `$.ui.open` (`id` and `requestId` `cards`, `focus: true`), its redraw from atoms, the `ui.close` hook and the `Pane` render could not be verified against a real Claude Code engine here, only through `claude plugin test` (kit 2.1.316 harness, which cannot raise `ui.close`, so closing is covered through a pane that cannot be placed). Verify by hand with `claude --plugin-dir plugins/agents` during an `agents:orchestrate` run.
- built 2026-10-31: the engine takes one handler per event without a matcher, so `register.tsx` registers `session.start` once and runs each feature's start step on its own `.catch`; a record repeated for a card in the script's JSON is one span (earliest start, latest end) with the latest record's figures, never summed (FR-001); on opening, the pane re-reads the ledger as a baseline, so changes made while it was closed are not logged (the saved log is kept).
