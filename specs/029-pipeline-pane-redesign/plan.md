# Implementation Plan: Pipeline pane, dashboard layout

**Branch**: `docs/mod-specs-2` (spec) | **Date**: 2026-10-31 | **Spec**: `spec.md`

## Summary

Keep 013's reading, snapshot, poll and recommendation. Refactor `pipeline-view.ts` into a pure structured model (`build`), keep `view` and `text` as the compact and headless output, draw the model with native elements in a new `pipeline-draw.tsx`, and add three small atoms, three options and the interaction (expand, filter). Five phases, as the proposal orders them; **phase 1 is its own task group and the recommended starting point**, because it changes no pixel and every later phase builds on it.

## Technical Context

**Language**: TypeScript hooks module (types from `'claude-code'`, no Node, no DOM). **Engine**: Claude Code 2.1.312 (the CI pin). **Events**: `ui.render` (`Pane`, `requestId: 'pipeline'`), `command.run`, `tool.call` (after `next`), `session.start`, as 013. **Storage**: `$.state` only. **Project Type**: Claude Code plugin module (the `project` plugin).

## Files

| Path | What |
|---|---|
| `plugins/project/hooks/pipeline-model.ts` | gains `STAGE_NAME` (by `ArtifactSpec.stage`), `stageStrip` and `progress` (pure); no other change |
| `plugins/project/hooks/pipeline-view.ts` | gains the structured model and `build`, the glyph and tone tables, the width modes; keeps `view`, `text`, `Line`, `Tone` (plus `info`) unchanged for compact and headless |
| `plugins/project/hooks/pipeline-draw.tsx` | new: `drawDashboard($, e, model, handlers)` mapping the model to `Box`, `Text`, `Button` (the region table in `data-model.md`); no `$.fs`, no `$.process`, no `$.state` write while drawing |
| `plugins/project/hooks/pipeline.ts` | declares and reads the new atoms (an atom is read only in the file that declares it); writes `checkedAt` every tick; sets `pollMs` on open, close and stop |
| `plugins/project/hooks/register.tsx` | the `Pane` hook chooses `drawDashboard` or the 013 lines by `pipelineLayout`; wires the press handlers; adds the options to its `Options` type |
| `plugins/project/hooks/pipeline-fixtures.ts` | gains a larger README (28 rows) and the golden dashboard strings |
| `plugins/project/hooks/pipeline.test.tsx` | 013's tests kept; model, draw and interaction tests added |
| `plugins/project/types/index.d.ts` | the three atoms (`pipeline.filter`, `pipeline.checkedAt`, `pipeline.pollMs`) |
| `plugins/project/.claude-plugin/plugin.json` | `userConfig`: `pipelineLayout`, `pipelineAscii`, `pipelineMaxRows` |
| `scripts/consistency.py` | fails when an artifact `stage` has no `STAGE_NAME` |
| `README.md` (mods table, `project` row), `docs/`, `AGENTS.md` | the dashboard, its options and the LIVE meaning, via `skill-docs` |

No change to `shared/pipeline.md`, `plugins/project/skills/status/SKILL.md`, `specsync.py`, `wt.py` or `token_report.py`.

## Compatibility and regression table

| Case | Expected | Test |
|---|---|---|
| Terminal, over 90 columns | full matrix, three large metrics | `build` golden at 120 + mounted tree on `terminal` |
| Terminal, 40 to 90 | condensed grid, one-line summary, glyph-only cells | golden at 80 and 56 |
| Terminal, under 40 | compact text, byte-equal to 013's `view` | golden at 39 equals 013's expected lines |
| Desktop | same model, native components only | mounted tree on `desktop`, keys found |
| vscode, mobile | same model; mobile opens by command only | mounted tree; 013's rule unchanged |
| Headless (`claude -p`) | 013's `text`, unchanged | `text()` equals 013's output on the fixtures |
| Empty pipeline | `No pipeline artifacts yet. Start with /project:init.` | fixture with nothing |
| Empty or missing README | the section says so, no bar | two fixtures |
| Long feature names | cut with `…`, no wrap | a 60-character name at 56 and 120 columns |
| Large feature list | at most `pipelineMaxRows` rows, then `+N more` | 200-row fixture |
| ASCII option | no non-ASCII character in the output | `pipelineAscii` golden, a regex over the text |
| No git | no `Stale`, a `Staleness needs git.` line | `hasGit: false` snapshot |
| Poll stopped | `STALE` after three intervals | mocked clock |
| Filter, expansion | atoms change, no `$.fs` or `$.process` call | spies on both, as 013's SC-003 test |
| `compact` option | 013's lines | the option test |
| Parity | per-feature state and the ready, blocked, done split equal the scripts | 013's `PipelinePaneParityTest`, untouched |

## Tasks (ordered; tests first within each group)

### Group 1 — Phase 1: structured view model (start here)

1. Copy 013's current `view` and `text` outputs for every fixture into `pipeline-fixtures.ts` as the **compatibility goldens**; add a test that `view` and `text` still equal them. This must pass before and after the whole change.
2. Write the model tests: `build` for `summary`, `progress`, the artifact rows (the four states), the matrix columns for three README shapes (F/B/Q, a single `Status` column, a README with no feature rows), the filter, the stage strip, the badge, and the three modes by width.
3. Add `STAGE_NAME`, `stageStrip` and `progress` to `pipeline-model.ts`, and the `scripts/consistency.py` check.
4. Implement `build` and the glyph, tone and column tables in `pipeline-view.ts` until the tests pass; export the types. Nothing is drawn yet.
5. `scripts/check.sh`; `claude plugin test plugins/project`. Stop here for review if wanted: no user-visible change.

### Group 2 — Phase 2: native table renderer

6. **Probe** (before anything else in this group): a throwaway test mounts an empty `Box height={1} backgroundColor="success" flexGrow={3}` beside a `flexGrow={1}` one on `terminal` and `desktop`, and a `Button` around a row `Box`. Record what paints. Decide the bar's primary (`Box` segments) or fallback (`█` runs) and whether a row is a `Button` or its first cell is.
7. Mounted-tree tests for the header, the Artifacts table and the matrix by key (`terminal` and `desktop`).
8. Implement `pipeline-draw.tsx`: header, table rows and cells with fixed widths and alignment, the status text cell; the `Pane` hook chooses the layout by `pipelineLayout`.
9. Add the options to `plugin.json` and `register.tsx`; the `compact` test.

### Group 3 — Phase 3: visual refinement

10. Tests for tone and glyph per status, the ASCII set, and "no colour alone" (every toned cell has a glyph or word).
11. Add the semantic colours (theme keys), the glyphs with fallbacks, the left-edge mark, the flat summary metrics, the progress bar and the stage strip, the section separation.
12. Add `pipeline.checkedAt` and `pipeline.pollMs`; the LIVE badge and caption; the mocked-clock tests (SC-004).

### Group 4 — Phase 4: interactions

13. Tests: expanding an artifact, a feature and each marker kind; the filter; the atoms survive a poll tick; spies prove no `$.fs` or `$.process` call and no timer from an interaction.
14. Implement the handlers (they write atoms only), the filter buttons with the active marker, the row cap line.

### Group 5 — Phase 5: compatibility and regression

15. Run the compatibility table above; fix gaps.
16. `claude plugin validate plugins/project`, `scripts/check.sh`, then a manual run on this repository (28 features) and on a repo with no pipeline, at about 120, 80 and 36 columns, in Unicode and ASCII.
17. `skill-docs`: README mods table, docs site, `AGENTS.md`, `CHANGELOG.md`; mark 013's row note and add 029 to `specs/README.md`.

## Risks

- **Background colours may not paint** an empty `Box` on every surface (spec Assumption). Mitigation: the probe in task 6; the bar's counts are always printed; the `█` fallback.
- **`Button` around a row** may not be accepted. Mitigation: the probe; the first cell as the button.
- **`$.clock.now()` in a render** may be refused. Mitigation: the ticker writes the verdict into an atom.
- **One redraw per interval** from `checkedAt`. Cost is small (an atom write), and it is the price of an honest badge; SC-003 still holds (no process, no read).
- **Matrix width**: a README with many side columns (a future format) overflows 90 columns. Mitigation: the status columns drop to glyph-only first, then the PRI column; tested with a six-column README.
- **Glyph support** differs by terminal. Mitigation: `pipelineAscii`, words beside every glyph.
- **013's tests are the contract.** Mitigation: task 1 pins `view` and `text` before any change, and parity is untouched.

## check.sh / CI impact

`claude plugin test` already runs for `project`; the run grows. `scripts/consistency.py` gains the `STAGE_NAME` check. No new CI job.

## Constitution Check

N/A — no constitution.
