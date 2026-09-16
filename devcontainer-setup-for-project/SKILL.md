---
name: devcontainer-setup-for-project
description: Design and scaffold devcontainer(s) from scratch for a project that doesn't have one yet — one per module or a single shared one, depending on how the project is actually structured. Analyzes each module's exact stack (language/version, package manager, existing formatter/linter config) to pick base images/features and wire matching IDE formatting settings, plus shared network topology, caches, env wiring, and hot-reload. Also designs docker-compose simulations of external resources the app depends on — relational/NoSQL databases (Postgres, MySQL, Oracle, MongoDB...), caches (Redis, DragonflyDB), object/blob storage (Azurite, LocalStack/MinIO), queues/streaming (RabbitMQ, Kafka, Redpanda), outbound mail capture (Mailpit), search indexes, mock identity/OIDC providers, vector databases, observability backends, feature-flag services, secrets managers, service discovery, reverse proxies, and beyond — each picked via a tier-0-vendor-sandbox/tier-1-identical/tier-2-substitute/tier-3-stub decision procedure with the version-gap or emulator status documented explicitly. Use when the user asks to "set up a devcontainer", "add devcontainer support", "containerize the dev environment", "simulate <a database/queue/cache/storage/mail service> locally", or to replicate how sibling/reference projects did theirs. Complements devcontainer-workflow, which covers day-to-day operation of a devcontainer that already exists — this skill is the design/setup phase that comes before it.
---

# Designing devcontainer(s) for a project

Scope: the one-time (or per-major-change) design work of deciding how many
devcontainers a project needs, what each one contains, how they reach each
other and any infra they depend on, and how hot-reload and a reverse-proxy
simulation fit in. Once a devcontainer exists, day-to-day lifecycle/exec work
is `devcontainer-workflow`'s job, not this skill's.

Commit/branch mechanics throughout all phases follow `git-workflow` — this
skill doesn't repeat those rules.

## Phase 0 — Discovery

- Inventory the target project: module boundaries, the language/toolchain
  each module actually uses, existing Dockerfiles/compose files/CI configs,
  and whether partial devcontainer config already exists (don't discard it
  before understanding why it looks the way it does).
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
  same way Phase 0 already asks you to check sibling projects for
  convention:
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

## Phase 1 — One devcontainer, or one per module?

| Signal | One devcontainer | One per module |
|---|---|---|
| Toolchains | Same language/runtime everywhere | Different languages/runtimes (e.g. a compiled backend + a Node frontend) |
| IDE workflow | Whole repo opened as one window | Each module opened as its own window, own debugger, own extension set |
| Reference convention | N/A | Sibling projects already split this way |
| Build/deploy coupling | Built/tested together via one root tool | Modules built and shipped as independent artifacts |

Default to splitting along the same lines the project already ships
independently along — if two modules produce two separate deployable
artifacts, they get two separate devcontainers. A single shared devcontainer
is right when the modules are genuinely one toolchain (e.g. a single-language
monolith, or a frontend and its BFF built by the same tool).

## Phase 2 — Shared network topology

- Create one external Docker network named after the project, shared by
  every devcontainer and every infra compose stack — this is what lets a
  container started by one devcontainer resolve a container started by
  another (or by an infra compose) by name.
- Each devcontainer joins it and registers a stable alias other services
  will use to reach it:
  ```json
  "runArgs": ["--network", "<project-network>", "--network-alias", "<short-alias>"]
  ```
  Pick the alias for what it *is* to callers (`api`, `web`), not the
  generated container name — the alias is the contract other config files
  (datasource URLs, proxy `proxy_pass` targets) will hardcode.
- Create the network idempotently so first-time setup needs no manual step:
  ```json
  "initializeCommand": {
    "docker-network": "docker network inspect <project-network> >/dev/null 2>&1 || docker network create <project-network>"
  }
  ```

## Phase 3 — Per-module devcontainer.json

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
- **Docker access inside the container**, only if this module's own
  build/tests need to talk to Docker (Testcontainers, a Docker-based build
  step): default to `docker-outside-of-docker` over `docker-in-docker` —
  it reuses the host daemon, spun-up containers are visible as ordinary
  siblings on the host for debugging, and it avoids a duplicated image
  cache. Reach for `docker-in-docker` only when isolation from the host
  daemon is an actual requirement. (`devcontainer-workflow` has the fuller
  troubleshooting writeup for this if that skill is also loaded.)
- **Two kinds of mount, two different volumes** — don't conflate them:
  - editor/extension cache (e.g. `/root/.vscode-server/extensions` or the
    IDE's equivalent) so extensions aren't re-downloaded on every rebuild.
  - dependency/build cache (`~/.m2`, `~/.cache/...`, a package manager's
    global store, etc.) so a rebuild doesn't cold-start dependency
    resolution.
  Name each volume `<project>-<module>-<purpose>` so it survives rebuilds
  without colliding with a sibling project's volumes on the same host.
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
  this module owns those tests).
- **Enable autobuild/watch mode** where the stack supports it (an
  "autobuild on save" IDE setting, or the language's own watcher). This is
  also a prerequisite for Phase 6's hot-reload — wire it here so it isn't
  forgotten and rediscovered later as a "why doesn't hot-reload work" bug.

## Phase 4 — Env & secrets

- `.env.example` committed with safe placeholders; `.env` gitignored.
  **Check, don't assume** — verify each module's own `.gitignore` actually
  excludes `.env`; it's been seen missing entirely on a module added later
  than the others.
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

## Phase 5 — Simulating external resources (databases, caches, queues/streams, storage, mail, and beyond)

Do this for anything a module talks to over the network that isn't another
module in this same project — a database, a cache, a queue/broker, an
object-storage bucket, an outbound-mail relay, a search index, an external
identity provider. None of it should require a real account, a real cloud
resource, or a shared team-wide environment just to run the project
locally.

### 5.1 Find out what's actually needed — don't guess from a category name

Pull the real list from the project itself, the same evidence-first way
Phase 0 reads production facts: connection strings/client-SDK
initialization in the code, the dependency manifest (a Kafka/RabbitMQ
client, a cloud SDK package, a mail-sending package), existing
Testcontainers modules or CI service containers (these already name the
exact engine and often the version), existing ADRs/runbooks describing
what production runs. Build a short `(category, concrete engine, version
if known)` list before picking a single image.

### 5.2 Three-tier decision procedure, per resource

Different resources in the same project routinely land in different tiers
— apply this once per resource, not once per project:

0. **The vendor already ships an official sandbox/test mode for exactly
   this purpose** (Stripe test-mode keys and magic card numbers, Twilio
   test credentials and magic phone numbers, reCAPTCHA/hCaptcha's
   always-pass test site keys). Nothing to containerize here — the
   correct "simulation" is the real API, in its real test mode, since the
   vendor already guarantees it never touches real money/SMS/whatever and
   already keeps it in sync with prod behavior better than any local
   stand-in could. Reach for tier 1–3 only when no such vendor-provided
   mode exists.
1. **An official, free, protocol-identical image exists** (most
   open-source engines — Postgres, MySQL/MariaDB, MongoDB, Redis,
   RabbitMQ — and some vendor-shipped emulators, like Azurite for Azure
   Storage). The dev container runs the *same* software as prod, just a
   smaller instance. Zero version-gap risk; use it and move on.
2. **Prod's exact engine has no free/local image** (a managed cloud DB, a
   commercial engine's exact edition, a SaaS-only service). Pick the
   closest well-known, protocol/wire-compatible open alternative, and
   **say so explicitly** — which engine/version you're actually running,
   why, and which specific behavior gaps to watch for — in the compose
   file's comments and in Phase 9's docs. This generalizes the
   Oracle-simulated-with-a-newer-free-Oracle-image move from Phase 0's
   worked case to every category, not just databases.
3. **No self-hostable engine exists at all** (a bespoke third-party API, a
   SaaS with no open protocol equivalent). The right "simulation" here
   isn't a container pretending to be that service — it's a
   contract-tested stub/mock server (WireMock, Prism, a hand-rolled stub
   honoring the same request/response shape) that the app hits instead.
   Don't force this case into tier 1 or 2 by reaching for an unrelated
   container that happens to expose *a* port.

### 5.3 Reference catalog — concrete starting points, not a ceiling

| Category | Prod-identical option (tier 1) | Common tier-2 substitute | Notes |
|---|---|---|---|
| Relational DB | `postgres`, `mysql`/`mariadb` official images | `gvenzl/oracle-free` (Oracle); `mcr.microsoft.com/mssql/server` is itself a real free edition, not actually a gap | SQLite usually needs no container — it's a file |
| Document / key-value DB | `mongo` official image | `amazon/dynamodb-local` (DynamoDB) | |
| Cache | `redis` official image | `docker.dragonflydb.io/dragonflydb/dragonfly` — RESP-compatible, worth it when prod itself runs Dragonfly, or as a faster local swap once compatibility is confirmed against the app's actual command usage | rarely needs a persistent volume — a clean slate on restart is usually the point |
| Object/blob storage | `mcr.microsoft.com/azure-storage/azurite` (Azure — Microsoft's own emulator, tier 1) | `localstack/localstack` (broad AWS emulation: S3/SQS/SNS/DynamoDB/Lambda) or the lighter S3-only `minio/minio`, for AWS; `fsouza/fake-gcs-server` for GCS | |
| Queue / streaming | `rabbitmq` (the `-management` tag adds the web UI), `nats` | `apache/kafka` in KRaft mode (no ZooKeeper needed since Kafka 3.3+) for full Kafka; `redpandadata/redpanda` when only the Kafka wire protocol matters and a lighter single-binary footprint is worth it | confirm KRaft env vars (`KAFKA_PROCESS_ROLES=broker,controller`, a node/cluster ID) — older Kafka compose examples still show the legacy two-container ZooKeeper setup |
| Outbound mail | `axllent/mailpit` | — | always a capture sink regardless of prod's real provider (SES, SendGrid, an SMTP relay) — dev must never send real mail. MailHog is the older, now-unmaintained equivalent; migrate to Mailpit if a reference project still uses it |
| Search index | `elasticsearch`/`opensearchproject/opensearch` official images | — | needs `discovery.type=single-node` (or the OpenSearch equivalent) for a one-node dev cluster |
| External identity/OIDC provider | — | `ghcr.io/navikt/mock-oauth2-server` | issues real signed JWTs from a local issuer/JWKS — this is what unblocks a "needs a real JWT" e2e gap; each URL path segment can serve a distinct issuer config, so multiple audiences/issuers come from one instance |
| Vector database | `pgvector/pgvector` (a Postgres image with the extension preinstalled, when the project already runs Postgres and only needs vector search added) | `qdrant/qdrant`, Weaviate | pick the Postgres-extension route first if the project is already relational — one fewer moving part than a dedicated vector store |
| Observability backend (traces/metrics/logs) | `jaegertracing/all-in-one`, `openzipkin/zipkin` | a local Grafana + Prometheus + Loki stack when metrics/logs matter too, not just traces | only worth it once the app actually emits OpenTelemetry/traces — otherwise there's nothing to look at |
| Feature flags | `unleashorg/unleash-server`, GrowthBook self-hosted | — | avoids needing a real LaunchDarkly/GrowthBook account for local dev |
| Secrets/config management | `hashicorp/vault` in `server -dev` mode (in-memory, fixed dev root token — this is Vault's own documented dev-mode flag, not a workaround) | `localstack` (AWS Secrets Manager) | only needed if the app fetches secrets at runtime rather than via env vars |
| Service discovery | `hashicorp/consul` in `-dev` mode | — | only needed if the app resolves other services through it instead of DNS/network alias |
| Third-party or internal API with no self-hosted engine (tier 3 above) | — | `wiremock/wiremock` (programmable stubbing), `stoplight/prism` (mocks straight from an OpenAPI spec) | naming a concrete tool here beats a "hand-rolled stub" every time one of these fits the shape of the API being doubled |

Apply Phase 5.2's procedure to whatever the project's own stack actually
needs, including categories not listed here — this table covers what most
projects hit, not every case.

**Prefer a vendor's bundled multi-service emulator suite over assembling
one container per category**, when the project is deeply tied to one
cloud/vendor — it keeps auth and config consistent across services
instead of wiring N containers with N separate credential shapes:
AWS-heavy → LocalStack alone already covers S3/SQS/SNS/DynamoDB/Lambda/
Secrets Manager; Firebase-heavy → the Firebase Local Emulator Suite covers
Firestore, Realtime Database, Auth, Functions, Pub/Sub and Storage
together; Azure has no single suite that broad, so its services stay
per-emulator (Azurite for storage, a separate Cosmos DB emulator, a
separate Service Bus emulator) — check whether one exists for the
specific Azure service before assuming Azurite alone covers it.

### 5.4 Bootstrapping each resource — pick the right pattern

- **Schema/migration tools** (Flyway, Liquibase, Prisma Migrate, Alembic,
  etc.) — for relational/document DBs, run the project's own migration
  tool against the container once it's healthy, same as Phase 0's worked
  Oracle case.
- **One-shot resource seeding** (create a bucket, a topic, an
  exchange/queue, an index) — model it as its **own** compose service that
  runs once and exits, gated on the target being healthy, not just
  started:
  ```yaml
  services:
    minio:
      image: minio/minio
      command: server /data
      healthcheck:
        test: ["CMD", "mc", "ready", "local"]
        interval: 5s
        retries: 10
    minio-seed:
      image: minio/mc
      depends_on:
        minio:
          condition: service_healthy
      entrypoint: >
        sh -c "mc alias set local http://minio:9000 <key> <secret> &&
               mc mb --ignore-existing local/<bucket-name>"
      restart: "no"
  ```
  `condition: service_healthy` is what actually waits for the resource to
  accept connections — a bare `depends_on:` by name only orders container
  *start*, and a seed step racing a not-yet-ready service fails
  intermittently in a way that looks like a flaky test rather than a
  compose ordering bug.
- **The app already auto-declares its own topics/queues/exchanges on
  startup** — common for message-queue clients. Check for this before
  adding a redundant seed step; a manual seed racing the app's own
  auto-declare is a needless extra failure mode.
- **Mail/cache/identity-mock** — typically need zero bootstrap; the app
  just needs the right connection env var pointed at them.

### 5.5 Compose mechanics

- Own `docker-compose.dev.yml`/`docker-compose.infra.yml`, `name:
  <project>` at the top (Compose otherwise prefixes every resource with
  the invoking directory's name, silently diverging from the shared
  network/volume names devcontainers expect), joined to the Phase 2
  external network, one stable service name per resource for other config
  to reference.
- **Give every long-running infra service a `healthcheck`** — beyond
  gating the seed pattern above, it's also what Phase 8's verification and
  any app-boot wait logic should key off, instead of a hand-rolled
  `sleep`.
- Persist state in a named volume only where dev actually benefits from
  surviving a restart (a DB's data, a broker's queues if durability itself
  is under test) — deliberately skip a volume where a clean slate on every
  restart is the more useful default (a cache, most mail-capture and
  mock-identity setups). This is a **different lifecycle** than an
  ephemeral test container (Testcontainers, CI service containers), which
  should stay ephemeral — don't merge the two.
- **Reuse an image the project already trusts, instead of introducing a
  new variant.** If the test suite already pulls a specific image for this
  same dependency (via Testcontainers or similar), reuse that exact image
  here — it's already validated against the project's schema/migrations,
  and a second image variant is one more thing to keep in sync for no
  benefit.
- Expose convenience host ports for native GUI clients (a DB client,
  RabbitMQ's/Mailpit's own bundled web UI) — the app itself still talks
  over the network alias; say so in a comment.
- **Optional companion UI containers** for resources that don't bundle one
  (Kafka/Redpanda has none built in — add `redpandadata/console` or
  `kafka-ui` alongside it only if the team will actually use it; skip it
  otherwise, it's one more container to keep healthy for a nice-to-have).

### 5.6 Emulator credentials are not secrets

Several emulators ship a fixed, publicly documented dev credential that
means nothing outside that emulator (Azurite's well-known
`devstoreaccount1` account/key pair, most mock-OIDC servers' default
signing setup). Commit these as-is in `.env.example` — they guard nothing
real, and inventing a "safer-looking" placeholder for them adds friction
for no security benefit. This is distinct from Phase 4's actual rule,
which is about real production credentials.

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

## Phase 7 — Simulating the reverse proxy (only if production has one)

Skip this phase entirely if production doesn't route through a path-based
reverse proxy — adding a proxy layer that doesn't exist in prod is a new
thing to debug with no payoff.

- Read the **real** production path prefixes from the project's own
  config (its `.env.example`, existing proxy/ingress config) — never
  invent a generic shortcut path. The entire value of this simulation is
  catching basename/CORS/cookie-path bugs that only appear when the path
  structure matches prod exactly; a simplified path defeats that purpose.
  A sibling project using a different, flatter convention doesn't mean
  this one should — match *this* project's own prod, not a sibling's.
- One `location`-equivalent block per upstream: the API path(s) proxy to
  the backend module's network alias; everything else proxies to the
  frontend dev server, with the HTTP version bump and `Upgrade`/
  `Connection` headers HMR's websocket needs to survive the hop.
- Rewrite cookie paths if the backend sets cookies scoped to a path that
  changes once fronted by the proxy.
- Ship this as its own compose file + config, run opt-in via a separate
  script — most edit-and-reload work doesn't need the proxy in the loop;
  only the moments actually checking cross-origin/path assumptions do.

## Phase 8 — Verification (do not skip — config that "looks right" often isn't)

Bring the whole thing up for real and prove each hop, not just that
containers boot:

1. Every infra container starts and reports healthy.
2. Migrations/schema setup run successfully against any DB. For **every
   other** simulated resource (cache, queue, storage, mail, identity
   mock), do one real round trip specific to its category, not just a
   green healthcheck: set+get a cache key, publish+consume one message,
   write+read one object, send one mail and see it land in the catcher's
   UI/API, request one token from the identity mock and have the app
   actually accept it. A healthcheck only proves the process started, not
   that the app can use it.
3. Each module's devcontainer starts and its app connects to infra **by
   network alias**, not just by an exposed host port — that's what
   actually proves the shared-network wiring, not just that infra works
   standalone.
4. If three or more containers are in play, confirm resolution from a
   *third* one (neither infra nor the module itself) using the alias.
5. Whatever the app enforces (auth, validation) still behaves correctly
   through the full chain — a dev shortcut that only "works" because a
   check got accidentally bypassed is worse than no dev environment.
6. Hot-reload both directions: edit backend source, confirm a restart in
   the logs without you restarting the process; edit frontend source,
   confirm an HMR log line, not a full page reload.
7. If a reverse-proxy simulation exists, repeat the smoke test through it
   with the **real** backend behind it, not a stub — the proxy config is
   exactly the part a stub can't validate.
8. Clean up every container/volume created purely for this verification
   (`docker ps -a`, `docker volume ls`) — an orphaned volume from a
   naming-prefix mistake (see Pitfalls) is easy to leave behind.

## Phase 9 — Document the decisions, not just the commands

Future readers — including future-you — need the *why*, not only the
compose file. In the project's README or a docs file, record:

- The setup's moving parts and the order to bring them up.
- Any version gap between the dev simulation and prod (Phase 5), stated
  plainly rather than left implicit.
- **Alternatives considered and rejected, with reasoning.** This is the
  actual deliverable the first time someone asks "why not just do X
  instead" — cheap to write down at design time, expensive to
  reconstruct later. The alternative that recurs most: "why not one
  single docker-compose.dev.yml for everything, app code included."
  Write the real answer for *this* project (typically: it breaks IDE
  debugging since the app then runs in a container the IDE never
  attached to, it duplicates toolchain resolution the devcontainer
  features already handle, and it diverges from the established
  reference convention) — and note where that alternative genuinely
  *would* fit instead (commonly: a CI job that only runs the stack and
  never edits it).

## Pitfalls (observed firsthand — check these before assuming a fresh bug)

| Symptom | Cause | Fix |
|---|---|---|
| A changed default in `.env.example` never takes effect | `initializeCommand`'s seeding only runs when `.env` is **absent** — an existing `.env` is never refreshed | `cp .env.example .env` by hand, then recreate the container (`devcontainer up --recreate` or equivalent) — a plain restart keeps the old `--env-file` values already baked into the running container |
| Compose resources named `<invoking-dir>_<resource>` instead of the intended project name | Compose prefixes resource names with the invoking directory when no explicit name is set | add a top-level `name: <project>` to the compose file; clean up anything already created under the stale prefix |
| "Permission denied" / can't write to a cache directory, only on a fresh volume | Newly created named volumes are root-owned; the container's user isn't root | `chown -R <remoteUser>:<remoteUser> <mount-target>` once via `postCreateCommand`, not a manual one-off |
| A `devcontainer.json` edit (features/mounts/image) has no visible effect | Those only apply at container (re)creation | recreate the container explicitly; never fake it with a plain restart |
| A seed/init step (Phase 5.4) intermittently fails right after `docker compose up` | `depends_on` without a `condition` only waits for the target container to **start**, not to be ready for connections | add a `healthcheck` to the target service and gate the seed step on `condition: service_healthy` |
| The app still reaches the real cloud endpoint even with the emulator container running | Cloud SDKs default to the real service; the emulator needs an explicit endpoint override (`AWS_ENDPOINT_URL`/path-style flag, an Azure emulator connection string) that's easy to leave unset | set the override explicitly in the dev `.env`/`.env.example`, and confirm it with the per-category round trip from Phase 8 — not just that the emulator container is up |
| A background process started only to verify something is still holding a port later | Long-lived dev/server processes outlive the tool call that started them | track what you started as you go and stop it before calling the task done (see `devcontainer-workflow`) |
