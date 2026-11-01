# Step 0: discover the container's actual facts

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
