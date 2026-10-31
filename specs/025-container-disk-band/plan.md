# Implementation Plan: Container and disk band

**Branch**: `docs/mod-specs-2` (spec) | **Date**: 2026-10-31 | **Spec**: `spec.md`

## Summary

In the `devcontainer` plugin's hooks module: an `AbovePrompt` band fed by running `hooks/leftovers.sh --json` through `$.process.run` and reading its JSON (the `--json` flag is a separate prior change), refreshed on session start, after container Bash calls and (throttled) on turn completion; and `/containers`, which prints the findings and the script's text form with a pointer to `wt.py resources`.

## Technical Context

**Language**: TypeScript hooks module. **Engine**: Claude Code 2.1.312. **Events**: `ui.render` (`AbovePrompt`), `session.start`, `turn.complete`, `tool.call` (after `next`, Bash), `command.run` (`$.command.register`). **Storage**: `$.state`. **Project Type**: Claude Code plugin module.

## Script call (the only process the mod starts)

| Purpose | Command |
|---|---|
| the report | `$.plugin.root + "/hooks/leftovers.sh"` with `--json` (a 10 s timeout; `findings: []` means nothing to report, `checked: false` means it could not check); `/containers` also runs it without `--json` for the script's advice text |

## Files

| Path | What |
|---|---|
| `plugins/devcontainer/hooks/leftovers.sh` | **prerequisite change, own branch**: `--json` flag (default output unchanged) |
| `scripts/test-containers-guard.sh` | **prerequisite**: `--json` cases (findings, clean, disabled, fake engine) |
| `plugins/devcontainer/hooks/containers-model.ts` | pure: `readReport(json)`, `bandText(findings, columns)` (order: disk, dangling, volumes, buildah), `commandText`, `TRIGGER` (the container-work command pattern), the no-prune guard |
| `plugins/devcontainer/hooks/containers.ts` | the script call, the atom, throttled refresh |
| `plugins/devcontainer/hooks/containers.test.tsx` | stories 1–3, the script mocked with recorded stdout |
| `plugins/devcontainer/hooks/hook-lines.ts` | existing copy of `shared/hook-lines.ts`; unchanged (017's toast keeps using it; the band reads JSON and needs none of it) |
| `plugins/devcontainer/hooks/register.tsx` | gains the band, the trigger hook and `/containers` beside the guard-ux and toast parts |
| `plugins/devcontainer/types/index.d.ts` | the `containers` atom |
| `plugins/devcontainer/.claude-plugin/plugin.json` | `userConfig` `containersBand`, `containersRefreshSeconds`, `containersCommand` |
| `scripts/consistency.py`, `scripts/test_consistency.py` | a check that the JSON keys the mod reads (`kind`, `count`, `free_gb`, `floor_gb`, `checked`, `reason`, `findings`) are the ones `leftovers.sh --json` prints |

No change to `wt.py`, `shared/containers.md` or `leftovers.sh`'s default output. The `--json` flag and its guard-suite cases are the one script change, landed first.

## Testing

- `claude plugin test plugins/devcontainer`; `$.process.run` mocked with recorded `leftovers.sh` outputs (never a real engine), the band mounted on `terminal` and `desktop`.
- A test greps every string the band and `/containers` can print for `prune`.
- `scripts/test-containers-guard.sh`: its existing text cases run unchanged; the new `--json` cases pass (this is a prerequisite change, so they land and pass before the mod's branch).
- Manual: in a repository with leftover buildah containers, before and after `wt.py finish`.

## check.sh / CI impact

012's loop already runs `claude plugin test` for `devcontainer`; `scripts/consistency.py` gains the JSON-key check (a case in `scripts/test_consistency.py`). The `devcontainer` bump also follows the `leftovers.sh` change. Version bump comes from `scripts/release.py`.

## Implementation order (tests first)

0. **Prerequisite, own branch**: `leftovers.sh --json` with its `scripts/test-containers-guard.sh` cases, landed on `dev` first.
1. `containers-model.ts` tests: reading the JSON, band wording at three widths, an unknown kind, the no-prune assertion; then the model.
2. The JSON-key check in `scripts/consistency.py` with its test.
3. `containers.ts`: the script call and the atom; tests with the script mocked (findings, `findings: []`, `checked: false`, unparseable output, a timeout).
4. The band in `register.tsx`, composed beneath others; the `terminal` and `desktop` tests.
5. The trigger hook and the throttle; the `/containers` command.
6. `userConfig`, the README mods table and `docs/site` through `skill-docs`; mark the spec Implemented.

## Risks

- **API churn** (early access): `AbovePrompt` props and `$.process.run`'s environment; re-checked at each pin bump.
- **A key drifting between script and mod**: guarded by the consistency check and the unknown-kind fallback.
- **Cost on a slow engine**: throttle, trigger list, timeout, no timer.
- **False reassurance**: `--json`'s `checked: false` tells "couldn't check" from "clean"; the band draws nothing for either, and `/containers` says which.
- **Second run of the script**: this feature supersedes 017 FR-005's "never a second time" for the band; the cost is bounded by the throttle and the trigger list.
- **Buildah containers of other projects** look like this project's problem; the band says `(any project)` and offers no removal.

## Constitution Check

N/A — no constitution.
