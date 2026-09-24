---
name: workflow
description: Operate a project's existing devcontainer autonomously — toolchain commands inside it via the devcontainer CLI (up, exec, rebuild) or docker exec, file edits on the host, rebuilds after config changes. Use when running, testing or building in a repo that has .devcontainer/. Creating one is devcontainer:setup.
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
  below (`docker ps` discovery + `docker exec`, ask the user to
  start/rebuild in their editor). Mention the install hint exactly once
  per session — prefer `bun`, fall back to `npm`/`yarn` — and never
  auto-install without asking:
  ```bash
  bun install -g @devcontainers/cli
  # fallback:
  npm install -g @devcontainers/cli
  # or: yarn global add @devcontainers/cli
  ```
- Never hand-roll a `docker run` to replicate `devcontainer.json` —
  with or without the CLI, that won't reproduce the
  volumes/features/env-file/network wiring.

## Step 0: discover the container's actual facts — don't assume

Every project's devcontainer is configured differently. Before running
anything, read `.devcontainer/devcontainer.json` (or
`.devcontainer/<folder>/devcontainer.json` if there are multiple) and note:

- **Container name.** If `runArgs` includes `["--name", "<x>"]`, that name
  is stable and `docker exec -it <x> ...` always works once it's up. If
  there's no explicit name, the tool that started it (VS Code, `devcontainer
  up`) picks a generated one — find it with:
  ```bash
  docker ps --format '{{.Names}}\t{{.Image}}\t{{.Status}}'
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
  than hand-rolling `docker run`:
  ```bash
  docker ps --format '{{.Names}}' | grep -i <project-or-container-name>
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
| **File actions** — reading or changing anything under version control | edit a config/manifest file, read source, check a diff | **Local tools**: `Read`, `Edit`, `Write`, `git` via `Bash` on the **host** path. Never go through `devcontainer exec` / `docker exec`. |
| **Environment actions** — anything that needs the container's toolchain, runtime, or OS | compiling, running the test suite, starting the app/server, resolving package versions, inspecting a build artifact | **In the container.** Prefer `devcontainer exec` when the CLI is available; use `docker exec` only as fallback. Never on the host if the host lacks the toolchain. |

Why this split, specifically:

- The bind mount means a `Write`/`Edit` on the host path is visible inside
  the container **instantly**, with no sync step and no stale-cache risk.
  There is essentially never a reason to `devcontainer exec ... cat`,
  `docker exec ... cat`, `sed`, `echo >`, or open an editor inside the container just to touch a file —
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
docker exec -u <remoteUser> --workdir <container-path-matching-cwd> <container-name> <command>
```

Generic recipes (substitute the project's actual build tool — `mvn`,
`npm`/`yarn`/`pnpm`, `pytest`/`tox`, `cargo`, `go test`, etc. — and use
the `devcontainer exec` form whenever the CLI is present):

```bash
# Fast signal after an edit — whatever the lightest "does this parse/compile" step is
devcontainer exec --workspace-folder <path> <build-tool> <compile-or-build-check>
# fallback: docker exec -u <user> --workdir <path> <container> <build-tool> <compile-or-build-check>

# Full test suite
devcontainer exec --workspace-folder <path> <build-tool> test
# fallback: docker exec -u <user> --workdir <path> <container> <build-tool> test

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
    (or `timeout <seconds> docker exec ...` as fallback) if you only need
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
# docker exec <container> bash -lc "ps aux | grep -i <process-pattern>"
# docker exec <container> bash -lc "ss -ltnp | grep <port>"
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
`run_in_background`, or a bare `devcontainer exec` / `docker exec`) purely to verify a fix —
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
- **Ask first:** `docker stop` / `devcontainer` stop-like teardown,
  `up --remove-existing`, `docker rm` / `docker volume rm` /
  `docker system prune` / `docker network rm` on the project's container
  or its volumes — the container likely holds dependency caches,
  in-progress state, or open connections, and destroying it is expensive
  to rebuild and hard to reverse. Exception: stopping a verification
  process you started yourself (tracked PID/port) is autonomous — clean
  it up without asking.
- Don't edit files by shelling into the container (`devcontainer exec ...
  vi`, `docker exec ... vi`, `sed -i`, `tee`, heredocs) — see the file-vs-environment split above. If
  you catch yourself reaching for exec to change file content,
  stop and use `Edit`/`Write` on the host path instead.
- Secrets (connection strings, API keys, credentials) belong in whatever
  gitignored env file the project already uses (check for a `.env` +
  `.env.example` pair and how `devcontainer.json` wires it in), not
  hardcoded into a tracked config file. If you find a real credential
  inline in a version-controlled file, flag it to the user rather than
  silently "fixing" it — it may be an intentional, not-yet-committed local
  edit.

## When the devcontainer itself needs Docker (Testcontainers, Docker-based builds/tests)

Trigger: the project's test suite or build needs a Docker daemon
*inside* the devcontainer itself — not just a language toolchain — most
commonly Testcontainers-based integration tests, or any build step that
shells out to `docker`.

Two devcontainer features give a container access to Docker:

- **`docker-outside-of-docker`** — reuses the **host's** daemon. It
  auto-mounts the host socket to `/var/run/docker-host.sock`, creates a
  `/var/run/docker.sock` symlink to it, and wires the container's
  default user into a `docker` group for non-root access. Prefer this
  by default: it's simpler, avoids duplicating image/layer storage
  inside the container, and containers started by tests are visible
  as normal siblings on the host (easier to debug, and Testcontainers'
  Ryuk reaper cleans them up the same way it would outside a container).
- **`docker-in-docker`** — starts a separate, nested `dockerd` inside
  the container, fully isolated from the host's Docker state. Only
  reach for this when isolation from the host daemon is an actual
  requirement (e.g. the tests build/tear down Docker itself, or must
  not see host-side images/containers/networks) — it costs extra disk
  for a redundant image cache and an extra daemon process for no
  benefit in the common case.

**If a project's `devcontainer.json` has both** — a `docker-in-docker`
feature *and* a manual `mounts` bind of `/var/run/docker.sock` — the
bind mount wins in practice: whatever talks to `/var/run/docker.sock`
reaches the host daemon, and the nested `dockerd` the feature provisions
sits unused. Confirm which daemon is actually in play with
`docker ps -a` inside the container: if it shows the devcontainer's own
container as a sibling, you're on the host daemon, and the
`docker-in-docker` feature is dead weight that should either be dropped
in favor of `docker-outside-of-docker`, or the redundant manual mount
should go and the setup made intentional one way or the other.

When switching to `docker-outside-of-docker`, remove any manual socket
`mounts` entry — the feature handles that mount itself, and a manual one
alongside it is redundant (harmless if it happens to agree, but
confusing to read and a landmine if the feature's internal path ever
changes).

### Rebuilding after a `devcontainer.json` change

Changing `features`/`image`/mounts in `devcontainer.json` doesn't take
effect until the container is rebuilt.

- **CLI available:** rebuild autonomously without disturbing the user —
  do not ask them to rebuild via VS Code:
  ```bash
  devcontainer up --workspace-folder <host-absolute-path-to-project-root> --recreate
  ```
  Then run the verification checklist below. Never fake a rebuild with
  `docker restart` or by hand-editing the running container's state —
  that doesn't re-run feature installation.
- **CLI missing or broken (`command -v devcontainer` empty, or
  `devcontainer --version` fails):** there is no way to
  trigger that rebuild programmatically — don't fake it with
  `docker restart` or by hand-editing the running container's state.
  Tell the user to rebuild via their editor ("Reopen in Container" /
  "Rebuild Container" in VS Code) and wait for their confirmation
  before running any verification. Suggest the bun-first install hint
  from Step -1 so future rebuilds can be autonomous.

### Verification checklist after a rebuild that changes Docker access mode

- `devcontainer exec --workspace-folder <path> docker info` (fallback:
  `docker exec ... docker info`) — confirm the daemon identity /
  server version matches the host's, not a freshly-provisioned nested
  one.
- Confirm non-root access **explicitly**, not just as root: check
  `id`/`groups` for the container's actual default user (plain
  `devcontainer exec` already runs as that user — don't use
  `docker exec -u root`, not with `sudo`), and run a plain `docker ps`
  as that user. Root can talk to the socket regardless of group
  permissions, so testing only as root can hide a permissions
  regression that would bite a normal dev session.
- Run the full test suite end-to-end (unit tests *and* the
  Testcontainers/Docker-backed integration tests), not just a Docker
  smoke command — the goal is confirming the actual test workload
  still works, not just that `docker ps` succeeds.

### Testcontainers cleanup latency isn't an orphan leak

Right after a test run finishes, `docker ps -a` can still show the
spun-up test container(s) and the Ryuk reaper container as `Up` for a
short while (observed ~15s). This is normal reaper latency, not a
leaked/orphaned container. Wait and re-check before concluding cleanup
is broken.
## Worked example: diagnosing a runtime/classpath failure

This is the general shape of a real fix done this way — useful as a
template for the next dependency/runtime bug in any language:

1. A build/run command fails with a runtime error (missing class, module
   not found, symbol mismatch, etc.). Read the **full** error/stack
   trace — the first "caused by" line is rarely the interesting one; the
   deepest cause usually is.
2. Use the project's dependency-tree-equivalent (in the container) to see
   what versions **actually** resolved, not just what the top-level
   manifest declares — transitive dependency management can silently
   override it.
3. If two dependencies need incompatible versions of something they share
   transitively, check what version each one's own manifest expects
   (read it from the container's local package cache, or fetch it from
   the registry) rather than guessing at a fix.
4. Make the fix with `Edit` on the host manifest file — never inside the
   container.
5. Re-run the dependency-tree-equivalent (container) to confirm the
   resolved versions now match expectations, then run the test suite and
   actually start the app (backgrounded, read via `TaskOutput`) to confirm
   it boots — a successful compile only proves the code compiles, not that
   the dependency graph is runtime-consistent.
6. Diff the host manifest (`git diff`) to show the user exactly what
   changed, and leave a short comment in the manifest explaining any
   non-obvious version constraint — future readers won't know why a
   version looks "downgraded" otherwise.
