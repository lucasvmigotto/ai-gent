# Adapter contract

One adapter per delegate agent (first: Codex). An adapter is a small script or module the skill calls; it never decides what to build.

| Operation | Behavior |
|---|---|
| `detect` | Is the agent's CLI on `PATH`, and does it run (`--version`)? Does not read its auth files. |
| `ready` | A cheap call proving it can start (auth is the user's login; a failure is reported as "not logged in", never worked around). |
| `preflight <worktree>` | Proves the run can succeed: stdin is closed, the worktree is writable, the agent can start, and the network is as expected (none for Codex). It does **not** expect the delegate to commit (see below): the delegate leaves its changes in the working tree and the supervisor stages and commits. |
| `run <worktree> <brief>` | Starts the agent in the background with stdin closed, writes events and the final message to files under `.ai-gent/delegate/<track>/`, records a PID file. |
| `status <track>` | Last event time, event count, running or exited. |
| `stop <track>` | Stops by the PID file; never by pattern. |
| `result <track>` | The final message, the exit code, usage when the agent reports it. |

## Codex (verified in trial 1)

- `codex exec --cd <worktree> -s workspace-write --add-dir <common .git dir> --add-dir <TMPDIR> -o <last> --json "<brief>" < /dev/null`
- Without `< /dev/null` it waits on "Reading additional input from stdin".
- The sandbox has no network and denies sockets and some caches; a delegate's own `check.sh` shows failures that are not real. Verification therefore runs in the supervisor.
- **A Codex delegate cannot commit.** The sandbox keeps `.git` read-only even when the directory is passed with `--add-dir` (trial 1: object store read-only; trial 2, with the common git dir added: `index.lock` read-only). So the contract is: the delegate edits the working tree and stops; the supervisor stages the delegate's changes by explicit path (only inside `allowed_paths`), commits with the project's message rules, and pushes. This also puts the commit message and the path check under the supervisor's control.
- **Progress** comes from `item.started` / `item.completed` events (trial 1: 39 and 47; trial 2: 20 and 24); the final `turn.completed` event carries `usage` (`input_tokens`, `cached_input_tokens`, `output_tokens`, `reasoning_output_tokens`).
- A delegate's `apply_patch` can fail on a stale view of a file (trial 2: `tasks.md`); it retries on its own, so a single error event is not a stall.
