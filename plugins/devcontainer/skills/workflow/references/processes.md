# Long-lived processes: running, finding and cleaning up

Part of `devcontainer:workflow` (`../SKILL.md`): read it before starting a server or dev process in the container, before stopping one, and before finishing a task that started any.

## Foreground vs. backgrounded execution

- **Short, bounded commands** (compiling, unit tests, dependency
  inspection, packaging) — run them as a normal blocking `Bash` call and
  read the output directly.
- **Long-lived or server processes** (dev servers, `run`/`serve` targets,
  anything that starts listening and doesn't exit on its own) — these
  need one of:
  - `timeout <seconds> devcontainer exec --workspace-folder <path> ...`
    (or `timeout <seconds> $CONTAINER_ENGINE exec ...` as fallback) if you only need
    to see the startup log and then let it die naturally, or
  - `run_in_background: true`, then read the result via `TaskOutput`, or
  - `Monitor`, if you need to react to specific log lines (e.g. wait for
    a "started"/"listening" line, or catch a stack trace) without
    blocking the turn.
  Do **not** pipe through `| tail -N` (or similar) and assume the
  pipeline's exit code reflects the underlying process's exit code — the
  tail/grep/etc. at the end of the pipe reports its own exit code
  regardless of what happened upstream. Read the actual output instead of
  trusting `$?`.
- To stop a backgrounded long-lived process, prefer `TaskStop` over
  reaching into the container to `kill` the process by hand.

## Check for existing app processes before starting or stopping anything

Long-lived processes (dev servers, `run`/`serve` targets, watch-mode
build tools) started inside a devcontainer persist across your tool calls
and across turns — the user may already have one running in their own
terminal or via their editor's run/debug UI, entirely independent of
anything you've done. Before starting one yourself, or before killing one
to apply a fix, check what's actually running first:

```bash
devcontainer exec --workspace-folder <path> bash -lc "ps aux | grep -i <process-pattern>"
devcontainer exec --workspace-folder <path> bash -lc "ss -ltnp | grep <port>"
# fallback without CLI:
# $CONTAINER_ENGINE exec <container> bash -lc "ps aux | grep -i <process-pattern>"
# $CONTAINER_ENGINE exec <container> bash -lc "ss -ltnp | grep <port>"
```

**Distinguish the user's own process from one you started or from a
stale leftover** using process ancestry and TTY, not just "is something
listening":
- A process attached to a real TTY (`pts/0`, `pts/1`, ...) in the `ps aux`
  TTY column is almost always the user's own interactive terminal
  session — they typed the command and are watching its output live. Do
  **not** kill this without asking first, even if it's the thing blocking
  your fix from taking effect. Tell them what changed and ask them to
  restart it themselves (Ctrl+C + rerun), or explicitly confirm before
  you stop it for them.
- A process with TTY `?` is detached/backgrounded — either something you
  (or a previous turn) started via `nohup ... &`, or a daemon the
  devcontainer itself launches. These are generally safe to manage
  directly, but confirm *which* one it is (command line, start time, any
  log file path in its args) before assuming it's disposable — it could
  still be a long-running service the user relies on (a database, a
  message broker) rather than the app under test.

This matters most right after a code/config change: the already-running
process has the **old** build/classes/bundle loaded in memory, so a fix
on disk does nothing until that process restarts. If it's the user's own
foreground process, say so explicitly rather than declaring the fix
"verified" — nothing is verified until the code that's actually running
reflects your change.

## Clean up every process you start for your own verification

If you start a server/dev-process yourself (`nohup ... &`,
`run_in_background`, or a bare `devcontainer exec` / `$CONTAINER_ENGINE exec`) purely to verify a fix —
not because the user asked you to leave something running — treat it as
scoped to the task: stop it before considering the task done, the same
way you'd clean up a scratch file.

Containers and images follow `../../../references/containers.md` §9 — which is
about artifacts, not just processes, and applies to every stage that
starts a container.

- Track what you started (PID, container, port) as you go, especially
  across several rounds of restart-to-test-a-fix — it's easy to lose
  count of which PID is your latest one.
- Before ending a task that involved starting test processes, check
  `ps`/`ss` again and stop anything you spun up that has no ongoing
  reason to keep running.
- Never leave a background process holding a port the user's own
  workflow needs (e.g. `:8080`, `:3000`) — a stale agent-started process
  squatting on a port is a confusing, hard-to-diagnose blocker for
  whatever the user tries next, and `ss -ltnp` alone won't tell them the
  listener is something an agent left running rather than their own
  tooling.
- If asked to stop/close/drop "any running process," don't just kill
  whatever's convenient — enumerate everything actually related to the
  app (backend, frontend dev server, any proxy/compose stack brought up)
  and confirm each one is actually stopped (port free, process gone), not
  just that one `kill` was sent.
- **A probe you run to answer a question ("does this feature install?")
  is scratch.** Removing it means the container *and* the images it built
  (`vsc-<workspace>-<hash>-features*`), not stopping it: a stopped
  container still pins its layers, and those images accumulate until the
  host stalls. Snapshot `ps -a` / `images` before you start, remove only
  the difference afterwards, and re-run both to confirm the count really
  dropped — an exit code of 0 proves nothing, especially when rootless
  vs. root or a `DOCKER_HOST` pointing at another store makes cleanup
  succeed against a different engine than the one holding the container.
