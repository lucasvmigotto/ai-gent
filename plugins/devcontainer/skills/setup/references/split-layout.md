# Splitting and laying out devcontainers (Phase 1.2–1.4)

Part of `devcontainer:setup` (`../SKILL.md`): read it when the project is not a plain API + client pair, or when laying out the configs of a split.

## 1.2 Everything else — signals table

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

## 1.3 Laying out a split

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

## 1.4 Two layers per module

Whatever Phase 1 decided for devcontainers applies to the tools layer too:
an API and a client get separate `api-tools` and `web-tools` services
built from their own Containerfiles. The layers differ only in comfort —
same toolchain versions, same lockfiles, same network aliases.
