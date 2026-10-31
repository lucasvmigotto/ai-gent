# Data model — worktree board

| Entity | Fields | Source |
|---|---|---|
| Worktree row | branch (or `(detached)`), path (relative to the repository root), dirty, ahead, behind (the last two null when no dev branch is found) | `wt.py list --json` (`cmd_list`), exactly its keys |
| Pull request | number, state (`OPEN`, `MERGED`, `CLOSED`), isDraft, checks (derived) | one `gh pr list --state all --limit 100 --json number,headRefName,state,isDraft,statusCheckRollup`, joined to a row by `headRefName` = `branch` |
| Board row | worktree row, pull request or `no PR` or `not pushed`, mark (`nothing of its own`, `failing`, `removable`) | derived |

## Check rollup (decided 2026-10-31 (the maintainer accepted the default))

`statusCheckRollup` is a list of checks. Each has `conclusion` (check runs: `SUCCESS`, `FAILURE`, `TIMED_OUT`, `CANCELLED`, `ACTION_REQUIRED`, `STARTUP_FAILURE`, `NEUTRAL`, `SKIPPED`) and `status` (`COMPLETED`, `IN_PROGRESS`, `QUEUED`, ...), or, for a commit status, `state` (`SUCCESS`, `FAILURE`, `ERROR`, `PENDING`, `EXPECTED`).

| Result | Rule, first match wins |
|---|---|
| failing | any check with a `FAILURE`, `TIMED_OUT`, `CANCELLED`, `ACTION_REQUIRED` or `STARTUP_FAILURE` conclusion, or a status `state` of `FAILURE` or `ERROR` |
| pending | any check whose `status` is not `COMPLETED`, or a status `state` of `PENDING` or `EXPECTED` |
| passing | the list is not empty and none of the above |
| none | the list is empty (the cell shows no check mark) |

## Marks

| Mark | Condition |
|---|---|
| nothing of its own | `ahead` 0 and `dirty` false |
| removable (hint `wt.py remove <branch>`) | the pull request is `MERGED` and `dirty` is false; never from `ahead`, which stays above 0 after a rebase merge |
| failing | the rollup is failing |

## State atoms (`plugins/agents/types/index.d.ts`)

| Key | Type | Purpose |
|---|---|---|
| `worktreeIsOpen` | boolean | whether the board is open (polls only then) |
| `worktreeRows` | board rows | the last good rows, kept across a reload and on a `wt.py` failure |
| `worktreeLocalAt` | number | when `wt.py list --json` last ran (ms), for the local interval and the Bash-triggered refresh |
| `worktreeRemoteAt` | number | when `gh pr list` last ran (ms), for the remote interval |
| `worktreeGh` | `{ usable: boolean, platform: string }` | `gh auth status` result and the platform from `git remote get-url origin`, so neither is re-run on every refresh |
