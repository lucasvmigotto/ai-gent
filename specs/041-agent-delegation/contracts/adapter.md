# Adapter contract

One adapter per delegate agent (first: Codex). An adapter is a small script or module the skill calls; it never decides what to build.

| Operation | Behavior |
|---|---|
| `detect` | Is the agent's CLI on `PATH`, and does it run (`--version`)? Does not read its auth files. |
| `ready` | A cheap call proving it can start (auth is the user's login; a failure is reported as "not logged in", never worked around). |
| `preflight <worktree>` | Proves the run can succeed: stdin is closed, the worktree is writable, **a commit works** (stage and commit a scratch file in a throwaway branch, then drop it), and the network is as expected (none for Codex). |
| `run <worktree> <brief>` | Starts the agent in the background with stdin closed, writes events and the final message to files under `.ai-gent/delegate/<track>/`, records a PID file. |
| `status <track>` | Last event time, event count, running or exited. |
| `stop <track>` | Stops by the PID file; never by pattern. |
| `result <track>` | The final message, the exit code, usage when the agent reports it. |

## Codex (verified in trial 1)

- `codex exec --cd <worktree> -s workspace-write --add-dir <common .git dir> --add-dir <TMPDIR> -o <last> --json "<brief>" < /dev/null`
- Without `< /dev/null` it waits on "Reading additional input from stdin".
- The sandbox has no network and denies sockets and some caches; a delegate's own `check.sh` shows failures that are not real. Verification therefore runs in the supervisor.
- In a worktree, commits need the **common** `.git` (objects), not only the per-worktree git dir. Trial 1 failed here: the delegate could not commit.
