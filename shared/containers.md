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
