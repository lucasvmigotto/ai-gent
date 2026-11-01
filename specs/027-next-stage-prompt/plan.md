# Implementation Plan: Next-stage prompt

**Branch**: `docs/mod-specs-2` (spec) | **Date**: 2026-10-31 | **Spec**: `spec.md`

## Summary

In the `project` plugin's hooks module: after an Edit/Write on a watched path (and an `mtimeMs` check at turn end for writes made by other means), recompute the pipeline snapshot with 013's `compute`, take its `next` (that is `recommend`'s answer), and toast it when it changed. The trigger extends 013's existing Edit/Write hook; the "before" value is seeded at session start. No new rules, no writes.

## Technical Context

**Language**: TypeScript hooks module. **Engine**: Claude Code 2.1.312. **Events**: `session.start` (detached seed of `nextStageLast`, no toast), `tool.call` (after `next`, Edit/Write; extends the hook at `register.tsx:158-166`), `turn.complete`. **Storage**: `$.state` atoms (`nextStageLast`, `nextStageShown`, `nextStageMtimes`), shape-tagged. **Project Type**: Claude Code plugin module.

## Read-only calls

| Purpose | Call |
|---|---|
| the snapshot and its recommendation | `compute($)` (`plugins/project/hooks/pipeline.ts`): `$.fs` reads of the artifacts, `specs/README.md`, markers; `git log -1 --format=%cs -- <path>`, cached by `mtimeMs`; its `.next` is `recommend(...)` |
| writes by other means | `$.fs.stat(path).mtimeMs` for the watched paths |

## Files

| Path | What |
|---|---|
| `plugins/project/hooks/register.tsx` | extends the Edit/Write hook (one call into `nextstage.ts`) and the `session.start` seed |
| `plugins/project/hooks/nextstage.ts` | the trigger, the session-start seed, the once-per-turn rule, the atoms (Planned) |
| `plugins/project/hooks/nextstage-model.ts` | pure: whether a recommendation differs, the toast wording (Planned) |
| `plugins/project/hooks/nextstage.test.tsx` | stories 1–3, fixtures from `pipeline-fixtures.ts` (Planned) |
| `plugins/project/types/index.d.ts` | the `nextStage*` atoms (`data-model.md`) |
| `plugins/project/.claude-plugin/plugin.json` | `userConfig`: `nextStageToast` |

No change to `pipeline-model.ts`, `pipeline.ts`, `project:status` or `shared/pipeline.md`.

## Testing

- `claude plugin test plugins/project`; `$.fs` and `$.process.run` mocked with the pane's fixture trees.
- Parity (SC-002): the test feeds each rule fixture of 013 and asserts the toast's stage and reason equal the snapshot's `next`, including the brief-only case (`/project:status`); if 013's rules change, both follow.
- Manual: `claude --plugin-dir plugins/project`, run `project:init` in a scratch directory, and watch for the toast after the brief.

## Implementation order (tests first)

1. Atoms and `userConfig`; `nextstage-model.ts` with unit tests (changed or not, wording).
2. Failing story 1–3 tests, including the first write of a session and the brief-only case.
3. `nextstage.ts`: the `nextStageLast` seed at `session.start`, the Edit/Write trigger, the turn-end `mtimeMs` check, once-per-turn.
4. Extend `register.tsx`'s existing Edit/Write hook and `session.start`.
5. `scripts/check.sh`, docs row, manual run.

## check.sh / CI impact

None beyond 012: `claude plugin test` already runs for `project`. One more test file.

## Risks

- **Rule drift**: avoided by construction (it calls `recommend`); the parity test makes a divergence fail.
- **Noise**: once per turn, once per distinct recommendation, an off switch.
- **Cost**: a recompute per turn with a watched write; `compute` is the same cost 013 pays per poll when open, and is cached by `mtimeMs`.
- **Two features reading the same files** (013 and 027): reuse the pane's atom when fresher, so a session with the pane open does not double the reads.

## Constitution Check

N/A — no constitution.
