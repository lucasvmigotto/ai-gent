---
name: infra
description: Simulate the external services an app needs with docker-compose — databases, caches, object storage, queues, SMTP capture, OAuth2/OIDC/SAML/LDAP providers, search, observability, API stubs — picking the closest local stand-in and documenting the gap. Use for "simulate <service> locally", "mock the identity provider", "catch outgoing emails".
---

# Simulating external resources for local development

Scope: anything a module talks to over the network that isn't another
module in this same project — a database, a cache, a queue/broker, an
object-storage bucket, an SMTP relay or mail API, an identity provider, a
search index, a third-party API. None of it should require a real account,
a real cloud resource, or a shared team-wide environment just to run the
project locally.

Relation to the sibling subskills: `devcontainer:setup` decides the
project's shared Docker network and each module's alias — reuse those. If
the project has a devcontainer but no shared network yet, create one the
same way (`docker network inspect <net> || docker network create <net>`)
rather than inventing a second convention. `devcontainer:workflow` covers
running things inside the devcontainer afterwards.

## 1. Find out what's actually needed — don't guess from a category name

Pull the real list from the project itself: connection strings/client-SDK
initialization in the code, the dependency manifest (a Kafka/RabbitMQ
client, a cloud SDK package, a mail-sending package, an OAuth2/OIDC client
or resource-server library), existing Testcontainers modules or CI service
containers (these already name the exact engine and often the version),
existing ADRs/runbooks describing what production runs, and the project's
own `.env.example`/deployment manifests for real env var names and prod
versions. Build a short `(category, concrete engine, version if known)`
list before picking a single image.

For identity and mail specifically, also record **how** the app uses them
(§4.1 of `references/identity.md` and §5.1 of `references/mail.md`) — the flow or transport decides the tool more than the
vendor name does.

## 2. Tier decision procedure, per resource

Different resources in the same project routinely land in different tiers
— apply this once per resource, not once per project:

0. **The vendor already ships an official sandbox/test mode for exactly
   this purpose** (Stripe test-mode keys and magic card numbers, Twilio
   test credentials and magic phone numbers, reCAPTCHA/hCaptcha's
   always-pass test site keys, SendGrid's `sandbox_mode`, Postmark's
   `POSTMARK_API_TEST` token). Nothing to containerize here — the correct
   "simulation" is the real API in its real test mode, since the vendor
   already guarantees it never touches real money/SMS/mail and keeps it in
   sync with prod behavior better than any local stand-in could. Reach for
   tier 1–3 only when no such vendor-provided mode exists, or when dev must
   work offline.
1. **An official, free, protocol-identical image exists** (most
   open-source engines — Postgres, MySQL/MariaDB, MongoDB, Redis,
   RabbitMQ, Keycloak when prod runs Keycloak — and some vendor-shipped
   emulators, like Azurite for Azure Storage). The dev container runs the
   *same* software as prod, just a smaller instance. Zero version-gap
   risk; use it and move on.
2. **Prod's exact engine has no free/local image** (a managed cloud DB, a
   commercial engine's exact edition, a SaaS identity provider like Entra
   ID/Okta/Auth0). Pick the closest well-known, protocol-compatible open
   alternative, and **say so explicitly** — which engine/version you're
   actually running, why, and which specific behavior gaps to watch for —
   in the compose file's comments and in the project docs (§10). Example:
   Oracle simulated with a newer free `gvenzl/oracle-free` image; Entra ID
   simulated with mock-oauth2-server issuing Entra-shaped claims.
3. **No self-hostable engine exists at all** (a bespoke third-party API, a
   SaaS with no open protocol equivalent). The right "simulation" isn't a
   container pretending to be that service — it's a contract-tested
   stub/mock server (WireMock, Prism, a hand-rolled stub honoring the same
   request/response shape) that the app hits instead. Don't force this
   case into tier 1 or 2 by reaching for an unrelated container that
   happens to expose *a* port.

## 3. Reference catalog — concrete starting points, not a ceiling

| Category | Prod-identical option (tier 1) | Common tier-2 substitute | Notes |
|---|---|---|---|
| Relational DB | `postgres`, `mysql`/`mariadb` official images | `gvenzl/oracle-free` (Oracle); `mcr.microsoft.com/mssql/server` is itself a real free edition, not actually a gap | SQLite usually needs no container — it's a file |
| Document / key-value DB | `mongo` official image | `amazon/dynamodb-local` (DynamoDB) | |
| Cache | `redis` official image | `docker.dragonflydb.io/dragonflydb/dragonfly` — RESP-compatible, worth it when prod itself runs Dragonfly, or as a faster local swap once compatibility is confirmed against the app's actual command usage | rarely needs a persistent volume — a clean slate on restart is usually the point |
| Object/blob storage | `mcr.microsoft.com/azure-storage/azurite` (Azure — Microsoft's own emulator, tier 1) | `localstack/localstack` (broad AWS emulation: S3/SQS/SNS/DynamoDB/Lambda) or the lighter S3-only `minio/minio`, for AWS; `fsouza/fake-gcs-server` for GCS | |
| Queue / streaming | `rabbitmq` (the `-management` tag adds the web UI), `nats` | `apache/kafka` in KRaft mode (no ZooKeeper needed since Kafka 3.3+) for full Kafka; `redpandadata/redpanda` when only the Kafka wire protocol matters and a lighter single-binary footprint is worth it | confirm KRaft env vars (`KAFKA_PROCESS_ROLES=broker,controller`, a node/cluster ID) — older Kafka compose examples still show the legacy two-container ZooKeeper setup |
| Outbound mail (SMTP) | — | `axllent/mailpit` | always a capture sink regardless of prod's real provider — dev must never send real mail. Details in §5 of `references/mail.md` |
| Inbound mailbox (IMAP/POP3) | — | `greenmail/standalone` | for apps that *read* mail; §5.4 of `references/mail.md` |
| Transactional mail HTTP API (SES, SendGrid, Postmark, Mailgun) | vendor sandbox (tier 0) | LocalStack SES; WireMock stub | §5.3 of `references/mail.md` |
| OAuth2 / OIDC identity provider | `quay.io/keycloak/keycloak` when prod runs Keycloak | `ghcr.io/navikt/mock-oauth2-server`, Keycloak, `ghcr.io/dexidp/dex` | details and the flow-based choice in §4 of `references/identity.md` |
| SAML IdP | Keycloak (as a SAML IdP) when prod is Keycloak | Keycloak | §4.6 of `references/identity.md` |
| LDAP / Active Directory | `osixia`/Bitnami OpenLDAP-style images (check current maintenance) | `lldap/lldap` for simple user/group lookups | §4.6 of `references/identity.md` |
| Search index | `elasticsearch`/`opensearchproject/opensearch` official images | — | needs `discovery.type=single-node` (or the OpenSearch equivalent) for a one-node dev cluster |
| Vector database | `pgvector/pgvector` (Postgres with the extension preinstalled, when the project already runs Postgres and only needs vector search added) | `qdrant/qdrant`, Weaviate | pick the Postgres-extension route first if the project is already relational — one fewer moving part |
| Observability backend (traces/metrics/logs) | `jaegertracing/all-in-one`, `openzipkin/zipkin` | a local Grafana + Prometheus + Loki stack when metrics/logs matter too | only worth it once the app actually emits OpenTelemetry/traces |
| Feature flags | `unleashorg/unleash-server`, GrowthBook self-hosted | — | avoids needing a real LaunchDarkly/GrowthBook account for local dev |
| Secrets/config management | `hashicorp/vault` in `server -dev` mode (in-memory, fixed dev root token — Vault's own documented dev-mode flag) | `localstack` (AWS Secrets Manager) | only needed if the app fetches secrets at runtime rather than via env vars |
| Service discovery | `hashicorp/consul` in `-dev` mode | — | only needed if the app resolves other services through it instead of DNS/network alias |
| Third-party or internal API with no self-hosted engine (tier 3) | — | `wiremock/wiremock` (programmable stubbing), `stoplight/prism` (mocks straight from an OpenAPI spec) | naming a concrete tool here beats a "hand-rolled stub" every time one of these fits the shape of the API |

Apply §2's procedure to whatever the project's own stack actually needs,
including categories not listed here. Image names, tags and maintenance
status drift — confirm the current image/tag before pinning one.

**Prefer a vendor's bundled multi-service emulator suite over assembling
one container per category**, when the project is deeply tied to one
cloud/vendor — it keeps auth and config consistent across services:
AWS-heavy → LocalStack alone already covers S3/SQS/SNS/DynamoDB/Lambda/
Secrets Manager/SES; Firebase-heavy → the Firebase Local Emulator Suite
covers Firestore, Realtime Database, Auth, Functions, Pub/Sub and Storage
together; Azure has no single suite that broad, so its services stay
per-emulator (Azurite for storage, a separate Cosmos DB emulator, a
separate Service Bus emulator) — check whether one exists for the
specific Azure service before assuming Azurite alone covers it. Some
LocalStack services are paid-tier only (Cognito, for example) — check
before planning around one.

## 4. OAuth2 / OIDC identity → `references/identity.md`

When the app authenticates users or services (OIDC login, API bearer
tokens, client credentials, SAML, LDAP/AD), read `references/identity.md`
in full before choosing a tool. It covers classifying the flow first
(4.1), mock-oauth2-server, Keycloak and Dex (4.2), matching production's
token and claim shape (4.3), the issuer-URL trap between the browser and
the container network (4.4), redirect URIs and CORS (4.5), SAML and
LDAP/AD (4.6), and dev-fixture credentials (4.7).

## 5. Mail → `references/mail.md`

When the app sends or reads mail, read `references/mail.md` in full. It
covers classifying SMTP vs. provider API vs. inbound mailbox (5.1),
Mailpit as the SMTP capture sink, so dev never sends real mail (5.2),
transactional-mail HTTP APIs such as SES, SendGrid and Postmark (5.3), and
GreenMail for inbound mailboxes (5.4).

## 6. Bootstrapping each resource — pick the right pattern

- **Schema/migration tools** (Flyway, Liquibase, Prisma Migrate, Alembic,
  etc.) — for relational/document DBs, run the project's own migration
  tool against the container once it's healthy.
- **Declarative imports the engine supports natively** — Keycloak realm
  import, Dex/mock-oauth2-server config files, LDIF for LDAP, RabbitMQ
  `definitions.json`. Prefer these over a scripted seed: they're one file
  in the repo, applied on every fresh start.
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
  adding a redundant seed step.
- **Mail capture / cache / mock-oauth2-server** — typically need zero
  bootstrap beyond their config; the app just needs the right connection
  env var pointed at them.

## 7. Compose mechanics

- Own `docker-compose.dev.yml`/`docker-compose.infra.yml`, `name:
  <project>` at the top (Compose otherwise prefixes every resource with
  the invoking directory's name, silently diverging from the shared
  network/volume names devcontainers expect), joined to the project's
  external network, one stable service name per resource for other config
  to reference (`db`, `cache`, `mailpit`, `auth`).
- **Give every long-running infra service a `healthcheck`** — it gates
  seed steps, and it's what verification and any app-boot wait logic
  should key off, instead of a hand-rolled `sleep`. Several images ship
  without `curl`/`wget` (Keycloak, some distroless images) — use the
  image's own CLI, a bash `/dev/tcp` probe, or the engine's health
  endpoint via a tool the image *does* include; check before writing
  `curl` into the test.
- Persist state in a named volume only where dev actually benefits from
  surviving a restart (a DB's data, a broker's queues if durability itself
  is under test) — deliberately skip a volume where a clean slate on every
  restart is the more useful default (a cache, mail capture, mock
  identity, an IdP whose realm is re-imported anyway). This is a
  **different lifecycle** than an ephemeral test container
  (Testcontainers, CI service containers), which should stay ephemeral —
  don't merge the two.
- **Reuse an image the project already trusts, instead of introducing a
  new variant.** If the test suite already pulls a specific image for this
  same dependency (via Testcontainers or similar), reuse that exact image
  here.
- Expose convenience host ports for native GUI clients and bundled UIs (a
  DB client, RabbitMQ's/Mailpit's/Keycloak's web UI) — the app itself
  still talks over the network alias; say so in a comment. An IdP's port
  is the exception that *must* be reachable from the host, since the
  browser logs in through it (§4.4 of `references/identity.md`).
- **Optional companion UI containers** for resources that don't bundle one
  (Kafka/Redpanda has none built in — add `redpandadata/console` or
  `kafka-ui` only if the team will actually use it).

## 8. Emulator credentials are not secrets

Several emulators ship a fixed, publicly documented dev credential that
means nothing outside that emulator (Azurite's well-known
`devstoreaccount1` account/key pair, Vault's dev root token, mock-OIDC
signing keys, Mailpit's accept-any SMTP auth, test-user passwords in a
dev realm export). Commit these as-is in `.env.example` — they guard
nothing real, and inventing a "safer-looking" placeholder adds friction
for no security benefit. Real production credentials still never go in
the repo.

## 9. Verification — one real round trip per resource

A healthcheck only proves the process started, not that the app can use
it. From the app's own container, over the network alias:

- DB: migrations run; the app reads/writes one row.
- Cache: set + get a key.
- Queue/stream: publish + consume one message.
- Storage: write + read one object.
- SMTP: trigger one real app email (signup, password reset) and find it
  via Mailpit's API — confirm subject, recipient and rendered links.
- Mail API: one send reaches the sandbox/emulator/stub, and nothing
  reached a real inbox.
- Inbound mail: send to GreenMail, confirm the app ingests it.
- Identity: log in through the **browser** as each persona (§4.3 of `references/identity.md`), call a
  protected API endpoint with the resulting token and see it accepted;
  call it with the no-roles persona and see it **rejected**; call it with
  no token and see a 401. A dev IdP that makes everything pass is a
  bypass, not a simulation.
- Stubs: the app's real client hits the stub and parses the response.

Clean up any container/volume created only for the verification.

## 10. Document the gaps

In the project's README or docs, per resource: what runs in dev, what runs
in prod, the tier, and the behavior gaps for tier 2–3 choices (e.g.
"Entra ID simulated with mock-oauth2-server — no conditional access, no
group overage claim, no real MFA"), plus the test personas and their
credentials.

## Pitfalls (observed firsthand — check these before assuming a fresh bug)

| Symptom | Cause | Fix |
|---|---|---|
| Compose resources named `<invoking-dir>_<resource>` instead of the intended project name | Compose prefixes resource names with the invoking directory when no explicit name is set | add a top-level `name: <project>` to the compose file; clean up anything already created under the stale prefix |
| A seed/init step intermittently fails right after `docker compose up` | `depends_on` without a `condition` only waits for the target container to **start** | add a `healthcheck` to the target service and gate the seed step on `condition: service_healthy` |
| The app still reaches the real cloud endpoint even with the emulator container running | Cloud SDKs default to the real service; the emulator needs an explicit endpoint override (`AWS_ENDPOINT_URL`/path-style flag, an Azure emulator connection string) | set the override explicitly in the dev `.env`/`.env.example`, and confirm it with §9's round trip |
| Login works in the browser, but the API rejects the token with 401 "invalid issuer" | `iss` holds the browser-facing URL (`localhost`), the API is configured with the internal alias URL (or vice versa) | §4.4 of `references/identity.md` — split issuer and JWKS URLs, or pin the IdP's public hostname |
| API fails to start: can't fetch OIDC discovery | `issuer-uri`-style config fetches discovery at boot from a URL only the browser can reach, or before the IdP is healthy | use the alias for JWKS/discovery from the container; gate the API on the IdP's healthcheck |
| Authenticated, but every endpoint is 403 | Dev tokens carry roles under a different claim than prod (`realm_access.roles` vs. `roles`) | emit prod's exact claim shape (§4.3 of `references/identity.md`) instead of changing the app's mapping |
| IdP login page says "invalid redirect_uri" | dev redirect URI not registered exactly (port, path, trailing slash, proxy prefix) | add the exact URI to the realm export / mock config |
| App errors on SMTP connect: "AUTH not supported" or TLS handshake failure | Mailpit's defaults (no auth, no TLS) don't match what the client insists on | enable accept-any auth / give Mailpit a cert (§5.2 of `references/mail.md`) rather than editing the app's mail config |
| Emails "sent" but nothing in Mailpit | app points at `localhost:1025` from inside its container (that's its own loopback) | use the alias `mailpit:1025` |
