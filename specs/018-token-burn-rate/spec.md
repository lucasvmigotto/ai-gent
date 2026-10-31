# Feature Specification: Token burn rate and ETA

**Feature Branch**: `docs/mod-specs-2` (specification only; built on its own branch)

**Created**: 2026-10-31

**Status**: Accepted

**Input**: Add a burn rate and an ETA to the token status line (014): how fast the account is spending billable tokens now, and how long what the session and week caps leave lasts at that pace (`session left 3.1M (~2h10m) · week left 22M (~15h)`), with a toast when the nearest cap is about to run out. The rate and the ETA are computed by `plugins/agents/scripts/token_report.py budget --json`, not by the mod.

**Depends on**: 014 (token status) — the status line, the refresh and throttle, `/tokens`, `budget --json`, all built (`plugins/agents/hooks/tokens.ts`, `token-model.ts`; `token_report.py` `cmd_budget`). 014's User Story 5 (`/tokens --live`) is **projected, not built** (`tokens.ts` has no `--live`); this feature does not need it.

## Context and ground rules

- Today `budget --json` says what the caps leave (`caps[].left`) but not how long that lasts; the coordinator and the user learn it by hitting the limit (the rate-limit stalls in the orchestration history).
- `token_report.py` stays the only thing that measures and judges. A slope is a measurement over timestamps the script already reads (`usage_since` collects each message's timestamp and usage, `token_report.py:1583`), so the rate belongs in the script and the mod only formats `rate` and `eta_minutes` (FR-001, FR-004). The mod divides nothing.
- Read-only: the mod writes nothing; the script change adds fields and one text line, no file.
- Owner: the `agents` plugin; the same module and `userConfig` as 014. Claude Code-only, like token tracking.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See how long the budget lasts at this pace (Priority: P1)

During a run the status line shows, after each cap, the time it lasts at the current rate: `tokens · 004-us1-core ~420k/391k · session left 3.10M (~2h10m) · week left 22.00M (~15h)`.

**Why this priority**: "3.1M left" means nothing without the pace; "~2h10m" tells the coordinator whether the next wave is worth starting before the window ends.

**Independent Test**: a `claude plugin test` case with a mocked `$.process.run` answering `budget --json` with `rate` and `eta_minutes` fields asserts the status text.

**Acceptance Scenarios**:

1. **Given** `budget --json` answers caps with `eta_minutes` 130 (session) and 900 (week), **When** the status refreshes, **Then** it reads `session left 3.10M (~2h10m) · week left 22.00M (~15h)`.
2. **Given** `eta_minutes` is null for a cap (no cap set, or no usage in the rate window), **When** it refreshes, **Then** that cap shows its `left` figure with no parenthesis.
3. **Given** an ETA over 48 hours, **When** it refreshes, **Then** it reads `(>2d)`.
4. **Given** `tokenBurnRate` is false, **When** it refreshes, **Then** the status is 014's, unchanged.

---

### User Story 2 - Be told before a cap runs out (Priority: P1)

When the nearest cap's ETA falls under a threshold (default 30 minutes), a toast says which cap and for how long.

**Why this priority**: the status line is easy to miss; the limit is not.

**Independent Test**: mocked `budget --json` answering ETAs 60, then 25, then 25, then 90, then 20; assert one toast at the first crossing, none on the repeat, and one again after the ETA went back above.

**Acceptance Scenarios**:

1. **Given** the session cap's ETA drops from 60 to 25 minutes, **When** refreshed, **Then** one toast: `session cap lasts ~25m at 7.40M/h — pause the wave or finish the card in flight`.
2. **Given** the ETA stays under the threshold at the next refresh, **When** refreshed, **Then** no second toast (one per crossing, per cap).
3. **Given** the ETA rises above the threshold and later falls under it again, **When** refreshed, **Then** a new toast.
4. **Given** a cap with `left` 0, **When** refreshed, **Then** no ETA toast (014's `stop` toast and notification already fire).

---

### User Story 3 - See the rate in `/tokens` (Priority: P2)

`/tokens` prints, with the script's `budget` text, one more line: `rate: 7.40M/h over the last 30 min (112 messages, all projects)`.

**Independent Test**: a `test_token_report.py` case under a fake `HOME` with transcripts spanning an hour asserts the line and the JSON `rate` object.

**Acceptance Scenarios**:

1. **Given** transcripts with billable usage in the last 30 minutes, **When** `budget` runs, **Then** the text has the `rate:` line and `--json` has `rate: {per_hour, window_minutes, messages}`.
2. **Given** no usage in the window, **When** `budget` runs, **Then** the line reads `rate: idle for 30 min` and `rate.per_hour` is 0.

### Edge Cases

- **Idle or just started**: fewer than `MIN_RATE_MESSAGES` (Planned, 3) messages in the window → `rate` is null and no ETA is shown; one message after a quiet hour must not read as a rate.
- **Bursts**: the rate is a window average (default 30 minutes, `rate_window_minutes` in `.ai-gent/budget.yaml`), not an instant; several parallel subagents make it jump when a wave starts. The ETA is an estimate and the status says `~`.
- **The caps' definitions**: the week cap counts the last 7 days across projects, and the session cap counts the session (or `session_window_hours`, a rolling window whose old usage ages out). The ETA divides `left` by the current rate and ignores that aging, so it is conservative for a rolling session window; documented, not modelled.
- **Account-wide rate**: the rate counts every project of the account's config dirs (the same messages the week figure uses, no second scan), so another project's burn shortens the ETA. That matches how the real limits behave.
- **Account switch**: the script already deduplicates messages by id across config dirs (`usage_since`); the rate inherits it.
- **No `budget.yaml` / no caps**: nothing to divide into; no ETA, the rate line still prints in `/tokens`.
- **Script failure**: 014's `tokens · unavailable` path; no ETA, no toast.
- **Cost**: no extra spawn and no extra scan; the rate is computed from the messages `budget` already collected for the week, and the refresh throttle is 014's.
- **Headless**: nothing, as in 014.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `token_report.py budget` MUST compute the rate from the billable usage of the messages it already collects for the week: `per_hour` over the last `rate_window_minutes` (default 30), the number of messages in the window, and per cap `eta_minutes` = `left / per_hour × 60`, rounded, or null when there is no cap, `per_hour` is 0 or fewer than `MIN_RATE_MESSAGES` messages fell in the window.
- **FR-002**: `budget --json` MUST add `rate` (`per_hour`, `window_minutes`, `messages`, or null) and `eta_minutes` to each `caps[]` entry; existing keys (`verdict`, `why`, `fit`, `total`, `need`, `caps`, `cards`, `batches`) are unchanged, so 014's parser keeps working.
- **FR-003**: The `budget` text MUST gain one line, `rate: …`, after the caps; no other text line changes. `test_token_report.py` covers it.
- **FR-004**: The mod MUST NOT compute a rate, an ETA or any token figure; it formats `eta_minutes` as `~2h10m` (under an hour `~25m`, over 48 hours `>2d`) and `per_hour` as `7.40M/h` with 014's `fmtTok` plus `/h` (the script's `fmt_tok` gives the same text for the `rate:` line).
- **FR-005**: The toast MUST fire once per crossing of `tokenEtaWarnMinutes` per cap (a cap's flag is cleared when its ETA is above the threshold or null), tracked in `$.state`; it MUST NOT fire for a cap with `left` 0.
- **FR-006**: `userConfig`: `tokenBurnRate` (boolean, default true) and `tokenEtaWarnMinutes` (number, default 30; 0 turns the toast off and keeps the status). The rate window is a key of `.ai-gent/budget.yaml`, next to the caps, not a `userConfig`.
- **FR-007**: The mod MUST add no `token_report.py` spawn: it reads the new fields from 014's `budget --json` run.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/agents` passes the burn-rate cases (status format, threshold toast once per crossing, off switch) on `terminal` and `desktop`.
- **SC-002**: `test_token_report.py` passes the rate cases (steady rate, idle, below `MIN_RATE_MESSAGES`, no cap, `rate_window_minutes` override) against a fake `HOME`, never real transcripts.
- **SC-003**: With the feature on, the number of `token_report.py` spawns in a session equals 014's (SC-003 there), and a `budget` run reads the same files as before.

## Out of scope

- Predicting when the wave will finish or when the window resets (the script has no model of either).
- A per-project or per-agent rate (the rate is the account's, like the week cap).
- A chart of the rate over time.
- `/tokens --live` (014 US5) and the live per-card figures; the rate does not depend on them.
- Changing the caps, the verdict rule or `budget.yaml`'s other keys.

## Assumptions

- [ASSUMPTION: Message timestamps in the transcripts are UTC ISO strings, as `usage_since`'s cutoff comparison already assumes.]
- [ASSUMPTION: The messages `usage_since` collects for the week include every message of the last `rate_window_minutes`, so one collection serves both figures.]

## Decisions (2026-10-31, with the maintainer)

- The rate is account-wide (the week figure's scope), computed from the messages `budget` already collects; no second pass over the transcripts. A session-scoped rate is not built.
- The ETA warning defaults to 30 minutes (`tokenEtaWarnMinutes`, a `userConfig` option; 0 turns the toast off). Toast only, no native notification; a `stop` verdict keeps its own, from 014.
- The rate window defaults to 30 minutes and is the `rate_window_minutes` key of `.ai-gent/budget.yaml`, next to the caps, not a `userConfig` (FR-006). decided 2026-10-31 (the maintainer accepted the default): the earlier line that called it a `userConfig` option contradicted FR-006 and the data model; the key was kept because the script, not the mod, owns the window.
- `budget --json` keeps all of 014's keys; `rate` and `caps[].eta_minutes` are additions only.
