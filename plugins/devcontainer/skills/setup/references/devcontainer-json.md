# The human layer in detail (Phase 3b)

Part of `devcontainer:setup` (`../SKILL.md`): read it when writing a module's `devcontainer.json` — base image order, engine access, volumes and ownership, formatter settings, IDE customizations, autobuild.

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
