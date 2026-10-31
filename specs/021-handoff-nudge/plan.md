# Implementation Plan: Hand-off nudge

**Branch**: `docs/mod-specs-2` (spec) | **Date**: 2026-10-31 | **Spec**: `spec.md`

## Summary

In the `project` plugin's hooks module: at turn end, read `$.session.usage().context.percent` and toast once when it passes the configured percent, with the checkpoint's age in commits; toast once on `session.compact` (`manual`, `auto`); and, only when `token_report.py` is found beside the plugin, run a throttled, detached `budget --json --projected=0` and toast once per low cap. The mod never writes and never depends on `agents`.

## Technical Context

**Language**: TypeScript hooks module (types from `'claude-code'`). **Engine**: Claude Code 2.1.312 (the CI pin). A probe mod using `session.compact`, `session.append` and `$.session.usage()` passed `claude plugin validate` under 2.1.312 on 2026-10-31, so the pin is unchanged; a `claude plugin test` case confirms each at runtime (task 3). **Events**: `turn.complete`, `session.compact` (matcher on `trigger`; always `next(e)`), `session.start` (registers nothing slow). **Storage**: `$.state` atom `handoffToasted` (shape-tagged; `data-model.md`). **Project Type**: Claude Code plugin module.

## Read-only calls

| Purpose | Call |
|---|---|
| the context figure | `$.session.usage()` → `context.percent` |
| the checkpoint (path, mtime, `isStale`) | 012's `checkpoint` atom (`plugins/project/hooks/checkpoint.ts`; type in `plugins/project/types/index.d.ts`) |
| commits since the checkpoint | `git rev-list --count --after=<mtime ISO> HEAD` |
| a clean working tree | `git status --porcelain` |
| is there a budget | `$.fs.exists(".ai-gent/budget.yaml")` |
| is the script there (optional) | `$.fs.exists("${$.plugin.root}/../agents/scripts/token_report.py")` |
| the caps (only when both exist) | `python3 ${$.plugin.root}/../agents/scripts/token_report.py budget --json --projected=0` (cwd the repository) |
| the repository root | `plugins/project/hooks/repo.ts` |

## Files

| Path | What |
|---|---|
| `plugins/project/hooks/handoff-model.ts` | pure: the context and cap threshold tests, the commit-count wording, the toast text (Planned) |
| `plugins/project/hooks/handoff.ts` | the calls, the throttle, the `session.compact` hook, the toasted atom (Planned) |
| `plugins/project/hooks/handoff.test.tsx` | stories 1–4; `$.session.usage()` and `$.process.run` mocked, `session.compact` raised, recorded `budget --json` output (Planned) |
| `plugins/project/hooks/register.tsx` | its `turn.complete` (line 73) and `session.start` (line 63) handlers are shared: gains one composing call in each, plus the `session.compact` hook |
| `plugins/project/types/index.d.ts` | the `handoffToasted` atom's type |
| `plugins/project/.claude-plugin/plugin.json` | `userConfig`: `handoffNudge`, `handoffNudgeContextPercent`, `handoffNudgeCapPercent` |
| `README.md` mods table, docs | one row for `project` (via `skill-docs`) |

No change to `project:checkpoint`, `checkpoint.py`, `token_report.py`, `shared/token-tracking.md`, `.github/workflows/check.yml` or the CI pin.

## Testing

- `claude plugin test plugins/project` (wired into `scripts/check.sh` by 012); `$.process.run` and `$.session.usage()` mocked, never a real transcript.
- A runtime case per engine feature the mod uses: raise `session.compact` (`auto`, `manual`, `precompute`) and mock `$.session.usage()`, so a version that validates but behaves differently fails.
- Manual: with a throwaway `.ai-gent/budget.yaml` of 1M caps against a fake `HOME`, then a real session; and a session with `agents` not installed.

## Implementation order (tests first)

1. Extend 012's types with the `handoffToasted` atom; add the three `userConfig` entries.
2. `handoff-model.ts` and its unit tests (both thresholds, wording, no-checkpoint and current-checkpoint cases).
3. Failing story 1–4 tests in `handoff.test.tsx`, including the runtime cases above and the no-script case.
4. `handoff.ts`: usage read, `session.compact` hook, the optional cap check; compose from `register.tsx`.
5. `scripts/check.sh`, docs row, manual check.

## check.sh / CI impact

None beyond 012: its loop already runs `claude plugin test` for `project`. The run grows by one test file. No CI version change.

## Risks

- **Engine drift**: `session.compact` and `$.session.usage()` are verified by `claude plugin validate` under the pin and by the runtime test cases; the API is early access.
- **Sibling script path**: the cap check degrades to silent when `agents` is not found beside the plugin; nothing else depends on it.
- **Cost**: `$.session.usage()` is cheap; `budget` is slow on large histories, so it is detached, throttled to 5 minutes, and skipped when the script or budget file is absent.
- **Noise**: once per condition per session; off switch and two thresholds in `/config`; the `session.compact` hook never changes the compaction.

## Constitution Check

N/A — no constitution.
