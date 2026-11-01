# Discovery in detail (Phase 0)

Part of `devcontainer:setup` (`../SKILL.md`): read it when the project has sibling
references, an unusual stack, or you need the catalog and evidence rules.

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
