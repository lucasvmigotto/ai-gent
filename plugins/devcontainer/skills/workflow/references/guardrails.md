# Guardrails

- **Autonomous (CLI available, no prompt needed):** `devcontainer up`
  (create-or-start), `devcontainer exec` (run commands), and
  `devcontainer up --workspace-folder <path> --recreate` (rebuild after a
  `devcontainer.json`/`features`/`image` change). Announce what you did
  after the fact.
- **Ask first:** `$CONTAINER_ENGINE stop` / `devcontainer` stop-like teardown,
  `up --remove-existing`, `$CONTAINER_ENGINE rm` / `$CONTAINER_ENGINE volume rm` /
  `$CONTAINER_ENGINE network rm` on the project's container
  or its volumes — the container likely holds dependency caches,
  in-progress state, or open connections, and destroying it is expensive
  to rebuild and hard to reverse. Exceptions, autonomous: stopping a
  verification process you started yourself (tracked PID/port), and the
  label-scoped removals of `containers.md` §9a — the project's dangling
  images, a landed worktree's volumes and containers (`clean`, `wt.py
  remove`), a wave's new buildah working containers (`wt.py finish`).
- **Never:** any `prune` (`system`, `image`, `volume`, even filtered) —
  remove by ID or label (`containers.md` §9).
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
