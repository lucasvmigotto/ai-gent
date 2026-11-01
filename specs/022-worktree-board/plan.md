# Implementation Plan: Worktree board

**Branch**: `docs/mod-specs-2` (spec) | **Date**: 2026-10-31 | **Spec**: `spec.md`

## Summary

A `Pane` drawn by the `agents` plugin's hooks module: rows from `wt.py list --json` refreshed on a poll and after `wt.py`/`git commit`/`git merge` Bash calls, PR and CI columns from one `gh pr list --json` per slower refresh when `gh` is signed in. `/worktree-board` opens it. Same shape as 013's pipeline pane.

## Technical Context

**Language**: TypeScript hooks module. **Engine**: Claude Code 2.1.312. **Events**: `ui.render` (`Pane`), `command.run`, `tool.call` (after `next`, Bash), `ui.close`. **Storage**: `$.state` atoms (`worktreeRows`, `worktreeIsOpen`, `worktreeLocalAt`, `worktreeRemoteAt`, `worktreeGh`; `data-model.md`), shape-tagged. **Project Type**: Claude Code plugin module.

## Read-only calls

| Purpose | Call |
|---|---|
| the worktrees | `python3 ${$.plugin.root}/scripts/wt.py list --json` (cwd the repository) |
| is `gh` usable | `gh auth status` (exit code only) |
| PR and checks, one call | `gh pr list --state all --limit 100 --json number,headRefName,state,isDraft,statusCheckRollup` |
| the remote's platform | `git remote get-url origin` (read only; part of FR-001's list) |

## Files

| Path | What |
|---|---|
| `plugins/agents/hooks/register.tsx` | `Options` gains `worktreeBoard`, `worktreeBoardSeconds`, `worktreeBoardRemoteSeconds`; restructured so `worktreeBoard(on, options)` composes on its own check ahead of the `tokenStatus` return (the board does not depend on 014's switch) |
| `plugins/agents/hooks/worktrees.ts` | the calls, the two pollers (local and remote, one timestamp atom each), the Bash-triggered local refresh, open/close, the atoms (Planned) |
| `plugins/agents/hooks/worktrees-model.ts` | pure: parse `wt.py list --json` and `gh` output, join by branch, the wording, the narrow form (Planned) |
| `plugins/agents/hooks/worktrees.test.tsx` | stories 1–3, both surfaces, a fixture of recorded outputs (Planned) |
| `plugins/agents/types/index.d.ts` | the `worktree*` atoms |
| `plugins/agents/.claude-plugin/plugin.json` | `userConfig`: `worktreeBoard`, `worktreeBoardSeconds`, `worktreeBoardRemoteSeconds` |
| `specs/022-worktree-board/data-model.md` | rows, the PR and check mapping, the atoms |
| `README.md` | the `agents` row of the mods table (`worktreeBoard` and its two intervals) |

No change to `wt.py`, `agents:parallel` or `shared/containers.md`.

## Testing

- `claude plugin test plugins/agents`; `$.process.run` mocked with recorded `wt.py list --json` and `gh pr list` outputs (including a failing check, a draft, a merged PR, no PR).
- Parity: the model's reading of `wt.py list --json` is checked against a fixture generated from the script on a throwaway repository (a `unittest` case in `test_wt.py` (which uses `unittest`, as `scripts/check.sh` runs it) that fails if the keys change).
- Manual: `claude --plugin-dir plugins/agents` during a real wave of three worktrees.

## Implementation order (tests first)

1. Atoms and `userConfig`; the parity fixture and its `unittest` case.
2. `worktrees-model.ts` with unit tests (join, wording, narrow form, null counts).
3. Failing pane tests for stories 1–3.
4. `worktrees.ts`: calls, pollers (two intervals, two timestamps), open/close, the Bash-triggered refresh (story 3 scenario 0), the `gh` availability atom.
5. Compose from `register.tsx` ahead of the `tokenStatus` return; `/worktree-board` command; headless text.
6. `scripts/check.sh`, docs row, manual wave.

## check.sh / CI impact

`claude plugin test` already runs for `agents` (014 added its module). One more test file, one more `unittest` case in `plugins/agents/scripts/test_wt.py`.

## Risks

- **API churn**: `Pane` and `$.ui.open` are early access; the pure model is kept apart from drawing.
- **Cost**: `wt.py list` spawns git per worktree; fixed intervals, no overlapping runs, a row cap.
- **gh rate limits**: one call per remote refresh at 120 s; off when not signed in.
- **Two features in one module** (014 and 022): each registers its own hooks and a throwing render hook is skipped alone.

## Constitution Check

N/A — no constitution.
