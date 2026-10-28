# Feature Specification: Token status line

**Feature Branch**: `docs/mod-specs` (specification only; built on its own branch)

**Created**: 2026-10-28

**Status**: Draft

**Input**: A Claude Code mod that keeps token tracking (`shared/token-tracking.md`) in view: a status-line entry with the cards in flight, their `Tokens: projected` against what they used, and the budget left; a toast when a card passes its projection or when the wave no longer fits the budget. It runs `plugins/agents/scripts/token_report.py` through `$.process.run` and reads the orchestration ledger through `$.fs`.

**Depends on**: 012 (checkpoint mod) — module layout, `check.sh` wiring, versioning and install, reused unchanged.

## Context and ground rules

- `token_report.py` stays the only thing that measures, attributes and judges a budget. The mod formats what the script and the ledger already say; it computes no token figure of its own and writes nothing (`tasks` keeps writing `tasks.md`, `tracker.md`, the ledger and the history).
- Owner: the `agents` plugin. Module `plugins/agents/hooks/register.tsx`, named in a new `plugins/agents/hooks/hooks.json` (the plugin has no hooks today).
- Token tracking is Claude Code-only already (it reads Claude Code's transcripts); the mod adds no new limitation.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See the cards in flight and their cost (Priority: P1)

During an `agents:orchestrate` run, the status line under the prompt reads, for example, `tokens · 004-us1-core ~420k/391k · 004-us2 ~300k/— · session left 3.1M · week left 22M`.

**Why this priority**: the coordinator and the user both decide waves on these numbers; today they need a command to see them.

**Independent Test**: a `claude plugin test` case with a fixture `.ai-gent/orchestrate-status.md` and a mocked `$.process.run` answering `budget --json` asserts the status text.

**Acceptance Scenarios**:

1. **Given** `.ai-gent/orchestrate-status.md` with a card table (a `Card` column, backticked ids, a state, a `Tokens` cell `~420k / 391k`), **When** the session starts or a turn completes, **Then** the status lists the cards whose state is not done, with their Tokens cell as written.
2. **Given** `.ai-gent/budget.yaml` with `session_billable` and `week_billable`, **When** the status refreshes, **Then** it shows what `token_report.py budget` reports as left for each cap.
3. **Given** no ledger and no budget file, **When** the session starts, **Then** no status is shown (`$.ui.status(undefined)`).
4. **Given** a budget file but no ledger, **When** refreshed, **Then** only the budget part shows.

---

### User Story 2 - Be told when a wave stops fitting (Priority: P1)

When the in-flight cards' projections no longer fit what is left, the user gets a toast with the verdict.

**Why this priority**: rstory hit the session limit twice; the budget check only helps if it is seen before the limit.

**Independent Test**: mocked `budget --json` answering `shrink`, then `stop`; assert one toast each and the status's warning mark.

**Acceptance Scenarios**:

1. **Given** in-flight cards `004-a,004-b`, **When** `budget --cards 004-a,004-b --json` answers `shrink` (the first card fits), **Then** a toast says `budget: shrink — only 004-a fits; run token_report.py budget for the batches`, and the status line gains `⚠ shrink`.
2. **Given** the verdict was already toasted, **When** it is unchanged at the next refresh, **Then** no new toast (one per verdict change).
3. **Given** the verdict goes back to `proceed`, **When** refreshed, **Then** the mark clears.

---

### User Story 3 - Be told when a card passes its projection (Priority: P2)

**Acceptance Scenarios**:

1. **Given** a ledger Tokens cell whose used figure exceeds its projected one, **When** the ledger changes, **Then** one toast per card: `004-us1-core used 520k, projected ~420k (+24%)`.
2. **Given** a cell `~420k / —` (not measured yet), **When** read, **Then** no comparison is made.

---

### User Story 4 - Refresh on demand (Priority: P3)

`/tokens` re-runs the budget, re-reads the ledger and prints the full `budget` text in the transcript as the command's output (not a model turn).

---

### User Story 5 - Refresh used figures live (Priority: P3, projected)

Projected for after the first cut. `/tokens --live` runs `token_report.py tasks --dry-run` (read-only) before the budget, so the used figures are current instead of as of the last `tasks` run. It is never run on a timer, because it reads every transcript of the project and is slow on large runs.

**Acceptance Scenarios**:

1. **Given** cards that used more since the last `tasks` run, **When** `/tokens --live` runs, **Then** the status and the printed text show the current figures, and the ledger file is not written.
2. **Given** a run that takes longer than the `userConfig` timeout, **When** `/tokens --live` runs, **Then** it reports the timeout and keeps the recorded figures.

### Edge Cases

- **No python3** or the script fails (non-zero exit, timeout): the status shows `tokens · unavailable` once and the reason goes to the debug log; no toast storm.
- **Malformed ledger** (no `Card` column, cells missing): cards are skipped; the budget part still shows.
- **No caps** in `budget.yaml`: `budget` says "no caps"; the status shows the cards only.
- **Cost of refreshing**: `budget` reads transcripts (the week's usage counts every project). Refreshes run on `session.start`, on `turn.complete` at most once per `userConfig` interval (default 120 s), and on `/tokens`; never on a timer while idle, never per tool call.
- **Account switches**: the script counts `CLAUDE_CONFIG_DIR`'s account; the mod passes the session's environment unchanged and does not second-guess it.
- **Narrow terminal**: the status text is cut to the cards that fit, then `+N more`; the budget part is kept first.
- **Non-terminal surfaces**: `$.ui.status` and `$.ui.toast` exist on all surfaces; where the transcript is printed into scrollback the toast is a notification-bar line. No pane.
- **Hot reload**: the last verdict and the cards already toasted are in `$.state`, so a reload does not repeat toasts; the status is recomputed.
- **Headless (`claude -p`)**: no status line or toast; the module does nothing.
- **Not an orchestrate run**: with no ledger, only the budget shows, and only if `budget.yaml` exists.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Every figure shown MUST come from the ledger's Tokens cells or from `token_report.py budget` output; the mod MUST NOT compute billable tokens itself.
- **FR-002**: `token_report.py budget` MUST gain a `--json` flag printing the verdict, the caps (`cap`, `used`, `left` per cap), the cards with their projections, and the fitting count — the text output unchanged. Tested in `plugins/agents/scripts/test_token_report.py`.
- **FR-003**: The mod MUST find the script relative to its own plugin root (`../scripts/token_report.py` from `hooks/`), never on `PATH`.
- **FR-004**: In-flight cards MUST be the ledger rows in state `queued`, `running` or `review`, the fixed vocabulary `agents:orchestrate`'s ledger adopts before this feature (`queued`, `running`, `review`, `landed`). A row with any other state is skipped and logged to the debug log.
- **FR-005**: Toasts MUST fire once per verdict change and once per card overrun per session (tracked in `$.state`). A `stop` verdict MUST also raise a native notification (`$.ui.notify`, title `Token budget`), once per change to `stop`.
- **FR-006**: A `userConfig` boolean (`tokenStatus`, default true) and a number (`tokenRefreshSeconds`, default 120) MUST control it.
- **FR-007**: The mod MUST register `/tokens`, answering with the `budget` text.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/agents` passes, covering stories 1–4 on `terminal` and `desktop`.
- **SC-002**: `test_token_report.py` passes with the new `--json` case, against a fake `HOME` (never real transcripts).
- **SC-003**: In a 30-minute session with no turns, the mod spawns `token_report.py` once (at start).

## Out of scope

- Running `tasks` (it writes files; the coordinator runs it after a landing).
- Live per-card usage while a subagent runs (the ledger's used figure is as of the last `tasks`).
- A pane or chart of history; `estimate`.
- Changing `shared/token-tracking.md` beyond documenting `budget --json`.

## Decisions (2026-10-28, with the maintainer)

- Used figures: as of the last `token_report.py tasks` run (no live refresh in the first cut). Live refresh is projected as User Story 5 (`/tokens --live`, P3), above.
- Ledger states: `agents:orchestrate`'s ledger gets a fixed vocabulary, `queued`, `running`, `review`, `landed`, as a separate skill change landed before this feature; FR-004 then matches it exactly instead of guessing.
- A `stop` verdict raises a toast and a native notification (`$.ui.notify`, title `Token budget`); a `shrink` verdict, a toast only.

## Assumptions

- [ASSUMPTION: The ledger keeps the card table shape `write_ledger` in `token_report.py` reads (a `Card` header cell, backticked ids).]
- [ASSUMPTION: `budget`'s run time stays within `$.process.run`'s 30 s default on a typical machine; the mod passes a 60 s timeout.]
