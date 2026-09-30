# Container rules — container-first development

Shared by the `devcontainer`, `devsecops` and `project` plugins (symlinked
into each plugin's references directory). Every stage that builds, runs or ships a
container follows these rules.

Development is treated as an experiment: the same inputs, run through the
same pinned container, give the same result on any machine and in CI.
Exceptions exist (network calls, clocks, real concurrency); they are named,
not accidental.

## 1. Engine — Podman first, Docker as fallback

Detect once and use the variable everywhere; never hardcode `docker` in
scripts, recipes or docs:

```bash
CONTAINER_ENGINE="${CONTAINER_ENGINE:-$(command -v podman >/dev/null 2>&1 && echo podman || echo docker)}"
"$CONTAINER_ENGINE" compose version   # podman compose delegates to a compose provider
```

Where Podman differs, handle it in the config rather than per machine:

| Topic | Docker | Podman (rootless) | Portable choice |
|---|---|---|---|
| Host from a container | `host.docker.internal` | `host.containers.internal` | `extra_hosts: ["host.docker.internal:host-gateway"]` in compose |
| Bind-mount ownership | container UID writes as host UID only if they match | UIDs are remapped | run as the image's non-root user; `userns_mode: keep-id` under Podman when files must be written back |
| SELinux hosts (Fedora, RHEL) | usually off | enforced | `:Z` (private) or `:z` (shared) on bind mounts |
| Ports below 1024 | allowed | refused rootless | publish ≥ 1024 on the host |
| Docker socket (Testcontainers, docker-outside-of-docker) | `/var/run/docker.sock` | `systemctl --user enable --now podman.socket`, then `DOCKER_HOST=unix://$XDG_RUNTIME_DIR/podman/podman.sock` | set `DOCKER_HOST` from the engine; `TESTCONTAINERS_RYUK_DISABLED=true` only if Ryuk can't start |
| Resource limits | cgroup v2 or v1 | need cgroup v2 (`podman info --format '{{.Host.CgroupsVersion}}'`) | check once; say so if limits are ignored |
| devcontainer CLI / VS Code | default | `--docker-path podman` / setting `dev.containers.dockerPath` | document it in the project README |

## 2. Three layers per module

```
Containerfile (one per module, multi-stage)
  base      pinned toolchain, thin (hardened -dev image when available)
  deps      dependencies resolved with cache mounts
  tools     base + deps + linters, test runners — what agents and CI run
  build     produces the artifact
  runtime   hardened runtime image + the artifact only
.devcontainer/<module>/   the human layer: a comfortable image, IDE settings
```

- **`tools` is the reference environment.** Agents and CI run every task
  (format, lint, typecheck, test, build, migrate) in it, through the
  project's task recipes (§6). If a result differs between the human
  devcontainer and `tools`, `tools` is right and the devcontainer has
  drifted.
- **The human devcontainer** is for people: a full shell, completion, IDE
  extensions. It may use a heavier image (the user's `devenv` family, or
  `mcr.microsoft.com/devcontainers/*`) but takes its toolchain versions from
  the same single source as `tools` (build args, `.tool-versions`,
  `.nvmrc`, `.java-version`, the manifest's engines field), and a
  `doctor` recipe compares the two.
- **`runtime`** has no shell, package manager or build tools. Debug it with
  the `tools` or `-dev` image, not by adding a shell to production.

## 3. Base images — hardened first, pinned always

Preference, per stage:

1. **Docker Hardened Images** (`dhi.io/<image>:<tag>`): the runtime image
   for `runtime`, the `-dev` variant for `base`/`deps`/`tools`/`build`.
   Check the catalog for the exact image and version; not every runtime or
   version exists.
2. The official distroless or `-slim` image for that runtime.
3. Alpine only when musl is verified to work for every native dependency.

A project's or user's own curated image family ranks alongside these for the
**human** devcontainer — a non-root user, a configured shell and
language-aware cache volumes already baked in is worth preferring over
assembling the same from a generic base. For the `tools` image, weigh §2's
"thin" rule: the comfort image is usually the wrong choice there. See §3b.

- **Pin by digest** with the tag as a comment
  (`FROM dhi.io/node:22-dev@sha256:… # 22-dev`) and let Renovate or
  Dependabot move the digests.
- **Authentication.** Pulling from `dhi.io` needs a Docker Hub login. CI
  always has a `DOCKER_HUB_PAT` secret; the username comes from the
  `DOCKER_HUB_USERNAME` variable, defaulting to the repository owner.
  Locally the developer logs in once (`"$CONTAINER_ENGINE" login dhi.io`).
  Never write the token into a file, a Containerfile, an image or a log:
  ```bash
  printf '%s' "$DOCKER_HUB_PAT" | "$CONTAINER_ENGINE" login dhi.io -u "${DOCKER_HUB_USERNAME:-$OWNER}" --password-stdin
  ```

## 3b. Composing a base image plus features

A devcontainer can also be *composed*: a base image carrying the common ground,
plus one devcontainer feature per toolchain. This is an addition to §3's
preference order, not a replacement for it — reach for it only under rule 1.

1. **Compose when no single image carries the toolchains.** Reach for
   base + features when a repo needs two unrelated runtimes, no published
   language image carries both, and a curated base (or a slim official one)
   already provides the common ground. When a language image *does* carry the
   needed toolchain, prefer it — one `FROM`, nothing to assemble.

2. **A feature supplies only what the base lacks.** Adding a feature for a
   toolchain the base already ships is redundant at best and a version
   conflict at worst. Check what the base actually contains before listing
   features.
   *Observed:* a language feature defaulted the editor formatter to a tool the
   project did not use, and its "install common tools" option pulled a linter,
   a formatter and several analysers that the project's own (already listed)
   tools covered — all of it invisible until read from the feature metadata.

3. **A package manager may replace the runtime feature.** Before adding a
   feature that installs a language runtime, check whether the project's
   package manager provisions it already.
   *Observed:* a Python project on `uv` dropped its Python feature entirely —
   `uv sync` fetched CPython on a base with **no Python at all**, honouring the
   `requires-python` in the manifest. Two features (manager + second runtime)
   instead of three, and the interpreter's version came from the project's own
   manifest rather than a second pin.

4. **A composed single devcontainer is only for a single-scoped
   architecture.** Compose one when the project *is* one scope: a single
   artifact family, or several that never relate technically.

   **Split when the application guards self-contained contexts that
   communicate with each other but do not relate technically.** Any one
   signal is enough:
   - separate deploy targets (a `Dockerfile` per module),
   - a client calling the service over a protocol,
   - a shared runtime contract (an OpenAPI document, a published schema),
   - separate debuggers, env files or restart lifecycles.

   That the modules share a language, a repository or a toolchain is **not** a
   reason to merge them — those are the cheapest things to duplicate and the
   least of what a devcontainer configures.

   *Split — two modules, one repo:* an `api/` and an `app/` with a
   `Dockerfile` each, the app calling the API through its own client module,
   and a `contracts/openapi.yaml` between them. Two devcontainers and two
   `tools` services.

   *Single — two artifacts, one scope:* a CLI and a static site in one
   repository, where the site never invokes the CLI and they share no
   contract. One devcontainer on a curated base plus a feature per toolchain;
   one `tools` image, because the checks are independent rather than
   conflicting.

5. **Persist what a feature installs, and own it once.** A feature-installed
   runtime lands in the **user's home**, so a rebuild without a volume
   re-downloads it (tens of megabytes for an interpreter). Volume the
   package manager's cache *and* its data directory. A fresh named volume is
   root-owned, so a non-root user cannot write into it — `chown` it in
   `postCreateCommand`, once, rather than repeating a manual fix every
   rebuild.

6. **"Offline" is a service, not a flag.** Where a compose `run` has no
   `--network` option, express the network-free guarantee as a second service
   that extends the tools one with `network_mode: none`, and have the recipes
   target it. Keeps the recipes portable across compose providers.
   *Observed:* `run --network=none` fails outright where compose delegates to
   a provider whose `run` lacks the flag.
   Note that some build steps cannot be network-free even with a warm cache —
   a packaging command that fetches its own build backend, for instance. Keep
   those on the networked service, and say why in the recipe.

7. **A dependency environment must live outside the bind mount.** Recipes
   mount the repository over the image's working directory, which **hides**
   whatever the image built there: a virtual environment at
   `<workdir>/.venv` disappears at run time and every recipe fails as if the
   dependencies were never installed. Build it elsewhere and point the tool
   at it (`UV_PROJECT_ENVIRONMENT=/opt/venv` and the equivalent for other
   ecosystems), so it survives the mount.
   *Observed:* an image whose `/src/.venv` was correct at build time ran
   `python: not found` the moment the repo was mounted.

8. **One file states the versions.** Feature `version` options, build args and
   CI all read the same source (`.tool-versions` or the manifest's own field);
   never `latest`. A `doctor` recipe prints what each layer actually has and
   fails on drift — composed containers have more places for a version to hide
   than a single `FROM` does.

> **Observed — a canvas test runtime needs a font backend.** A graphical suite
> built on node-canvas or `@napi-rs/canvas` draws **nothing** without
> fontconfig and fonts available: measured 0 ink pixels in a bare container
> against ~2000 on a host, so every glyph vanished and the suite compared
> blank text areas — passing while proving nothing about text. Install
> `fontconfig`, register the product's **own** font, and assert at build time
> that it resolves (`fc-list | grep -qi <family>`). Register the bundled face
> even where the platform has one: two environments resolving *different*
> fallback faces makes any pixel comparison meaningless.

## 4. Containerfiles

- First line `# syntax=docker/dockerfile:1` (BuildKit; Podman's buildah
  supports the same `RUN --mount` forms).
- **Always a `.dockerignore`**, written as deny-by-default: `*`, then `!`
  the paths the build needs. `.git`, `.env*` (except `.env.example`),
  `node_modules`, build outputs and IDE folders never reach the context.
- **Mount, don't copy, in build stages.** Source and manifests come in
  through `RUN --mount=type=bind,…`; package caches through
  `RUN --mount=type=cache,target=<cache dir>,sharing=locked`; private
  registry tokens through `RUN --mount=type=secret,id=…`. Bind mounts are
  read-only: write outputs to a path outside the mount (`/out`).
- **The final stage copies only the artifact**: `COPY --from=build
  --chown=<uid>:<gid> /out/ /app/` — the one place `COPY` is expected.
- Non-root `USER` with a fixed numeric UID; `HEALTHCHECK` where the
  runtime supports one (or a compose healthcheck when the image has no
  shell); OCI labels (`org.opencontainers.image.source`, `.revision`,
  `.version`).
- No `apt-get upgrade`, no `latest`, no unpinned `curl | sh`; package
  versions pinned where the package manager allows.

```dockerfile
# syntax=docker/dockerfile:1
FROM dhi.io/golang:<version>-dev@sha256:<digest> AS base   # <version>-dev
WORKDIR /src

FROM base AS deps
RUN --mount=type=bind,source=go.mod,target=go.mod \
    --mount=type=bind,source=go.sum,target=go.sum \
    --mount=type=cache,target=/go/pkg/mod,sharing=locked \
    go mod download

FROM deps AS tools
# linters and test tools pinned in go.mod (tool directives), not installed globally

FROM deps AS build
RUN --mount=type=bind,target=. \
    --mount=type=cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=0 go build -trimpath -ldflags=-buildid= -o /out/app ./cmd/app

FROM dhi.io/<static-runtime>:<tag>@sha256:<digest> AS runtime   # <tag>
COPY --from=build --chown=65532:65532 /out/app /app
USER 65532
ENTRYPOINT ["/app"]
```

The example shows the shape, not a template to paste: adapt paths, the
package manager and the build command to the module.

## 5. Determinism

- **Inputs pinned:** image digests, lockfiles with frozen installs
  (`npm ci`, `uv sync --frozen`, `mvn -o` after a resolve step,
  `go mod download` + `-mod=readonly`), tool versions from one file.
- **Environment fixed:** `TZ=UTC`, `LANG=C.UTF-8`,
  `SOURCE_DATE_EPOCH=$(git log -1 --format=%ct)` for builds, seeded
  randomness where the test runner allows it (and the seed printed).
- **Isolation per run:** `run --rm`, fresh tmpfs for scratch, no state
  carried between runs except declared caches; unit tests and builds with
  `--network=none` once dependencies are resolved. Integration tests get
  the infra network (`devcontainer:infra`) and nothing else.
- **Record the experiment:** the recipe prints the image digest and tool
  versions it ran with, so a result can be reproduced or compared.

## 6. Task recipes — one entry point for people, agents and CI

A `justfile` (or the project's existing Makefile/task runner) with the
same recipe names everywhere — `fmt`, `lint`, `typecheck`, `test`,
`build`, `doctor`, `shell` — each running in the module's `tools` image:

```make
engine := env_var_or_default("CONTAINER_ENGINE", `command -v podman >/dev/null 2>&1 && echo podman || echo docker`)

test module="api":
    {{engine}} compose -f compose.tools.yml run --rm --network=none {{module}}-tools <test command>
```

- `compose.tools.yml` defines one `<module>-tools` service per module:
  `build: {target: tools}`, the repo bind-mounted read-write at the
  working directory, cache volumes named `<project>-<module>-<purpose>`,
  the §7 limits.
- CI calls the same recipes (`devsecops:pipeline`), so a green run
  locally means the same thing as a green run in CI.

## 7. Resource limits — never stall the host

- Every long-running service (tools, infra, the human devcontainer's
  compose services) sets `cpus`, `mem_limit` and `pids_limit`; build and
  test runs pass `--cpus`/`--memory`. Size the whole stack to at most
  about half of the host's CPUs and memory, and let the user raise it.
- **Compose profiles** so only what the task needs starts
  (`--profile db`, `--profile mail`); nothing heavy starts by default.
- Healthchecks with a `start_period`, `restart: "no"` in development, and
  explicit parallelism for builds that fan out (`-j`, `--max-workers`).
- On WSL2, the VM's own ceiling lives in `%UserProfile%\.wslconfig`
  (`memory=`, `processors=`); mention it when limits seem ignored.

## 8. Registries — pushing images

- **GHCR:** `ghcr.io/<owner>/<repo>[-<module>]`, owner lowercased (GHCR
  rejects uppercase). Log in with the workflow's token as the repository
  owner, with `packages: write` on that job only:
  ```bash
  printf '%s' "$GITHUB_TOKEN" | "$CONTAINER_ENGINE" login ghcr.io -u "$GITHUB_REPOSITORY_OWNER" --password-stdin
  ```
- **Docker Hub:** `docker.io/<namespace>/<image>`; the namespace and login
  default to the repository owner, overridable with `DOCKER_HUB_USERNAME`,
  and the token is `DOCKER_HUB_PAT`.
- Tag with the version and the commit SHA; deploy and promote by digest
  (`devsecops:pipeline`), sign and attest per `devsecops:supply-chain`.

## 9. Clean up what you create — containers *and* images

A container or image you start to answer a question ("does this feature
install?", "does the stack come up?", "did the build pass?") is **scratch**.
It is removed when the question is answered, the same way you'd delete a
scratch file. The user's own dev environment is **standing** and is never
yours to remove. Left alone, a handful of probes will pin gigabytes and
RAM indefinitely, because a *stopped* container still holds its layers.

This applies to every stage that starts a container — `devcontainer:*`,
`backend`/`frontend:build`, `qa:e2e`/`qa:load`, `db:connect`/`db:investigate`,
`devsecops:*` — not just the container tooling.

- **Name scratch things** with a prefix you own
  (`--name scratch-uv-probe`). Devcontainer builds leave
  `localhost/vsc-<workspace>-<hash>-features*` images that are impossible to
  attribute later; a deliberate name is what makes them removable.
- **Snapshot, act, then verify.** Before starting: `$CONTAINER_ENGINE ps -a`,
  `images`, `volume ls`. Remove only what the diff shows you added —
  `rm -f` the containers, then `rmi` their images (a *running* container
  pins its image, so stop before removing), then `volume rm` anything you
  added. Re-run the same three commands and **confirm the count actually
  dropped**; an exit code of 0 is not evidence. Don't wrap the removal in
  `>/dev/null 2>&1` either — a loop that discards the engine's complaint
  ("image is in use", "no such image") reports success while removing
  nothing, and the wasted space looks identical to a slow disk.
- **Target precisely; never prune broadly.** `--filter dangling`,
  `image prune` and `system prune -a` will happily delete images another
  project is mid-build on, and volumes that look unused are frequently not.
  Remove by **exact ID or your own prefix**. If a sweep is genuinely the
  right call, list exactly what it would take and get agreement first.
- **Ask before removing anything you did not create** — including something
  that looks orphaned. On a shared host, "unused" is indistinguishable from
  "someone's in-flight work". Offer the list; let the user decide.
- **If cleanup didn't take, suspect the context before retrying.** Rootless
  vs. root store, or a `DOCKER_HOST`/`system connection` pointing elsewhere,
  produces a removal that reports success against a *different* store and
  leaves the real one untouched. Check
  `"$CONTAINER_ENGINE" system connection ls` and `info --format
  '{{.Store.GraphRoot}}'` before concluding the engine is broken.
- Report what you removed and what you deliberately left, so the user can
  see the workspace is clean without re-deriving it.
