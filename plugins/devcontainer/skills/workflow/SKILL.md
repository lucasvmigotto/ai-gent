---
name: workflow
description: Operate a project's containers autonomously — tasks through its tools recipes (thin, CI-identical containers), otherwise the devcontainer via its CLI or Podman/Docker exec; file edits on the host; rebuilds after config changes. Use when running, testing or building in a repo with .devcontainer/, a compose.tools.yml or a Containerfile. Creating them is devcontainer:setup.
---

# Working with a devcontainer

Trigger: the project has a `.devcontainer/devcontainer.json` (or
`.devcontainer.json`) and the host is missing the toolchain the project
needs (no `java`/`mvn`, no matching `node`/`python` version, a native
dependency that only exists in the container image, etc.), OR the user
says something like "use the container to do X" / "I don't have `<tool>`
installed locally."

The container and the host almost always share the project files via a
bind mount. That single fact is what drives every rule below.

## Step -2: engine and tools layer — the preferred path

Do this once per session and cache the result (rules in
`../../references/containers.md`):

```bash
CONTAINER_ENGINE="${CONTAINER_ENGINE:-$(command -v podman >/dev/null 2>&1 && echo podman || echo docker)}"
ls justfile Makefile compose.tools.yml 2>/dev/null; just --list 2>/dev/null
```

- **If the project has a tools layer** (`compose.tools.yml` and task
  recipes such as `just test`), run every environment action through the
  recipes: they use the thin, pinned container CI uses, so their result is
  the one that counts. Use the recipe's name (`just lint`, `just test
  module=api`) rather than re-typing the container command. Tasks with no
  recipe run as `$CONTAINER_ENGINE compose -f compose.tools.yml run --rm
  <module>-tools <command>`; suggest adding a recipe when one repeats.
- **The human devcontainer** (below) is for what the tools layer doesn't
  cover: a running dev server with hot reload, a debugger session, or a
  project that has no tools layer yet.
- Every command below uses `$CONTAINER_ENGINE`, never a hardcoded
  `docker`: Podman when installed, Docker otherwise. With Podman, pass
  `--docker-path podman` to the devcontainer CLI.

## Step -1: probe for the `devcontainer` CLI — manage autonomously if present

Do this once at the start of the session and cache the result:

```bash
command -v devcontainer && devcontainer --version
```

- **If available (`CLI_AVAILABLE=true`):** manage the container lifecycle
  yourself without disturbing the user — `up` (create-or-start),
  `exec` (run commands), and `up --recreate` (rebuild after a
  `devcontainer.json`/`features`/`image` change) are all autonomous,
  non-destructive operations. Do not ask the user to start/rebuild via
  VS Code when you can just run the CLI. Announce what you did after
  the fact.
- **If missing or broken (`command -v devcontainer` empty, or
  `devcontainer --version` fails — e.g. a `bun`-installed shim with no
  `node` runtime):** fall back to the manual flow
  below (`$CONTAINER_ENGINE ps` discovery + `$CONTAINER_ENGINE exec`, ask the user to
  start/rebuild in their editor). Mention the install hint exactly once
  per session — prefer `bun`, fall back to `npm`/`yarn` — and never
  auto-install without asking:
  ```bash
  bun install -g @devcontainers/cli
  # fallback:
  npm install -g @devcontainers/cli
  # or: yarn global add @devcontainers/cli
  ```
- Never hand-roll a `$CONTAINER_ENGINE run` to replicate `devcontainer.json` —
  with or without the CLI, that won't reproduce the
  volumes/features/env-file/network wiring.

## Step 0: discover the container's actual facts — don't assume

Every project's devcontainer is configured differently. Before running
anything, read `.devcontainer/devcontainer.json` (or
`.devcontainer/<folder>/devcontainer.json` if there are multiple) and note:

- **Container name.** If `runArgs` includes `["--name", "<x>"]`, that name
  is stable and `$CONTAINER_ENGINE exec -it <x> ...` always works once it's up. If
  there's no explicit name, the tool that started it (VS Code, `devcontainer
  up`) picks a generated one — find it with:
  ```bash
  $CONTAINER_ENGINE ps --format '{{.Names}}\t{{.Image}}\t{{.Status}}'
  ```
  and match it by image name or by grepping for the project name.
- **User to exec as** — the `remoteUser` / `containerUser` field (falls
  back to the image default, often `root`, if unset). Running as the
  wrong user causes file-ownership mismatches on anything the container
  writes back to the bind mount (build output, caches, lockfiles).
- **Workdir inside the container** — normally
  `/workspaces/<workspace-folder-name>` (devcontainer CLI convention),
  but check `workspaceFolder` if it's set explicitly. Map this against the
  host path yourself: the bind-mounted project root on the host
  corresponds 1:1 to this path inside the container; a subdirectory on the
  host is the same subdirectory inside the container.
- **Toolchain/features** — the `features` block and/or base `image` tell
  you what's actually installed (language version, extra CLIs) — useful
  when a build fails in a way that looks version-related.
- **Env file / secrets wiring** — `runArgs: ["--env-file", ".env"]` and/or
  `initializeCommand` often seed a local `.env` from `.env.example` on
  first boot. If the app needs config/secrets, check there before
  guessing at variable names.

Confirm it's actually running before doing anything else:

- **CLI available:** ensure it with one idempotent command (creates if
  missing, starts if stopped, no-op if running) — no user prompt needed:
  ```bash
  devcontainer up --workspace-folder <host-absolute-path-to-project-root>
  ```
  Use the host path that contains `.devcontainer.json` (or the repo root
  when the config lives in `.devcontainer/`). The CLI resolves container
  name, `remoteUser`, and `workspaceFolder` from the config itself, so
  you don't need to derive them by hand for lifecycle/exec purposes.
- **CLI missing:** check manually, then ask the user to start it rather
  than hand-rolling `$CONTAINER_ENGINE run`:
  ```bash
  $CONTAINER_ENGINE ps --format '{{.Names}}' | grep -i <project-or-container-name>
  ```
  Tell the user to start the devcontainer (VS Code "Reopen in
  Container") and wait for confirmation.

If any of the above is genuinely ambiguous from the config (e.g. multiple
containers, unclear which one hosts this project, no obvious name match),
ask the user rather than guessing — running commands in the wrong
container silently produces confusing failures.

## The one rule that matters: split file actions from environment actions

| Kind | Examples | How |
|---|---|---|
| **File actions** — reading or changing anything under version control | edit a config/manifest file, read source, check a diff | **Local tools**: `Read`, `Edit`, `Write`, `git` via `Bash` on the **host** path. Never go through `devcontainer exec` / `$CONTAINER_ENGINE exec`. |
| **Environment actions** — anything that needs the container's toolchain, runtime, or OS | compiling, running the test suite, starting the app/server, resolving package versions, inspecting a build artifact | **In the container.** Prefer `devcontainer exec` when the CLI is available; use `$CONTAINER_ENGINE exec` only as fallback. Never on the host if the host lacks the toolchain. |

Why this split, specifically:

- The bind mount means a `Write`/`Edit` on the host path is visible inside
  the container **instantly**, with no sync step and no stale-cache risk.
  There is essentially never a reason to `devcontainer exec ... cat`,
  `$CONTAINER_ENGINE exec ... cat`, `sed`, `echo >`, or open an editor inside the container just to touch a file —
  that only adds a layer of shell-quoting/escaping risk for zero benefit,
  and it bypasses the host-side diff tooling the user actually reviews.
- Conversely, anything that needs to *execute* code, resolve dependencies,
  or touch a runtime the host doesn't have must run in the container.
- Read-only exploration (`grep`, `find`, viewing source) is fine on the
  host either way, since the files are identical — prefer the host for
  it, since it avoids the exec overhead and quoting entirely.

## The exec template

Preferred (CLI available — handles user/env/workdir from `devcontainer.json` automatically):

```bash
devcontainer exec --workspace-folder <host-absolute-path-to-project-root> <command>
```

Keep reusing the same `--workspace-folder` for every command in the
session — don't re-derive it each time.

Fallback (CLI missing — fill in the three placeholders from Step 0):

```bash
$CONTAINER_ENGINE exec -u <remoteUser> --workdir <container-path-matching-cwd> <container-name> <command>
```

Generic recipes (substitute the project's actual build tool — `mvn`,
`npm`/`yarn`/`pnpm`, `pytest`/`tox`, `cargo`, `go test`, etc. — and use
the `devcontainer exec` form whenever the CLI is present):

```bash
# Fast signal after an edit — whatever the lightest "does this parse/compile" step is
devcontainer exec --workspace-folder <path> <build-tool> <compile-or-build-check>
# fallback: $CONTAINER_ENGINE exec -u <user> --workdir <path> <container> <build-tool> <compile-or-build-check>

# Full test suite
devcontainer exec --workspace-folder <path> <build-tool> test
# fallback: $CONTAINER_ENGINE exec -u <user> --workdir <path> <container> <build-tool> test

# Inspect exactly what versions/deps actually resolved (the equivalent of
# `mvn dependency:tree`, `npm ls <pkg>`, `pip show`, `cargo tree`, etc.) —
# this is how classpath/version-conflict bugs get diagnosed, and it beats
# guessing from the manifest file alone, since transitive resolution can
# silently override what the top-level manifest says
devcontainer exec --workspace-folder <path> <dependency-tree-equivalent>

# Look inside a built artifact for what actually got embedded
devcontainer exec --workspace-folder <path> <artifact-inspection-command>

# Query the package registry directly from inside the container when you
# need to know what versions exist, or what a dependency's own manifest
# requires transitively
devcontainer exec --workspace-folder <path> curl -s <registry-metadata-url>
```

Watch out for verbosity flags that suppress the exact output you need:
e.g. Maven's `dependency:tree` logs at INFO through the Maven logger, so a
blanket `-q` silences it entirely. Drop quiet/silent flags for any command
whose output you actually need to read; keep them only for high-volume,
low-signal steps where you mainly care about the exit code and the tail of
any failure.

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

## Guardrails

- **Autonomous (CLI available, no prompt needed):** `devcontainer up`
  (create-or-start), `devcontainer exec` (run commands), and
  `devcontainer up --workspace-folder <path> --recreate` (rebuild after a
  `devcontainer.json`/`features`/`image` change). Announce what you did
  after the fact.
- **Ask first:** `$CONTAINER_ENGINE stop` / `devcontainer` stop-like teardown,
  `up --remove-existing`, `$CONTAINER_ENGINE rm` / `$CONTAINER_ENGINE volume rm` /
  `$CONTAINER_ENGINE system prune` / `$CONTAINER_ENGINE network rm` on the project's container
  or its volumes — the container likely holds dependency caches,
  in-progress state, or open connections, and destroying it is expensive
  to rebuild and hard to reverse. Exception: stopping a verification
  process you started yourself (tracked PID/port) is autonomous — clean
  it up without asking.
- Don't edit files by shelling into the container (`devcontainer exec ...
  vi`, `$CONTAINER_ENGINE exec ... vi`, `sed -i`, `tee`, heredocs) — see the file-vs-environment split above. If
  you catch yourself reaching for exec to change file content,
  stop and use `Edit`/`Write` on the host path instead.
- Secrets (connection strings, API keys, credentials) belong in whatever
  gitignored env file the project already uses (check for a `.env` +
  `.env.example` pair and how `devcontainer.json` wires it in), not
  hardcoded into a tracked config file. If you find a real credential
  inline in a version-controlled file, flag it to the user rather than
  silently "fixing" it — it may be an intentional, not-yet-committed local
  edit.

## When the devcontainer itself needs Docker → `references/docker.md`

When tests or builds need a Docker daemon inside the devcontainer
(Testcontainers, builds that shell out to `docker`), read
`references/docker.md`: `docker-outside-of-docker` (the default) vs.
`docker-in-docker`, the both-configured trap, the verification checklist
after changing the access mode, and why Testcontainers cleanup lag isn't
an orphan leak.

## Rebuilding after a `devcontainer.json` change

Changing `features`/`image`/mounts in `devcontainer.json` doesn't take
effect until the container is rebuilt.

- **CLI available:** rebuild autonomously without disturbing the user —
  do not ask them to rebuild via VS Code:
  ```bash
  devcontainer up --workspace-folder <host-absolute-path-to-project-root> --recreate
  ```
  Then verify the change took effect (for a Docker access-mode change, the
  checklist in `references/docker.md`). Never fake a rebuild with
  `$CONTAINER_ENGINE restart` or by hand-editing the running container's state —
  that doesn't re-run feature installation.
- **CLI missing or broken (`command -v devcontainer` empty, or
  `devcontainer --version` fails):** there is no way to
  trigger that rebuild programmatically — don't fake it with
  `$CONTAINER_ENGINE restart` or by hand-editing the running container's state.
  Tell the user to rebuild via their editor ("Reopen in Container" /
  "Rebuild Container" in VS Code) and wait for their confirmation
  before running any verification. Suggest the bun-first install hint
  from Step -1 so future rebuilds can be autonomous.

## Worked example → `references/worked-example.md`

For a runtime or classpath failure (works in one place, fails in the
container; version conflicts), `references/worked-example.md` walks
through the diagnosis end to end using the exec template above.
