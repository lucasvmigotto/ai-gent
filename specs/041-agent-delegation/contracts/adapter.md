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

A second Claude Code profile works as a delegate. The profile is a config directory passed as `CLAUDE_CONFIG_DIR=<dir>`; nothing inside it is read or copied (`AI_GENT_CLAUDE_CONFIG_DIR` names it for the adapter; unset, `run` refuses, see below). `AI_GENT_CLAUDE_BIN` overrides the program.

- `detect`: `claude --version`.
- `ready`: `claude -p "Reply with exactly: READY" --output-format json` with stdin closed; answers in about 5 s. Ready only when the exit code is 0, `is_error` is false and `result` contains `READY`; anything else is "not logged in".
- `run`: `claude -p "<brief>" --output-format json --allowedTools <list> --disallowedTools <list> --max-turns N < /dev/null`, working directory the worktree. Each tool list is one comma-joined argument. Started by the watcher like Codex, never detached with a shell `&`. `--max-turns` defaults to 40.
- Builder role: allowed `Read Grep Glob Edit Write Bash(cargo test:*) Bash(cargo clippy:*) Bash(cargo fmt:*) Bash(cargo build:*) Bash(cargo check:*) Bash(git status:*) Bash(git diff:*)`. Git is read-only (`status`, `diff`, `log`): the CLI lets a deny rule win over an allow rule, so `Bash(git:*)` is not denied wholesale; each mutating subcommand is (`add am apply branch checkout cherry-pick clean clone commit config fetch merge mv pull push rebase remote reset restore revert rm stash switch tag`). The delegate never commits; the supervisor stages by explicit path and commits.
- Reviewer and researcher roles are read-only: allowed `Read Grep Glob` and no Bash at all (even `git diff --output=<file>` writes); disallowed `Edit Write NotebookEdit Bash`. The supervisor puts the diff text into the review prompt, so the reviewer needs no git.
- Every role is denied `Read`, `Grep` and `Glob` (`Read(//<abs>/**)` syntax) on the profile's config directory, on every `.claude*` and `.codex*` entry under the home directory (names listed, contents never read), and on `~/.password-store` and `~/.gnupg`.
- **No OS sandbox.** A Claude profile runs without one. The tool lists are requests the CLI enforces, not a sandbox, and subprocesses are not covered: a build script (`cargo build`, `check`, `test`) or proc macro that an edit introduced can call git, the network or read files. The compensations are outside the worker: the supervisor's FR-011 diff checks, verification run outside the worker, and the worker never holding push credentials.
- The CLI prints one JSON object when it ends, with `result`, `is_error`, `num_turns` and `usage` (`input_tokens`, `cache_read_input_tokens`, `output_tokens`). The watcher keeps it as the single event in `events.jsonl`; `result` reads the final message and the usage from it. Usage maps `input_tokens` plus `cache_read_input_tokens` to the input, `cache_read_input_tokens` to the cached part, `output_tokens` to the output (no reasoning count).
- There is no progress before the end: `run` does not wait for a first event (the requested wait is ignored), `status` shows one event once the run ends, and `items_started` and `items_completed` stay 0.
- The watcher also reads that object: `status.json` tokens and the delegation log line carry the usage, and `last.txt` is written from `result`.
- Stall: with no events while it works, a Claude run is never stalled sooner than 1 hour (the stall limit is the larger of the requested one and 3600 s; Codex keeps the requested one, 600 s by default).
- `is_error`: a run that exits 0 with `is_error: true` is recorded as `failed`, not `done`. `exit.json` gets `is_error` and a `reason` (the `subtype` without `error_`, `max-turns` for `error_max_turns`, else `error`), and `result` answers both next to the unchanged `exit_code` (0).
- Profile required: `run` refuses without `AI_GENT_CLAUDE_CONFIG_DIR` (or an explicit profile in the library call) with "a Claude delegate needs its own profile: set AI_GENT_CLAUDE_CONFIG_DIR", so the supervisor's own profile is never used by accident. `detect` and `ready` still work with the variable set.
- `stop` is by PID file only, as for Codex.

## Completion events (T022, built)

A coordinator learns that a detached worker finished from an append-only log, not by polling `status`.
**Built:** the log, `delegate events`, `delegate watch`, `delegate ack`, `delegate idle`. **Not built:** the
skills do not call them yet (the protocol is `shared/worker-continuation.md`).

**`.ai-gent/delegate/events.jsonl`**, one JSON line per terminal or attention transition of a track:

| Field | Meaning |
|---|---|
| `id` | `track:started:state` (`started` from `status.json`). A later run of the same track has another `started`, so an event of an earlier run is told apart |
| `ts` | Epoch seconds when it was recorded |
| `track`, `agent`, `role`, `tasks` | As in `status.json` |
| `state` | `done`, `failed`, `stalled` or `taken-back` |
| `result` | The verdict's reason if any, else `accepted` or `rejected`, else `null` |

Never a brief, a message, a path or a diff. Every `status::write` that shows one of those states appends the event; `stalled` is computed from the clock (`stall_secs`), so `watch` and `events` record it. The append takes an exclusive OS lock (`File::try_lock` on `events.lock`, released when the file closes, so a crashed writer cannot leave it held and no age heuristic exists), reads the whole log under it and refuses an id already anywhere in it (so a restart, a second writer or a scan over hundreds of finished tracks adds nothing). The log is never trimmed, since dropping an id would let its event be appended again; it is one short line per transition. `watch` and `events` build each event from a single read of `status.json`, so state, `started` and tasks belong to one run and writes the line in one `write`, so two writers never interleave. Limit: `started` has one-second resolution, so a run restarted within the same second of the same track reuses the id.

**Commands** (all take `--root`):

- `delegate events [--pending]`: records transitions not yet in the log, then prints the events (`--json`: an object with `events`).
- `delegate watch [--once] [--since-ack]`: prints each unacknowledged event as one line `event id=… ts=… track=… agent=… role=… tasks=T1,T2 state=… result=…` (`--json`: the event object per line). Without `--once` it keeps running (checks every second), prints new events, and prints `idle: free_slots=N` when slots are free and no track is live (once per change). Printing does not acknowledge. `--since-ack` states what `watch` always does: acknowledged events are never replayed.
- `delegate ack <id>… | --all`: appends `{id, ts}` to `.ai-gent/delegate/acks.jsonl`; an unknown id is refused.
- `delegate idle --tasks-file F`: one line, exit 0, never dispatches: `free_slots=N ready_tasks=M reason=<dispatch|dependency|approval|none-ready|cap>`. `free_slots` is the pool of `delegate plan` (`max_volumes`, else `max_parallel`, default 3, minus live tracks, and the disk floor). A task is ready when no track (taken-back ones excepted) holds it, it needs no owner decision and each `depends` is in a done track. `dispatch`: a slot and a ready task, start it; `cap`: ready work, no slot; `approval`, `dependency`, `none-ready`: why nothing is ready.

Tests: `delegate::events::tests` in `ai-gent-core` and `rust/crates/ai-gent/tests/delegate_events.rs`.
