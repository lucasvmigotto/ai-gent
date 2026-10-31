# Implementation Plan: Band priority

**Branch**: `docs/mod-specs-2` (spec) | **Date**: 2026-10-31 | **Spec**: `spec.md`

## Summary

A shared pure rule (`shared/band-priority.ts`) decides which active bands draw, copied into the hooks modules of `project`, `git`, `repo` and `devcontainer`; each records its bands in a `bands` atom and reads the others' through `dependencies`; `/bands` lists them all.

## Technical Context

**Engine**: Claude Code 2.1.312 (the CI pin). **Events**: `ui.render` (`AbovePrompt`), `command.run`. **Storage**: `$.state`. **Testing**: `claude plugin test`, unit tests of the pure rule.

## Files

| Path | What |
|---|---|
| `shared/band-priority.ts` | Planned: `visible(bands, cap)`, `label` |
| `plugins/<name>/hooks/band-priority.ts` | copies (`consistency.py --sync-shared-code`) for project, git, repo, devcontainer |
| `plugins/<name>/types/index.d.ts` | `bands` atom declaration per plugin |
| `plugins/project/hooks/register.tsx`, `plugins/git/hooks/register.tsx` | each existing band declares a priority and goes through `visible` |
| `scripts/consistency.py` | copy-equality for the new shared file |
| `plugins/project/hooks/bands.test.tsx` | stories 1–2 |

## Implementation order (tests first)

1. Probe on 2.1.312: can one plugin's `AbovePrompt` hook know what another draws or has recorded (`$.state.get` of another plugin's atom, absent plugin)? Record the result in the spec.
2. Pure rule and its unit tests (priorities, ties by `since`, `hold` never hidden).
3. Shared file, copies, `consistency.py` check.
4. `bands` atoms and types per plugin; existing bands (012, 017) adopt the rule with their priorities (`hold`; `normal`).
5. `/bands` and the `+N more` line.
6. Bands 023, 025 and 028 declare theirs as they are built.
7. `scripts/check.sh`, README mods table, docs row.
