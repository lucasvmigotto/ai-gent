# Implementation Plan: Card cost on landing

**Branch**: `docs/mod-specs-2` (spec) | **Date**: 2026-10-31 | **Spec**: `spec.md`

## Summary

Inside the `agents` module: after each ledger read (014's `refresh`, plus an immediate read after a Bash call that ran `token_report.py tasks`), compare the cards' states with the ones last seen in `$.state`, and toast each card that is newly `landed` with a used figure, with a hint when the miss is large. A documentation section in `token-economy.md` says what to do about a miss. No script change, no write.

## Technical Context

**Language**: TypeScript hooks module. **Engine**: Claude Code 2.1.312. **Events**: `session.start` and throttled `turn.complete` (014's refresh), `tool.call` (`Bash`, after `next`). **Storage**: `$.state` (the states last seen, the toasted list). **Project Type**: Claude Code plugin module plus a skill reference.

## Files

| Path | What |
|---|---|
| `plugins/agents/hooks/landing-model.ts` | pure: `landedNews(cards, seen, toasted, firstLook)` (all three are atom values), `waveSummary(wave)`, `errorPercent`, the toast and hint wording, the wave summary counts |
| `plugins/agents/hooks/landing.ts` | the atoms, the call from 014's refresh, the `tool.call` hook, the toasts |
| `plugins/agents/hooks/register.tsx` | `Options` gains `cardCostToast` and `cardCostHintPercent`; one line composing `landing(on, options)` after the existing `tokenStatus` check (so `tokenStatus` false also turns it off) and skipped when `cardCostToast` is false; 014's code otherwise unchanged. `refresh($)` and `refreshThrottled` take the options, as 018's plan adds, so the hint threshold reaches the landing code |
| `specs/019-card-cost-on-landing/data-model.md` | the atoms and the ledger cell reading |
| `plugins/agents/hooks/landing.test.tsx` | stories 1–3 and the edge cases |
| `plugins/agents/hooks/pct-fixtures.ts` | the projected/used table and its expected percentages, shared with the parity test |
| `plugins/agents/scripts/test_token_report.py` | parity: `pct_error` over the fixture table |
| `plugins/agents/types/index.d.ts` | `landingSeen`, `landingToasted`, `landingFirstLook`, `landingWave` atoms |
| `plugins/agents/.claude-plugin/plugin.json` | `userConfig` `cardCostToast`, `cardCostHintPercent` |
| `plugins/agents/skills/orchestrate/references/token-economy.md` | "Reading the landing toast" |
| `plugins/agents/skills/orchestrate/SKILL.md` | one sentence pointing at it (description unchanged, within 400 characters) |
| `README.md` | the `agents` row of the mods table |

## Testing

- `claude plugin test plugins/agents`: ledger fixtures read in sequence through the kit's `$.fs`; time is the mocked clock.
- Parity: the fixture table is the contract between `errorPercent` (TypeScript) and `pct_error` (Python), as 013's fixture is between its model and the scripts; rounding is decided here: `errorPercent` rounds half to even like Python's `{:+.0f}`, the table has exact `.5` cases, and `pct_error` is fed the same parsed cell values (not the exact figures, which only `tracker.md` uses).
- `scripts/check.sh` verifies the new link in `SKILL.md` resolves.

## check.sh / CI impact

None beyond the existing plugin test and script test.

## Risks

- **Toast storm**: a resumed run or a hot reload. Mitigated by the first-look set and the three-toast cap.
- **Figure lag**: the used figure appears only after `tasks`. Mitigated by the immediate read after the `tasks` Bash call; a coordinator that forgets `tasks` gets no toast, which is itself the signal the skill's token step was skipped.
- **Overlap with 014's overrun toast**: a card over projection while running toasts once (014); at landing it toasts again with the final figure. Different moments, different text; they stay separate (spec Decisions).
- **Ledger shape drift**: shares 014's `parseLedger`, so one place to fix.

## Constitution Check

N/A — no constitution.

## Implementation tasks (tests first)

1. Parity fixture and test: `errorPercent` cases against `pct_error`; then `errorPercent`.
2. Pure tests for `landedNews`: first look, transition, delayed figure, no projection, fix round, vanished row, several at once; then `landing-model.ts`.
3. Pure tests for the toast and hint wording and the wave counts; then the wording code.
4. Plugin tests (`landing.test.tsx`): the landing toast, the hint, the wave toast, the `tasks` Bash trigger, the off switch; then `landing.ts`, the atoms and the wiring in `register.tsx`.
5. `userConfig` and `types/index.d.ts`.
6. Write the `token-economy.md` section (it records the 25% default, since the file has no tolerance figure today) and the `SKILL.md` sentence; README row; `scripts/check.sh`.
