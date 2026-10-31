# Feature Specification: Session chrome (workspace line and context meter)

**Status**: Implemented

**Input**: The prompt area stacks one band per plugin (holds, hygiene, CI, release, containers) and the context figure is only a toast at 80%. Fold the quiet bands into one compact Workspace line, and show the context window's fill under the prompt.

**Depends on**: 021 (the context threshold), 030 (band priority and cap).

## Scope

Only the workspace status above the prompt and one line under it. Out of scope and untouched: the Pipeline pane (`plugins/project/hooks/pipeline*`), the native prompt, the native auto-mode row, native agent management. Every command, each plugin's data collection, refresh, severity and priority, and every existing test stay.

## What the engine allows (types 2.1.316)

| Wanted | Possible | How |
|---|---|---|
| One compact line above the prompt | yes | `AbovePrompt` (`bodyColumns`, `maxRows`, Buttons) |
| Show/hide details with a native Button | yes | Button `key`, `onPress`, an atom for the state |
| Context meter below the prompt | partly | `PromptHint.tail`: dim text appended to the engine's hint line, cut where the row ends, left out under four columns. No colour, no tree of ours under the prompt |
| Green/amber/red meter | no (colour) | `tail` is a string drawn dim; the level is carried by a glyph and a word (`⚠ approaching`, `✖ critical`; ASCII `!`, `!!`) |
| A bar following the width | yes | the tail is sized from `viewport.columns` less the hint's width |
| Agent activity (running/waiting/done/failed, duration, tokens) | no | no render component for the native agent list and no hook data on running agents is exposed (`ToolProgress` and `Spinner` are per-tool rows owned by the engine). Recorded as a limitation, nothing built |
| Auto-mode row restyle | no | `SessionMode` exposes only label strings (`modes`), drawn dim by the engine; its amber identity cannot be set or kept by a hook. Untouched |
| Branch / background-agent rows | no | no component |
| Plugin-update count | no | the update notice reaches the person only as a toast text; no count is recorded in any atom, so none is invented |
| Container count | no | `leftovers.sh --json` reports leftovers and disk, not running containers; the summary counts leftovers |

## Requirements

- **FR-001** A band record may carry an optional `summary` (`{ text, tone: ok | info | warn }`), plain words with no glyph. It is part of the shared `band-priority.ts` contract; `parseBands` drops a malformed one and keeps the band.
- **FR-002** Only a `normal` or `info` band with a summary can fold (`isFoldable`). A `hold` or a `failure` never folds, nor does a band without a summary (a low disk records none).
- **FR-003** The project plugin draws the Workspace line when `workspaceLine` is on (default) and it has declared itself leader in the `workspaceLeader` atom; the other band plugins read that atom and, while it is true, do not draw their folded bands alone. Absent, older or failing reads mean false: every band draws as before.
- **FR-004** Folded bands do not count toward `bandCap`; the cap applies to the rest. `/bands` lists a folded band as `in the workspace line`.
- **FR-005** The line: `Workspace: ✓ CI passing · Release 2.23.0` (ok/info segments) and a warnings line `⚠ main behind origin · 3 container leftovers`. Wide (80+) carries the `Workspace:` label; narrower drops it; segments that do not fit become `+N`, a first segment that does not fit is cut with an ellipsis. A Details button (`▸`/`▾`, ASCII `>`/`v`) lists every folded band whole.
- **FR-006** Visibility: checkpoint holds, failed CI and low disk always draw as their own bands (never folded, never behind Details). Passing/running CI, the next release and a repo-hygiene summary fold. Container leftovers fold as one counted warning. A branch behind or diverged from its remote is a warning. Nothing active draws nothing.
- **FR-007** Context meter (opt-in `contextMeter`, default off because it adds to the engine's hint line): `Context ████░░░░ 43% used · 57% remaining`, the bar and the remaining text dropping as the width shrinks, down to `ctx 43%`. The figure is `$.session.usage().context.percent`, read once at session start and at each turn end, kept in the `contextPercent` atom and drawn from it (drawing reads no engine, nothing polls). When the hand-off nudge is on, its read is the same read.
- **FR-008** Levels from the nudge's `handoffNudgeContextPercent` T (default 80, invalid values fall back to it): critical from T, approaching from floor(0.75 x T) (60 at the default), else ok.
- **FR-009** A missing, NaN, infinite or failed figure shows nothing and clears an earlier one; a number is rounded and clamped to 0-100.
- **FR-010** `chromeAscii` draws both with ASCII only.

## Defaults taken

`workspaceLine` on (the consolidation asked for; it changes nothing unless the project plugin is present); `contextMeter` off; the amber threshold is 75% of the nudge's; the meter text is dim and carries the level in words; no `—` placeholder (nothing is drawn instead).

## Out of scope / limitations

Agent activity, the auto-mode row, colour on the meter, a plugin-update count, a container count, and hiding a folded release band (its Hide button lives on the band, which no longer draws; use `releaseBand` off).
