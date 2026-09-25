---
name: setup
description: Design and scaffold container-first development — per-module multi-stage Containerfiles with a thin tools stage that agents and CI run, task recipes, resource limits, Podman or Docker — plus human devcontainers, API and client apart by default. Use for "set up a devcontainer", "containerize the dev environment", "split the devcontainer into api and web".
---

# Designing devcontainer(s) for a project

Scope: the one-time (or per-major-change) design work of deciding how many
devcontainers a project needs, what each one contains, how they reach each
other and any infra they depend on, and how hot-reload fits in.

**Container-first.** Read `../../references/containers.md` before
anything else. Each module gets two environments built from the same
pinned toolchain: a thin `tools` image that agents and CI run every task
in (the reference environment), and a comfortable devcontainer for the
person at the keyboard. Everything below applies to both unless a phase
says otherwise; the engine is Podman when installed, Docker otherwise
(`$CONTAINER_ENGINE`), never hardcoded.

Sibling subskills own the rest:

- `devcontainer:infra` — simulating external resources (databases, caches,
  queues, storage, SMTP/mail, OAuth2/OIDC identity, and beyond). Phase 5
  below hands off to it.
- `devcontainer:proxy` — simulating a production reverse proxy. Phase 7
  below hands off to it.
- `devcontainer:workflow` — day-to-day lifecycle/exec work once a
  devcontainer exists.

Commit/branch mechanics throughout all phases follow `git:workflow` — this
skill doesn't repeat those rules.

## Phase 0 — Discovery

- Inventory the target project: module boundaries, the language/toolchain
  each module actually uses, existing Dockerfiles/Containerfiles/compose
  files/CI configs, task runners (`justfile`, Makefile, `package.json`
  scripts), and whether partial devcontainer config already exists (don't
  discard it before understanding why it looks the way it does).
- Detect the container engine (`containers.md` §1) and, for Podman,
  whether cgroup v2 limits and the user socket are available. Check the
  hardened image catalog for each module's runtime and version
  (`containers.md` §3) and note where a fallback image is needed.
- **Classify the project's shape before anything else** — Phase 1 keys
  off this answer:
  - **API + client**: a backend module serving an HTTP/gRPC API and a
    frontend module consuming it, in separate folders or packages — even
    if both are TypeScript, even if they sit in one monorepo workspace.
  - **Full-stack single app**: one framework serves UI and server code
    from one process/build (Next.js, Nuxt, SvelteKit, Remix, Rails or
    Django or Laravel with server-rendered views/Inertia, a frontend with
    its API routes/BFF inside the same package).
  - **Other**: a single service, a library, a CLI, workers — no client
    module at all.
  Evidence: separate manifests (`api/pom.xml` + `web/package.json`),
  separate Dockerfiles or CI jobs, separate deploy targets, a client that
  calls the API through a base URL/proxy instead of importing server code.
- **If reference/sibling projects exist** (same org, same monorepo family,
  a previous project by the same team), read their
  `.devcontainer/devcontainer.json`, compose files, and any reverse-proxy
  config *before* designing anything. An established convention other
  people already know how to operate beats a novel one, even a better one.
  Separate what's genuinely project-specific in those references from what's
  boilerplate worth copying as-is.
- Identify the project's **own** production facts that any later simulation
  must match, not approximate: real reverse-proxy path prefixes, real env
  var names, real service names/ports, the real prod version of anything
  you're about to simulate with a substitute. These live in the project's
  own `.env.example`, existing proxy config, CI/CD files, or deployment
  manifests — never invent a plausible-looking stand-in when the real value
  is one file-read away.
- **Read the stack precisely per module**, not just "this is a Java
  project": exact language version (from the manifest —
  `pom.xml`/`build.gradle`, `package.json` engines field, `pyproject.toml`,
  `go.mod`, `Cargo.toml`, `*.csproj`), package manager actually in use
  (npm vs. yarn vs. pnpm, pip vs. poetry vs. uv), and every formatter/linter
  config already checked in (`.editorconfig`, `.prettierrc*`,
  `eslint.config.*`/`.eslintrc*`, a Java `checkstyle.xml` or formatter
  profile, `.rustfmt.toml`, `ruff`/`black` config in `pyproject.toml`,
  `.golangci.yml`, etc.). This is what Phase 3's IDE settings and feature
  choices key off — guessing a version or reusing a default formatter
  config that doesn't match what's actually enforced produces a devcontainer
  that fights the project's own CI lint step.
- **Check reference image/feature catalogs before picking anything**, the
  same way you already check sibling projects for convention:
  - `https://containers.dev/features` — the official
    `ghcr.io/devcontainers/features/*` catalog (languages, Docker access,
    CLIs). Treat this as the default source for features.
  - If this user has their own curated devcontainer image family
    (e.g. `lucasvmigotto/devenv`, published at
    `https://devenv.lucasvmigotto.me` and `ghcr.io/lucasvmigotto/devenv`) —
    check its current image catalog for a matching language+distro tag
    before defaulting to a generic `mcr.microsoft.com/devcontainers/*`
    base. It's worth preferring when available: non-root `developer` user
    (UID 1000) baked in, passwordless `sudo`/`doas`, a real shell (ZSH +
    Spaceship, Nerd Fonts) instead of a bare `bash`, and — notably —
    dependency-cache directories already declared as owned `VOLUME`s per
    language, which sidesteps the fresh-volume-ownership pitfall in Phase 3
    entirely for whatever cache paths that image already declares. The
    catalog and tag scheme evolve, so read the current docs/README rather
    than hardcoding a remembered tag.
  - Either way, don't add a feature that duplicates what the chosen base
    image already provides (e.g. a `java` feature on top of a
    `devenv:java-*` base) — that's redundant at best and a version
    conflict at worst. Features are for what the base image *doesn't*
    already bake in.

## Phase 1 — How many devcontainers?

### 1.1 API + client → one devcontainer each (the priority rule)

**If Phase 0 classified the project as API + client, give the API and the
client their own devcontainer by default — even when both use the same
language/runtime.** A matching toolchain is not a reason to merge. The two
sides still differ in everything a devcontainer actually configures:

- debugger and launch config (a JVM/Node inspector attached to a server vs.
  a browser debugger),
- IDE extension set and formatter settings,
- forwarded ports and network alias (`api` vs. `web`),
- env file (DB/secret wiring vs. public `VITE_*`/`NEXT_PUBLIC_*` values),
- watch process and its restart behavior,
- lifecycle — a merged container means rebuilding the backend for a new
  native dependency also tears down the frontend's dev server, and vice
  versa.

Separate containers keep each of those local to its side, and they mirror
how the two artifacts ship. The shared network (Phase 2) is what keeps them
talking; nothing is lost by the split.

A single shared devcontainer is right **only** when Phase 0 found one of:

- a **full-stack framework** serving UI and server code from one
  process/build (Next.js, Nuxt, SvelteKit, Remix, Rails/Django/Laravel with
  server-rendered views or Inertia),
- a **BFF or API routes that live inside the frontend package** itself
  (same manifest, same build, same dev server) — that's one app, not two,
- a **single-language monolith with no separate client module**.

"Both are TypeScript" or "a frontend and its backend built by the same
tool" is *not* on that list when they're separate modules — split them.

### 1.2 Everything else — signals table

For modules that aren't an API/client pair (workers, libraries, CLIs,
multiple backend services):

| Signal | One devcontainer | One per module |
|---|---|---|
| Toolchains | Same language/runtime everywhere | Different languages/runtimes |
| IDE workflow | Whole repo opened as one window | Each module opened as its own window, own debugger, own extension set |
| Reference convention | N/A | Sibling projects already split this way |
| Build/deploy coupling | Built/tested together via one root tool | Modules built and shipped as independent artifacts |

Default to splitting along the same lines the project already ships
independently along — if two modules produce two separate deployable
artifacts, they get two separate devcontainers.

**When signals conflict, the API + client rule, toolchain, and
build/deploy-coupling outrank reference convention.** A sibling project that
merged a backend and a frontend into one devcontainer is not itself proof
that merging is right for *this* project — it may just be a call nobody
revisited. Read a sibling's choice as evidence of *style* (base images,
feature versions, IDE settings, extension list) to reuse, not as license to
skip the split.

### 1.3 Laying out a split

- **Where the configs live:**
  - Monorepo (one repo root, shared tooling): one subfolder per module
    under the root —
    `.devcontainer/api/devcontainer.json`, `.devcontainer/web/devcontainer.json`.
    VS Code's "Reopen in Container" offers a picker; the CLI takes
    `devcontainer up --workspace-folder . --config .devcontainer/api/devcontainer.json`.
  - Independent module folders (each with its own manifest, no shared
    root tooling): a `.devcontainer/` inside each module (`api/.devcontainer/`,
    `web/.devcontainer/`), opened as separate windows.
- **Mount the repo root when modules share it.** With the subfolder layout
  the repo root is already mounted; set
  `"workspaceFolder": "/workspaces/${localWorkspaceFolderBasename}/api"` so
  the window opens in its own module. With per-module `.devcontainer/`
  folders that still need a root lockfile or `packages/*` workspace, mount
  the parent explicitly:
  ```json
  "workspaceMount": "source=${localWorkspaceFolder}/..,target=/workspace,type=bind",
  "workspaceFolder": "/workspace/api"
  ```
  Otherwise workspace-linked dependencies silently fail to resolve inside
  the container.
- **Share without merging.** Put what both sides genuinely have in common
  (base image, a common Dockerfile, shared features) in one place both
  configs reference; keep ports, extensions, env files, volumes
  (`<project>-api-*` vs. `<project>-web-*`) and lifecycle commands per
  side.
- **The browser is not on the Docker network.** The client's code runs in
  the host's browser, which can't resolve the `api` alias. Route browser
  API calls through a relative path the frontend dev server proxies
  (Vite `server.proxy`, Angular `proxy.conf.json`, CRA `proxy`, …) to
  `http://api:<port>` — the dev server runs in the `web` container and
  *can* resolve it. Don't hardcode `http://api:<port>` into client code or
  a `VITE_API_URL` the browser will fetch directly.
- The frontend dev server must bind `0.0.0.0` (not `localhost`) or the
  forwarded port and the Phase 7 proxy can't reach it.

### 1.4 Two layers per module

Whatever Phase 1 decided for devcontainers applies to the tools layer too:
an API and a client get separate `api-tools` and `web-tools` services
built from their own Containerfiles. The layers differ only in comfort —
same toolchain versions, same lockfiles, same network aliases.

## Phase 2 — Shared network topology

- Create one external network named after the project, shared by
  every devcontainer, every tools service and every infra compose stack — this is what lets a
  container started by one devcontainer resolve a container started by
  another (or by an infra compose) by name.
- Each devcontainer joins it and registers a stable alias other services
  will use to reach it:
  ```json
  "runArgs": ["--network", "<project-network>", "--network-alias", "<short-alias>"]
  ```
  Pick the alias for what it *is* to callers (`api`, `web`), not the
  generated container name — the alias is the contract other config files
  (datasource URLs, dev-server proxy targets, proxy `proxy_pass` targets)
  will hardcode.
- Create the network idempotently so first-time setup needs no manual step,
  with whichever engine the host has:
  ```json
  "initializeCommand": {
    "network": "sh -c 'e=${CONTAINER_ENGINE:-$(command -v podman >/dev/null 2>&1 && echo podman || echo docker)}; $e network inspect <project-network> >/dev/null 2>&1 || $e network create <project-network>'"
  }
  ```
  Put this in **every** devcontainer of a split — whichever one is opened
  first must be able to create it.

## Phase 3 — The tools layer: Containerfile, compose.tools.yml, recipes

Per module, following `containers.md` §2–§7:

- **One multi-stage Containerfile** (`base` → `deps` → `tools` → `build`
  → `runtime`), hardened images pinned by digest, sources and caches
  mounted instead of copied, a deny-by-default `.dockerignore`. If the
  module already has a production Dockerfile, extend it with the missing
  stages rather than writing a second one.
- **`compose.tools.yml`** with one `<module>-tools` service per module:
  `build.target: tools`, the repo bind-mounted, named cache volumes,
  `cpus`/`mem_limit`/`pids_limit`, the shared network, profiles for
  optional parts.
- **Task recipes** (`justfile`, or the runner the project already uses)
  with the standard names — `fmt`, `lint`, `typecheck`, `test`, `build`,
  `doctor`, `shell` — each running in the tools service; `--network=none`
  for unit tests and builds once dependencies are resolved.
- **One source for versions** (build args, `.tool-versions`, `.nvmrc`,
  the manifest's engines field) that the Containerfile and the
  devcontainer both read; `doctor` prints the versions from both layers
  and fails on drift.

## Phase 3b — The human layer: per-module devcontainer.json

The devcontainer is for people: a comfortable shell, completion and IDE
support are its job, so a heavier image is fine here. It must still use
the toolchain versions from the single source above, join the same
network, and run the same recipes when someone wants CI's answer.

- **Pick the base image deliberately, in this order of preference:**
  1. A matching image from the user's own curated catalog (Phase 0), if
     one exists for this module's exact language+version+distro — it
     arrives non-root, shell-configured, and cache-volume-aware for free.
  2. An official `mcr.microsoft.com/devcontainers/*` base/language image
     with `ghcr.io/devcontainers/features/*` layered on for anything extra
     the base doesn't include.
  3. A project-specific Dockerfile only when neither covers a genuine
     requirement (an unusual native dependency, an internal base image
     mandated by the org) — the most to maintain, so the last resort, not
     the default.
  Never hand-roll what a feature or catalog image already solves.
- **Container engine access inside the container**, only if this
  module's own build/tests need it (Testcontainers, a container-based
  build step): with Podman, forward the user socket and set `DOCKER_HOST`
  (`containers.md` §1); with Docker, default to `docker-outside-of-docker`
  over `docker-in-docker` —
  it reuses the host daemon, spun-up containers are visible as ordinary
  siblings on the host for debugging, and it avoids a duplicated image
  cache. Reach for `docker-in-docker` only when isolation from the host
  daemon is an actual requirement. (`devcontainer:workflow` has the fuller
  troubleshooting writeup.)
- **Two kinds of mount, two different volumes** — don't conflate them:
  - editor/extension cache (e.g. `/root/.vscode-server/extensions` or the
    IDE's equivalent) so extensions aren't re-downloaded on every rebuild.
  - dependency/build cache (`~/.m2`, `~/.cache/...`, a package manager's
    global store, etc.) so a rebuild doesn't cold-start dependency
    resolution.
  Name each volume `<project>-<module>-<purpose>` so it survives rebuilds
  without colliding with a sibling module's or project's volumes on the
  same host.
  **Ownership pitfall:** a freshly created named volume is root-owned; if
  `remoteUser`/`containerUser` isn't root, the first write into it fails.
  Fix it once, permanently, via `postCreateCommand` — don't leave it as a
  one-off manual fix future setups will hit again:
  ```json
  "postCreateCommand": {
    "cache-ownership": "sudo chown -R <remoteUser>:<remoteUser> <mount-target>"
  }
  ```
  Skip this fix only for a cache path the base image itself already
  declares as an owned `VOLUME` for the developer user (some curated
  catalog images do this per language) — confirm that before assuming it's
  needed, and still apply it for any mount you add that isn't one of those.
- **Formatting/linting settings mirror what the project already
  enforces, never a fresh default.** For each detected config from
  Phase 0 (`.editorconfig`, `.prettierrc*`, `eslint.config.*`, a Java
  formatter profile/`checkstyle.xml`, `.rustfmt.toml`, `ruff`/`black`
  config, `.golangci.yml`, ...):
  - install the matching IDE extension (the formatter/linter's own, not a
    generic substitute),
  - point `editor.defaultFormatter` (or the language's equivalent setting)
    at it explicitly rather than leaving the IDE to guess among several
    installed formatters,
  - turn on `editor.formatOnSave` (or the per-language override) so the
    container's formatting matches what CI will check, not a superset or
    subset of it,
  - if the project enforces a style guide document (e.g. a checkstyle
    ruleset hosted at a URL, an internal lint config package), wire the
    setting to that exact source instead of a bundled default that happens
    to be close.
  A devcontainer whose formatter reformats differently than CI's lint step
  produces spurious diffs on every save — treat a mismatch here as a bug,
  not a minor detail.
- **IDE customizations** (settings/extensions) beyond formatting — debugger,
  framework tooling, and anything the module owns that a reference
  project's list wouldn't have (e.g. an e2e test runner's extension, if
  this module owns those tests). In a split, each side lists only its own.
- **Enable autobuild/watch mode** where the stack supports it (an
  "autobuild on save" IDE setting, or the language's own watcher). This is
  also a prerequisite for Phase 6's hot-reload — wire it here so it isn't
  forgotten and rediscovered later as a "why doesn't hot-reload work" bug.

## Phase 4 — Env & secrets

- `.env.example` committed with safe placeholders; `.env` gitignored.
  **Check, don't assume** — verify each module's own `.gitignore` actually
  excludes `.env`; it's been seen missing entirely on a module added later
  than the others.
- In a split, each side gets its **own** `.env`/`.env.example` — the
  client's holds only values that are safe to ship to a browser.
- Wire it into the container with `"runArgs": ["--env-file", ".env"]`.
- Seed `.env` from `.env.example` **only when absent**, never overwrite an
  existing one:
  ```json
  "initializeCommand": { "env-file": "cat .env >/dev/null 2>&1 || cp .env.example .env" }
  ```
  Consequence worth flagging to whoever edits defaults later: changing
  `.env.example` afterwards does **not** refresh an existing `.env`,
  including one already baked into a running container's environment (see
  Pitfalls).

## Phase 5 — External resources → `devcontainer:infra`

For anything a module talks to over the network that isn't another module
of this project — databases, caches, queues, object storage, SMTP/outbound
mail, OAuth2/OIDC identity providers, search, and the rest — load
`devcontainer:infra` and follow it. It joins its compose stack to the
Phase 2 network, so pass it the network name and module aliases decided
here.

## Phase 6 — Hot-reload, wired end to end

Two independent halves — verify both, since one working never implies the
other does:

- **Compiled/backend side:** add the stack's own watch-and-restart
  mechanism (a dev-tools dependency, `nodemon`, `air`, etc.) as a
  dev-only/optional dependency, and confirm the IDE's autobuild setting
  from Phase 3 is actually what feeds it — a watcher with autobuild off
  never sees a change land.
- **Interpreted/frontend side:** the dev server's own HMR usually needs no
  new dependency, but does need any reverse proxy sitting in front of it
  (Phase 7) to pass websocket upgrade headers through, or HMR silently
  degrades to full page reloads without ever surfacing an error.

## Phase 7 — Reverse proxy → `devcontainer:proxy`

Only if production routes through a path-based reverse proxy: load
`devcontainer:proxy`. Skip entirely otherwise.

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
11. Clean up every container/volume created purely for this verification
   (`$CONTAINER_ENGINE ps -a`, `$CONTAINER_ENGINE volume ls`) — an
   orphaned volume from a naming-prefix mistake is easy to leave behind.

## Phase 9 — Document the decisions, not just the commands

Future readers — including future-you — need the *why*, not only the
config. In the project's README or a docs file, record:

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
