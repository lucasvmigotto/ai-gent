# Verification, documentation and pitfalls (Phases 8–9)

Part of `devcontainer:setup` (`../SKILL.md`): read it when verifying a setup, writing its README section, or when something behaves oddly (the pitfalls table).

## Phase 8 — Verification (do not skip — config that "looks right" often isn't)

Bring the whole thing up for real and prove each hop, not just that
containers boot:

1. Every infra container starts, reports healthy, and passes
   `devcontainer:infra`'s per-category round trip.
2. Every recipe (`fmt`, `lint`, `typecheck`, `test`, `build`) passes in
   each module's tools service, with `--network=none` where declared, and
   CI runs the same recipes. `doctor` reports no version drift between
   the tools image and the devcontainer.
3. Limits hold: `$CONTAINER_ENGINE stats --no-stream` shows every service
   capped, and the whole stack stays within the budget agreed with the
   user. On a Podman host, the same recipes pass with Podman.
4. Each module's devcontainer starts and its app connects to infra **by
   network alias**, not just by an exposed host port — that's what
   actually proves the shared-network wiring.
5. In a split: the client, loaded in the host browser, reaches the API
   through the dev-server proxy (Phase 1.3), not a direct alias URL.
6. In a split: **rebuild one side and confirm the other keeps running** —
   the API's dev server survives a `web` container rebuild and vice versa.
   If it doesn't, something is still coupled (a shared container, a shared
   `--name`, a shared volume that one side's `postCreateCommand` rewrites).
7. If three or more containers are in play, confirm resolution from a
   *third* one (neither infra nor the module itself) using the alias.
8. Whatever the app enforces (auth, validation) still behaves correctly
   through the full chain — a dev shortcut that only "works" because a
   check got accidentally bypassed is worse than no dev environment.
9. Hot-reload both directions: edit backend source, confirm a restart in
   the logs without you restarting the process; edit frontend source,
   confirm an HMR log line, not a full page reload.
10. If a reverse-proxy simulation exists, run `devcontainer:proxy`'s smoke
   test with the **real** backend behind it.
11. Clean up every container/volume/image created purely for this
   verification (`$CONTAINER_ENGINE ps -a`, `volume ls`, `images`) per
   `../../../references/containers.md` §9 — an orphaned volume from a
   naming-prefix mistake is easy to leave behind, and so is the pile of
   images a feature probe leaves behind. Remove them; re-run the three
   commands to confirm the counts dropped.

## Phase 9 — Document the decisions, not just the commands

Future readers — including future-you — need the *why*, not only the
config. In the project's README (its *Development* section, per
`../../../references/readme.md`, prerequisites as `[!IMPORTANT]`) or a docs
file, record:

- The setup's moving parts and the order to bring them up (which
  devcontainer to open for which work, in a split).
- Any version gap between the dev simulation and prod (from
  `devcontainer:infra`), stated plainly rather than left implicit.
- **Alternatives considered and rejected, with reasoning.** This is the
  actual deliverable the first time someone asks "why not just do X
  instead" — cheap to write down at design time, expensive to
  reconstruct later. The two that recur most:
  - "Why not one devcontainer for both API and client?" — write the real
    answer for *this* project (separate debuggers/extensions/env, one
    side's rebuild not taking down the other, matching how they ship), or
    — if you did merge — which Phase 1.1 exception applied.
  - "Why a separate tools layer when the devcontainer already has the
    toolchain?" — because the tools image is thin, pinned and identical in
    CI, so its result is the reproducible one; the devcontainer is
    optimized for comfort and is allowed to be heavier.
  - "Why not one single docker-compose.dev.yml for everything, app code
    included?" — typically: it breaks IDE debugging since the app then
    runs in a container the IDE never attached to, it duplicates toolchain
    resolution the devcontainer features already handle, and it diverges
    from the established reference convention. Note where it genuinely
    *would* fit instead (commonly: a CI job that only runs the stack and
    never edits it).

## Pitfalls (observed firsthand — check these before assuming a fresh bug)

| Symptom | Cause | Fix |
|---|---|---|
| A changed default in `.env.example` never takes effect | `initializeCommand`'s seeding only runs when `.env` is **absent** — an existing `.env` is never refreshed | `cp .env.example .env` by hand, then recreate the container (`devcontainer up --recreate` or equivalent) — a plain restart keeps the old `--env-file` values already baked into the running container |
| "Permission denied" / can't write to a cache directory, only on a fresh volume | Newly created named volumes are root-owned; the container's user isn't root | `chown -R <remoteUser>:<remoteUser> <mount-target>` once via `postCreateCommand`, not a manual one-off |
| A `devcontainer.json` edit (features/mounts/image) has no visible effect | Those only apply at container (re)creation | recreate the container explicitly; never fake it with a plain restart |
| Client works from inside the `web` container (`curl http://api:8080`) but the browser gets DNS/CORS errors | The browser runs on the host and can't resolve Docker aliases; client code is calling `http://api:<port>` directly | call a relative path and let the frontend dev server proxy it to the alias (Phase 1.3) |
| Second devcontainer of a split fails with "network not found" | Only the first devcontainer's `initializeCommand` creates the shared network | put the idempotent network-create command in every devcontainer (Phase 2) |
| Workspace dependency (`@repo/shared`, a `packages/*` link) missing inside a module's container | Per-module `.devcontainer/` mounts only the module folder, not the repo root | mount the parent via `workspaceMount` + `workspaceFolder` (Phase 1.3) |
| Passes in the devcontainer, fails in CI (or the other way round) | The layers drifted: a different toolchain version, a globally installed tool, a cache only one side has | run the recipe in the tools service; `doctor` shows the version drift; fix the single version source, never the CI job |
| A background process started only to verify something is still holding a port later | Long-lived dev/server processes outlive the tool call that started them | track what you started as you go and stop it before calling the task done (see `devcontainer:workflow`) |
