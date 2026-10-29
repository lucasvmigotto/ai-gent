# Implementation Plan: Token status line

**Branch**: `docs/mod-specs` (spec) | **Date**: 2026-10-28 | **Spec**: `spec.md`

## Summary

The `agents` plugin gets a hooks module that, on `session.start`, throttled `turn.complete` and `/tokens`, reads `.ai-gent/orchestrate-status.md` with `$.fs`, runs `python3 <plugin>/scripts/token_report.py budget --cards <in flight> --json` with `$.process.run`, and sets `$.ui.status` / `$.ui.toast`. `token_report.py` gains `budget --json`.

## Technical Context

**Language**: TypeScript hooks module; Python 3.9+ stdlib for the script change. **Engine**: Claude Code 2.1.312. **Events**: `session.start`, `turn.complete`, `command.run` (`$.command.register`). **Storage**: `$.state` (last verdict, toasted overruns, last refresh time). **Project Type**: Claude Code plugin module plus a CLI flag.

## Files

| Path | What |
|---|---|
| `plugins/agents/hooks/hooks.json` | new: `{ "modules": ["./register.tsx"] }` |
| `plugins/agents/hooks/register.tsx` | module entry |
| `plugins/agents/hooks/token-model.ts` | pure: the ledger's columns, the figures, the status line, what is news |
| `plugins/agents/hooks/tokens.ts` | the reading (`$.fs`, `$.process.run`), the atoms, the refresh and throttle, `/tokens` |
| `plugins/agents/hooks/tokens.test.tsx` | stories 1–4, `$.process.run` mocked |
| `plugins/agents/types/index.d.ts` | state contract (`tokens.*` atoms); `plugin.json` `"types"` |
| `plugins/agents/.claude-plugin/plugin.json` | `userConfig` `tokenStatus`, `tokenRefreshSeconds`; `"types"` |
| `plugins/agents/scripts/token_report.py` | `budget --json` (Backend) |
| `plugins/agents/scripts/test_token_report.py` | a `budget --json` case under the fake `HOME` |
| `shared/token-tracking.md` | one line documenting `--json` |

## Testing

- `claude plugin test plugins/agents` (wired by 012 for every plugin with a module).
- `python3 -m unittest plugins/agents/scripts/test_token_report.py`, already run by `scripts/check.sh`.
- The mod's tests never spawn the real script: `$.process.run` is mocked with recorded `budget --json` outputs, so they need no transcripts.

## check.sh / CI impact

- `agents` becomes a plugin with a module, so 012's loop picks up its tests; no new step.
- `scripts/consistency.py` already checks cards and tags; unaffected.

## Risks

- **API churn** (early access): `turn.complete`'s input and `$.ui.status` may change. Validate in check.sh; keep the parsing pure and separately tested.
- **Cost**: `budget` scans a week of transcripts across projects. Throttled refresh; live usage is `/tokens --live` (User Story 5, projected), never on a timer.
- **Ledger shape**: the ledger is hand-kept by the coordinator except its Tokens column. Prerequisite: `agents:orchestrate` fixes its state vocabulary (`queued`, `running`, `review`, `landed`) in its own change, landed before this feature; rows with another state are skipped.
- **Exposure**: the status shows only figures and card ids, never prompts or descriptions from transcripts.

## Constitution Check

N/A — no constitution.
