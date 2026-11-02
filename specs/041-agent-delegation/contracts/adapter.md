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

## Reviewer role (verified in trial 4)

- A Codex reviewer runs as `codex exec --cd <worktree> -s read-only -o <last> --json "<review prompt>" < /dev/null`. The `codex exec review` subcommand cannot be used: `--base` rejects a custom prompt, and the prompt is what carries the acceptance criteria to check.
- The review prompt names the document or diff, the files it must be checked against, what to check (consistency, claims that contradict the cited files, unlabelled claims, empty cells, conclusions that do not follow) and says the reviewer has no network, must list what it could not check, and must not edit anything.
- Trial 4: a read-only review of a 50-line survey took 28 s and about 99k input and 1.1k output tokens, and found five spec-compliance gaps neither the builder nor the supervisor had noticed.
- The loop: builder drafts, supervisor fact-checks decisive claims, a different agent reviews against the acceptance criteria, the findings go back to the builder (or the supervisor fixes them), then a second review; the task is done only when the acceptance criteria pass, not when the builder says so.

## Claude profile (verified in trial 5)

A second Claude Code profile works as a delegate. The profile is a config directory passed as `CLAUDE_CONFIG_DIR=<dir>`; nothing inside it is read or copied (`AI_GENT_CLAUDE_CONFIG_DIR` names it for the adapter; unset, the CLI's own default profile is used, which is the supervisor's, not a second one). `AI_GENT_CLAUDE_BIN` overrides the program.

- `detect`: `claude --version`.
- `ready`: `claude -p "Reply with exactly: READY" --output-format json` with stdin closed; answers in about 5 s. Ready only when the exit code is 0, `is_error` is false and `result` contains `READY`; anything else is "not logged in".
- `run`: `claude -p "<brief>" --output-format json --allowedTools <list> --disallowedTools <list> --max-turns N < /dev/null`, working directory the worktree. Each tool list is one comma-joined argument. Started by the watcher like Codex, never detached with a shell `&`. `--max-turns` defaults to 40.
- Builder role: allowed `Read Grep Glob Edit Write Bash(cargo:*) Bash(git status:*) Bash(git diff:*)`; disallowed `Bash(git commit:*) Bash(git push:*) Bash(git add:*) Bash(git reset:*)`. The delegate never commits; the supervisor stages by explicit path and commits.
- Reviewer and researcher roles are read-only: allowed `Read Grep Glob Bash(git status:*) Bash(git diff:*)`; `Edit` and `Write` disallowed.
- The CLI prints one JSON object when it ends, with `result`, `is_error`, `num_turns` and `usage` (`input_tokens`, `cache_read_input_tokens`, `output_tokens`). The watcher keeps it as the single event in `events.jsonl`; `result` reads the final message and the usage from it. Usage maps `input_tokens` plus `cache_read_input_tokens` to the input, `cache_read_input_tokens` to the cached part, `output_tokens` to the output (no reasoning count).
- There is no progress before the end: `run` does not wait for a first event (the requested wait is ignored), `status` shows one event once the run ends, and `items_started` and `items_completed` stay 0.
- `stop` is by PID file only, as for Codex.
