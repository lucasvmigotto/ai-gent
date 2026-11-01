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

Inventory before designing; read `references/discovery.md` for the full list
(stack and exact versions, engine, sibling projects, production facts, image
catalogs). **Classify the project's shape first — Phase 1 keys off it:**
**API + client** (a backend serving an API and a frontend consuming it, in
separate folders or packages — even if both are TypeScript); **full-stack
single app** (one framework serves UI and server code from one process or
build); **other** (a service, library, CLI, workers). A sibling project's
convention is evidence of *style*, not license to skip a split.

## Phase 1 — How many devcontainers?

**API + client → one devcontainer each, by default** — even when both use
the same language or runtime. A matching toolchain is not a reason to
merge; the sides still differ in everything a devcontainer configures:
debugger and launch config, IDE extensions and formatter settings,
forwarded ports and network alias (`api` vs `web`), env file (secrets vs
public `VITE_*`/`NEXT_PUBLIC_*`), the watch process, and lifecycle (a
merged container means a backend rebuild tears down the frontend's dev
server too). Separate containers keep each local to its side and mirror
how the artifacts ship; the shared network (Phase 2) keeps them talking.

One shared devcontainer is right **only** for a full-stack framework, a
BFF or API routes inside the frontend package (one manifest, one build,
one dev server), or a single-language monolith with no client module.
"Both are TypeScript" is *not* on that list when they are separate
modules — split them.

For anything else (workers, libraries, CLIs, several backends), split
along the lines the project already ships independently along: two
deployable artifacts, two devcontainers. When signals conflict, the API +
client rule, toolchain and build/deploy coupling outrank a sibling's
convention. `references/split-layout.md` has the signals table and how to
lay a split out (where the configs live, mounting the repo root,
sharing without merging). Two rules from it you always need:

- **The browser is not on the container network.** Route browser API calls
  through a relative path the frontend dev server proxies to
  `http://api:<port>` (Vite `server.proxy`, Angular `proxy.conf.json`);
  never hardcode the alias into client code or a `VITE_API_URL` the
  browser fetches directly.
- The frontend dev server binds `0.0.0.0`, not `localhost`, or the
  forwarded port and the Phase 7 proxy can't reach it.

The tools layer follows the same split: `api-tools` and `web-tools`
services from their own Containerfiles, differing only in comfort — same
toolchain versions, lockfiles and network aliases.

## Phase 2 — Shared network topology

- Create one external network named after the project, shared by every
  devcontainer, every tools service and every infra compose stack — what
  lets a container started by one resolve one started by another, by name.
- Each devcontainer joins it with a stable alias named for what it *is*
  to callers (`api`, `web`), not the generated container name — the alias
  is the contract other files (datasource URLs, dev-server proxy targets,
  `proxy_pass`) hardcode:
  ```json
  "runArgs": ["--network", "<project-network>", "--network-alias", "<short-alias>"]
  ```
- Create the network idempotently, with whichever engine the host has,
  in **every** devcontainer of a split (whichever opens first must be able
  to create it):
  ```json
  "initializeCommand": {
    "network": "sh -c 'e=${CONTAINER_ENGINE:-$(command -v podman >/dev/null 2>&1 && echo podman || echo docker)}; $e network inspect <project-network> >/dev/null 2>&1 || $e network create <project-network>'"
  }
  ```

## Phase 3 — The tools layer: Containerfile, compose.tools.yml, recipes

Per module, following `containers.md` §2–§7:

- **One multi-stage Containerfile** (`base` → `deps` → `tools` → `build`
  → `runtime`). Research the base images (`../../references/research.md`,
  about 5 searches and 5 fetches): current tags, EOL status and Podman or
  Docker changes, with dated URLs; label `[RESEARCHED: S<n>]`, else
  `[ASSUMPTION]`, "verify before relying". Hardened images pinned by digest, sources and caches
  mounted instead of copied, a deny-by-default `.dockerignore`. If the
  module already has a production Dockerfile, extend it with the missing
  stages rather than writing a second one.
- **`compose.tools.yml`** with one `<module>-tools` service per module:
  `build.target: tools`, the repo bind-mounted, named cache volumes,
  `cpus`/`mem_limit`/`pids_limit`, the shared network, profiles for
  optional parts.
- **Task recipes** (`justfile`, or the runner the project already uses)
  with the standard names — `fmt`, `lint`, `typecheck`, `test`, `build`,
  `doctor`, `shell`, `clean` — each running in the tools service;
  `--network=none` for unit tests and builds once dependencies are
  resolved. `image` builds with the ownership label and drops the
  project's own dangling images; volumes are created labelled before
  they're mounted, named per worktree from `AI_GENT_WORKTREE`; `clean`
  removes by label and ID (`containers.md` §6, §9a).
- **One source for versions** (build args, `.tool-versions`, `.nvmrc`,
  the manifest's engines field) that the Containerfile and the
  devcontainer both read; `doctor` prints the versions from both layers
  and fails on drift.

## Phase 3b — The human layer: per-module devcontainer.json

For people: a comfortable shell, completion and IDE support, so a heavier
image is fine. It still uses the single version source, joins the same
network and can run the same recipes. Read `references/devcontainer-json.md`
when writing the file (base image order, engine access, the two cache
volumes and their ownership, formatter settings that mirror the project's
CI, autobuild/watch which Phase 6 depends on). Never hand-roll what a
feature or catalog image solves.

## Phase 4 — Env & secrets

- `.env.example` committed with safe placeholders; `.env` gitignored.
  **Check, don't assume** — verify each module's own `.gitignore` excludes
  `.env`; it has been seen missing on a module added later.
- In a split, each side gets its **own** `.env`/`.env.example` — the
  client's holds only values safe to ship to a browser.
- Wire it with `"runArgs": ["--env-file", ".env"]`, and seed `.env` from
  `.env.example` **only when absent**:
  ```json
  "initializeCommand": { "env-file": "cat .env >/dev/null 2>&1 || cp .env.example .env" }
  ```
  Changing `.env.example` later does **not** refresh an existing `.env`,
  nor one baked into a running container's environment.

## Phase 5 — External resources → `devcontainer:infra`

For anything a module talks to over the network that isn't another module
of this project — databases, caches, queues, object storage, SMTP, OAuth2
or OIDC providers, search — load `devcontainer:infra` and follow it. Pass
it the network name and module aliases decided here; it joins its compose
stack to the Phase 2 network.

## Phase 6 — Hot-reload, wired end to end

Two independent halves; verify both, since one working never implies the
other does:

- **Compiled/backend:** the stack's watch-and-restart mechanism as a
  dev-only dependency (`nodemon`, `air`, ...), fed by the IDE's autobuild
  setting from Phase 3b — a watcher with autobuild off never sees a change.
- **Interpreted/frontend:** the dev server's HMR needs no new dependency,
  but any reverse proxy in front of it (Phase 7) must pass websocket
  upgrade headers, or HMR degrades to full page reloads without an error.

## Phase 7 — Reverse proxy → `devcontainer:proxy`

Only if production routes through a path-based reverse proxy: load
`devcontainer:proxy`. Skip entirely otherwise.

## Phase 8 — Verification (do not skip — config that "looks right" often isn't)

Bring the whole thing up for real and prove each hop; `references/verification.md`
has the eleven-step checklist. In short: infra healthy and round-tripping;
every recipe passes in each tools service (and under Podman on a Podman
host), `doctor` shows no drift; limits hold (`stats --no-stream`); each
app reaches infra **by network alias**; in a split, the browser reaches the
API through the dev-server proxy, and **rebuilding one side leaves the
other running**; app-level checks (auth, validation) still behave through
the full chain; hot-reload works both directions; the proxy smoke test
passes with the real backend; `clean dry=1` lists nothing after `clean`;
and every container, volume and image made only for verification is
removed (`containers.md` §9) and the counts re-checked.

## Phase 9 — Document the decisions, not just the commands

Record in the project README (its *Development* section, per
`../../references/readme.md`, prerequisites as `[!IMPORTANT]`): the moving
parts and the order to bring them up, any version gap between the dev
simulation and prod, and the **alternatives considered and rejected, with
the reason for this project** — "why not one devcontainer for both?",
"why a separate tools layer?", "why not one docker-compose.dev.yml for
everything?". `references/verification.md` has the answers to draft from.

## Pitfalls

Symptoms seen firsthand — a changed `.env.example` that never takes
effect, a cache directory that refuses writes on a fresh volume, a
`devcontainer.json` edit with no effect, browser DNS or CORS errors, a
second devcontainer failing with "network not found", a missing workspace
dependency, passing locally and failing in CI, a leftover process holding
a port — are tabled with cause and fix in `references/verification.md`.
Check it before assuming a fresh bug.
