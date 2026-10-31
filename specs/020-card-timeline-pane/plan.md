# Implementation Plan: Card timeline pane

**Branch**: `docs/mod-specs-2` (spec) | **Date**: 2026-10-31 | **Spec**: `spec.md`

## Summary

A `Pane` drawn by the `agents` plugin's hooks module: a snapshot of the ledger's cards (via 014's `parseLedger`, extended for `Branch` and `Worktree`), the state transitions the mod observed (a capped log in `$.state`), and, on demand, spans from `token_report.py tasks --dry-run --json`. A `stat` poll refreshes it while open; `/cards-pane` opens it or prints its text form. `token_report.py tasks --dry-run --json` is the prerequisite change (its own branch, landed first); this plan only consumes it.

## Technical Context

**Language**: TypeScript hooks module; Python 3.9+ stdlib for the script flag. **Engine**: Claude Code 2.1.312. **Events**: `ui.render` (`Pane`, `requestId: 'cards'`), `command.run`, `ui.close`; `session.start` only to register the `/cards-pane` command through `$.command.register` (the pane never opens by itself). **Storage**: `$.state` (open flag, last seen states, the log, the last measurement). **Project Type**: Claude Code plugin module plus a CLI flag.

## Prerequisite script change (`plugins/agents/scripts/token_report.py`, its own branch, landed before this feature and before 014's `/tokens --live`)

`build_parser` (`tasks` subparser): `--json`. `cmd_tasks`: with `--json`, require `--dry-run` (else `p.error(...)`, exit 2, at argument parsing; `build_parser` is where it can raise it), and print one object from the existing `groups` and `by_card`/`records` data (`card`, `feature`, `model`, `projected`, `used`, `start`, `end` as UTC with a `Z`, `agents`, `calls`) instead of the text table. `--project` stays required: the mod derives the slug like `cmd_budget` (FR-005). Nothing else in `cmd_tasks` changes; the `--dry-run` branch already writes nothing.

## Files

| Path | What |
|---|---|
| `plugins/agents/hooks/register.tsx` | `Options` gains `cardsPane` and `cardsPollSeconds`; restructured so 014/018/019 compose after the `tokenStatus` check and `cardsPane(on, options)` composes on its own `cardsPane` check (not behind `tokenStatus`); 014's code otherwise unchanged |
| `plugins/agents/hooks/cards-model.ts` | pure: states and counts, transition detection, the capped log, interval overlap and peak, the axis scale |
| `plugins/agents/hooks/cards-view.ts` | pure: the snapshot as lines (wide, narrow, text form) |
| `plugins/agents/hooks/cards.ts` | everything that needs `$`: the ledger read, the `stat` poll, the measurement run, open/close, the atoms |
| `plugins/agents/hooks/cards-fixtures.ts` | a ledger and a `tasks --json` output with the expected rows, overlaps and peak |
| `plugins/agents/hooks/cards.test.tsx` | stories 1–4, `$.process.run` mocked |
| `plugins/agents/hooks/token-model.ts` | `LedgerCard` gains optional `branch` and `worktree`; existing fields and tests unchanged |
| `plugins/agents/types/index.d.ts` | the `cards` atoms |
| `plugins/agents/.claude-plugin/plugin.json` | `userConfig` `cardsPane`, `cardsPollSeconds` |
| `plugins/agents/scripts/test_token_report.py` | (prerequisite change) `tasks --dry-run --json` cases |
| `shared/token-tracking.md` | (prerequisite change) one line documenting `tasks --json` |
| `README.md` | the `agents` row of the mods table |

No change to `plugins/agents/skills/orchestrate/SKILL.md`: the `Since` column was decided against, and `Branch`/`Worktree` stay optional columns it does not define. The `token_report.py` and `shared/token-tracking.md` rows marked prerequisite ship in the prior change, not with this feature.

## Testing

- `claude plugin test plugins/agents`: the pane mounted on `terminal` and `desktop` against the fixture; the poll through the mocked clock and `$.fs`; the measurement through a mocked `$.process.run`.
- `test_token_report.py`: `--json` shape and read-only guarantee (a fake `HOME`, the working tree unchanged after the run).
- Contract: the fixture's JSON keys are what the script prints; `test_token_report.py` asserts the same key list, so a renamed field breaks one side loudly.
- Manual: `claude --plugin-dir plugins/agents --debug` during a real `agents:orchestrate` run.

## check.sh / CI impact

None beyond the existing plugin and script tests.

## Risks

- **API churn** (early access): `Pane` props and `$.ui.open` may move; keep the model and view pure and separate from drawing (013's mitigation).
- **Cost**: `--measure` reads all transcripts; user-triggered only, one at a time, with a timeout.
- **Observed-only log** is partial by design; the pane says since when. If that is too thin, a `Since` ledger column (decided against for now) is the fix, not guessing.
- **Two readers of the ledger** (014 and this): both go through `parseLedger`, so one place to change when the shape does.
- **A third feature in one module** (014, 019, 020): each registers its own hooks with its own `.catch`, so one failure leaves the others working.

## Constitution Check

N/A — no constitution.

## Implementation tasks (tests first)

0. **Prerequisite (separate change, already landed):** `tasks --dry-run --json` (shape, `Z` timestamps, `--json` alone rejected by the parser, nothing written) and its `shared/token-tracking.md` line.
1. Tests for the mod's slug derivation (a cwd with spaces and dots becomes the `--project=-…` form `cmd_budget` computes).
2. Extend `parseLedger` tests for `Branch` and `Worktree` (014's cases unchanged); then the code.
3. Pure tests for `cards-model.ts` (counts, transitions, the 200-entry cap, overlap and peak, axis scale, the minimum bar width); then the model.
4. Pure tests for `cards-view.ts` (wide, narrow under 60 columns, text form, with and without a measurement); then the view.
5. Plugin tests (`cards.test.tsx`): open, poll and redraw, log, Measure success, failure and timeout, the closed pane reading nothing, the no-ledger reply; then `cards.ts` and the wiring in `register.tsx`.
6. `userConfig`, `register.tsx` options threading and `types/index.d.ts`; the README row; `scripts/check.sh`.
