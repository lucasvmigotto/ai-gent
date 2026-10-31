# Implementation Plan: Token burn rate and ETA

**Branch**: `docs/mod-specs-2` (spec) | **Date**: 2026-10-31 | **Spec**: `spec.md`

## Summary

`token_report.py budget` computes a rate over the last `rate_window_minutes` from the messages it already collects for the week, and per cap an `eta_minutes`; it prints one `rate:` line and adds `rate` and `caps[].eta_minutes` to `--json`. The `agents` module's existing refresh (014) formats them into the status line and toasts a cap whose ETA crosses `tokenEtaWarnMinutes`. No new spawn, no new scan.

## Technical Context

**Language**: Python 3.9+ stdlib for the script; TypeScript hooks module. **Engine**: Claude Code 2.1.312. **Events**: none new; it rides 014's `session.start`, throttled `turn.complete` and `/tokens`. **Storage**: `$.state` (one flag per cap). **Project Type**: Claude Code plugin module plus a CLI field.

## Script change (`plugins/agents/scripts/token_report.py`)

| Where | Change |
|---|---|
| `usage_since` (`:1583`) | split into `usage_messages(cutoff_iso, dirs)` returning the deduplicated `(t, _, u)` list and `usage_since`, which sums it (behavior unchanged, tested) |
| new `usage_rate(msgs, now, minutes)` | `(per_hour, n)` from the messages whose `t` is in the window, billable by `bill(u)`; `now` is a parameter so the unit tests freeze it |
| `BUDGET_DEFAULTS` | `rate_window_minutes: 30` |
| new constant | `MIN_RATE_MESSAGES = 3` |
| `cmd_budget` (`:1661`) | collect the week once with `usage_messages`; derive `used_week`, the burn and, when `session_window_hours` is set, `used_session` (cutoff on the same list) from it, so that branch no longer calls `usage_since` a second time (that was an extra scan; the plain-session branch uses `gather` as before). Name the new value `burn`: `cmd_budget` already has a local `rate, rate_src = event_rate(history)` (the event-risk rate) and the JSON key `rate` is built from `burn` at print time. Per cap `eta_minutes`; the `rate:` text line; `--json` fields |

## Files

| Path | What |
|---|---|
| `plugins/agents/scripts/token_report.py` | as above (Backend) |
| `plugins/agents/scripts/test_token_report.py` | rate cases under the fake `HOME` |
| `plugins/agents/hooks/token-model.ts` | `Budget` type gains `rate` and `caps[].eta_minutes`; `fmtEta`; `statusText` appends the ETA; `etaNews` (pure: which caps crossed); `fmtRate` (`7.40M/h`, from `fmtTok`) |
| `plugins/agents/hooks/tokens.ts` | `refresh($, options)` and `refreshThrottled` gain an options argument (`tokenBurnRate`, `tokenEtaWarnMinutes`); `refresh` calls `etaNews`, toasts, updates the flags |
| `plugins/agents/hooks/register.tsx` | the `Options` type gains `tokenBurnRate` and `tokenEtaWarnMinutes`; they are passed to every `refresh` call (`session.start`, `turn.complete`); `tokenStatus` false still returns first and so also turns this feature off (it is part of the token status) |
| `plugins/agents/hooks/tokens.test.tsx` | stories 1–3 (mocked `$.process.run`) |
| `plugins/agents/types/index.d.ts` | `tokenEtaWarned: string[]` (caps under the threshold) |
| `plugins/agents/.claude-plugin/plugin.json` | `userConfig` `tokenBurnRate`, `tokenEtaWarnMinutes` |
| `shared/token-tracking.md` | the `rate` and `eta_minutes` fields and `rate_window_minutes` in `budget.yaml` |
| `README.md` | the `agents` row of the mods table |

No new hooks file: the feature is a few lines of 014's module.

## Testing

- `python3 -m unittest plugins/agents/scripts/test_token_report.py`, already in `scripts/check.sh`.
- `claude plugin test plugins/agents`, already wired.
- `usage_rate` unit tests pass a frozen `now`. `cmd_budget` computes `now` itself, so its cases build transcripts with timestamps relative to the real clock, as the existing tests do (`test_token_report.py:614-616`); no clock injection into `cmd_budget`.
- `usage_since`'s behavior is pinned by a regression test before the split.

## check.sh / CI impact

None: the existing script test and plugin test cover it. `scripts/consistency.py` is unaffected (no tag or card change).

## Risks

- **Misleading ETA**: a short burst reads as a high rate. Mitigated by the window average, the message minimum and the `~`; the toast says "at this rate".
- **Account-wide vs session**: decided account-wide (spec Decisions); a session-scoped rate would need a second pass over `gather` and is not built.
- **Refactoring `usage_since`**: it feeds the week figure; the regression test comes first.

## Constitution Check

N/A — no constitution.

## Implementation tasks (tests first)

1. Regression test pinning `usage_since` on a fake `HOME`; then split it into `usage_messages` and the sum.
2. Tests for `usage_rate` (steady, idle, below `MIN_RATE_MESSAGES`, window override); then `usage_rate`, the `BUDGET_DEFAULTS` key and the constant.
3. Tests for `budget` text and `--json` (`rate`, `eta_minutes`, null cases, existing keys unchanged); then `cmd_budget`.
4. Pure tests for `fmtEta`, `statusText` with ETAs and `etaNews` (crossing, repeat, recovery, `left` 0); then the model code.
5. Plugin test for the status and the toast in `tokens.test.tsx`; then wire `refresh` and the `$.state` flags.
6. `userConfig`, `register.tsx` options threading and `types/index.d.ts`; document in `shared/token-tracking.md` and the README row; `scripts/check.sh`.
