# Feature Specification: Workers pane shows the slots over the limit

**Feature Branch**: `feat/workers-over-limit-color`

**Created**: 2026-11-06

**Status**: Draft

**Input**: a screenshot of the Workers pane with `6 / 3 slots · 3 over the limit`, where the capacity block is the same yellow as an ordinary warning and the meter draws only the three slots of the cap. `specs/041-agent-delegation/spec.md` (FR-017, the workers pane), `plugins/project/hooks/workers-model.ts` (`capacity`), `workers-draw.tsx`.

**Depends on**: 041 (the Workers pane and `/workers`, in the `project` plugin's mod).

## Context and ground rules

- The cap is the worker-slot pool; running and stalled delegated tracks (kinds `profile` and `agent`, in their own worktrees) each hold one. Session subagents (kind `subagent`) never do: spec 041 FR-006 says the supervising session's subagents do not count against the worktree cap. The screenshot's `6 / 3` came from counting subagents as slots, which was the defect [OBSERVED: the screenshot, 6 running against a cap of 3].
- Today `capacity()` gives the whole block one tone, `warn`, drawn in the theme's `warning` colour, and the meter has `cap` cells, so six workers on three slots show three filled cells and the excess has no cell of its own [OBSERVED: `workers-model.ts`, `capacity`].
- The pane's own rule stays: colour never stands alone. Every state has a glyph or a word, and the text of the model (what `/workers` prints and what the tests compare) is the source of truth; colour is added on top by ranges of that text.
- Read-only and counts only, as the rest of the pane. No new option: the colour follows the theme's `error` tone, which is also the failed-worker tone.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See which slots are over the limit (Priority: P1)

A person with `6` workers on a cap of `3` opens the Workers pane and sees the three slots of the cap as `[■]` and the three slots beyond it as `[!]` in a different colour from the default yellow, with `3 over the limit` in that colour too.

**Why this priority**: it is the whole request; the number is already shown, the picture is what is missing.

**Independent Test**: `workers-model.test.ts` (text and ranges) and `workers.test.tsx` (the drawn segments).

**Acceptance Scenarios**:

1. **Given** cap 3 and 6 slots in use, **When** the pane is drawn wide, **Then** the meter reads `[■] [■] [■] [!] [!] [!]` with `3 over the limit` flush right, the `[!]` cells and the status in the error tone, the `[■]` cells in the normal occupied tone, and the figure line `6 / 3 slots` in the error tone.
2. **Given** cap 3 and 3 or fewer in use, **When** drawn, **Then** nothing changes: `[■]`/`[ ]` cells, `full` or `N available`, no error tone.
3. **Given** ASCII mode, **When** over, **Then** the cells are `[#]` and `[!]`, and the text is still pure ASCII.
4. **Given** a narrow pane (under 60 columns), a cap above 12, or an overflow whose cells do not fit the width, **When** over, **Then** the capacity falls back to the one-line text (`6/3 slots · 3 over the limit`) and that whole line is in the error tone.
5. **Given** no slots in use, **When** drawn, **Then** as today.
6. **Given** cap 3, two running delegated tracks and three running subagents, **When** drawn, **Then** `2 / 3 slots`, `1 available`, no `[!]` cell, no error tone, and `+3 subagents` beside the capacity label; the pipeline line reads `... slots 2/3 · +3 subagents`.
7. **Given** three delegated tracks and any number of subagents on cap 3, **Then** `full`, never over. **Given** four delegated tracks on cap 3, **Then** `1 over the limit`, whatever the subagents.
8. **Given** a cap that cannot be known (no `parallel.yaml`, unreadable or over 64 KiB, `max_parallel` of 0, non-numeric or above 99), **When** drawn, **Then** `N in use (limit unknown)` in the dim tone, no `[!]` cell, no error tone, in the pane and in the pipeline line (`... · N in use (limit unknown)`). A readable file with no `max_parallel` key uses the default of 3.

## Requirements *(mandatory)*

- **FR-001**: With a known cap and `used > cap`, the capacity figure line and the status text (`N over the limit`) use the `bad` tone, which draws as `error`, never `warning`.
- **FR-002**: In that case the meter has `used` cells: the first `cap` as today (`[■]`, `[#]` in ASCII) in the `info` tone, the `used - cap` after them as `[!]` in the `bad` tone. `[!]` is plain ASCII, so the ASCII form needs no other glyph.
- **FR-003**: When the cells do not fit the pane (`used` above 12, `cap` above 12, a pane under 60 columns, or `4 * used - 1 + 1 + status` wider than the pane) the capacity is the one-line compact text, wholly in the `bad` tone.
- **FR-004**: A `Line` can carry `marks`: ranges of its own text with their own tone. The drawer splits the text at the marks and draws one segment each in a row; a line without marks is drawn as before. The text of every line, with or without marks, is exactly what `/workers` prints.
- **FR-005**: The pipeline pane's compact line (`Workers: ... slots 6/3`) gets the `bad` tone when slots in use exceed the cap, below failed (also `bad`) and above stalled (`warn`), and its text does not change.
- **FR-006**: A snapshot with no workers and every non-over state keep their current text and tone.
- **FR-007**: `slotsUsed` counts only delegated tracks (kind `profile` or `agent`) that are running or stalled, and is the one figure behind the pane meter, the over-limit logic and the pipeline pane's compact line. Session subagents stay listed in the pane and in the totals line; their running or stalled count is shown as `+N subagents` beside the meter and never enters the used figure, an `[!]` cell or the error colour.
- **FR-008**: `parseCap` returns `null` (unknown) when the file is missing, unreadable or over the size limit, or `max_parallel` is not a whole number from 1 to 99. It returns the default (3, what `wt.py` applies) only for a readable file that has no `max_parallel` key. The snapshot carries `cap: number | null`. With a null cap the pane and every compact line say `N in use (limit unknown)` in the dim tone, never guess a number, never draw `[!]` and never use the error colour.

## Success Criteria

- **SC-001**: The three acceptance cases and the ASCII, narrow, wide-cap and no-fit fallbacks are tests, and each fails when the colour or the overflow cells are removed (mutation).
- **SC-002**: `scripts/check.sh` passes, including `claude plugin test` for the `project` plugin and `claude plugin validate`.
- **SC-003**: The text of every golden test that is not over the limit is byte-identical to before.

## Out of scope

- Changing the cap's source or any state's colour (what counts as a used slot is FR-007).
- A user option for the colour (the theme decides).
- Showing the excess on the pipeline pane beyond the existing line.
