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

Read `.devcontainer/devcontainer.json` before running anything and note the
container name (`runArgs --name`, else `$CONTAINER_ENGINE ps`), the user to
exec as (`remoteUser`), the workdir (`/workspaces/<folder>` or
`workspaceFolder`), the toolchain and the env-file wiring. Then ensure it is
running: `devcontainer up --workspace-folder <host-root>` (idempotent); without
the CLI, check `ps` and ask the user to start it rather than hand-rolling
`run`. If which container hosts this project is ambiguous, ask. Read
`references/discovery.md` for the full checklist and commands.

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

Preferred (CLI available — takes user, env and workdir from `devcontainer.json`):

```bash
devcontainer exec --workspace-folder <host-absolute-path-to-project-root> <command>
```

Reuse the same `--workspace-folder` for every command in the session.
Fallback (CLI missing — fill the three placeholders from Step 0):

```bash
$CONTAINER_ENGINE exec -u <remoteUser> --workdir <container-path-matching-cwd> <container-name> <command>
```

`references/exec-recipes.md` has recipes for a fast signal after an edit,
the full suite, what dependencies actually resolved (`mvn dependency:tree`,
`npm ls`, `cargo tree`), inspecting a built artifact and querying the
registry. Drop quiet flags (`-q`) on any command whose output you need to
read; keep them only where you care about the exit code and the tail of a
failure.

## Long-lived processes → `references/processes.md`

Read it before starting, stopping or finishing with a server or dev
process. The rules:

- **Short, bounded commands** run as a normal blocking `Bash` call.
  **Servers** (anything that listens and doesn't exit) run under
  `timeout <s> devcontainer exec …`, with `run_in_background: true` and
  `TaskOutput`, or under `Monitor` to react to a log line. Never trust
  `$?` through a `| tail` pipe — read the output. Stop a backgrounded
  process with `TaskStop`, not a hand-rolled `kill`.
- **Look before starting or stopping** (`ps aux`, `ss -ltnp`). A process on
  a real TTY (`pts/N`) is the user's own terminal: don't kill it without
  asking — say what changed and ask them to restart it. TTY `?` is
  detached (yours, a previous turn's, or a daemon the devcontainer
  launches): confirm which before treating it as disposable. A fix on disk
  does nothing until the running process restarts; nothing is verified
  until the code that's running reflects the change.
- **Clean up what you started for verification** — track PID, container
  and port; stop them before calling the task done, and never leave a
  background process holding a port the user's workflow needs. A probe
  container is scratch too: remove it *and* its images (a stopped
  container still pins layers), snapshot `ps -a`/`images` before and after,
  and re-run both to confirm the count dropped (`containers.md` §9).

## Guardrails

- **Autonomous:** `devcontainer up`, `devcontainer exec`, `up --recreate` after a config change; announce afterwards.
- **Ask first:** stop, `up --remove-existing`, `rm`, `volume rm`, `network rm` on the project's container or volumes (exceptions: your own verification process and the label-scoped removals of `containers.md` §9a).
- **Never:** any `prune`; edit files by shelling into the container (use `Edit`/`Write` on the host); hardcode secrets in tracked config (flag a found one, don't silently fix it).
- Read `references/guardrails.md` before any teardown or rebuild, or when unsure which side a command falls on.

## When the devcontainer itself needs Docker → `references/docker.md`

When tests or builds need a Docker daemon inside the devcontainer
(Testcontainers, builds that shell out to `docker`), read
`references/docker.md`: `docker-outside-of-docker` (the default) vs.
`docker-in-docker`, the both-configured trap, the verification checklist
after changing the access mode, and why Testcontainers cleanup lag isn't
an orphan leak.

## Rebuilding the tools image

A changed Containerfile makes the next `image` recipe build a new image
under the same tag and leaves the previous one dangling, a few gigabytes
each time. The recipe removes the project's own dangling images by label
(`containers.md` §6); when it doesn't yet, remove them yourself —
`$CONTAINER_ENGINE images -q --filter dangling=true --filter
label=me.lucasvmigotto.ai-gent.project=<project>`, then `rmi` each ID
without `-f` — and suggest adding it. Unlabelled dangling images aren't
attributable: list them and ask. An interrupted build leaves buildah
working containers (`ps -a --external`) that pin layers; they go at the
end of the task through `wt.py finish` (`containers.md` §9a).

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
