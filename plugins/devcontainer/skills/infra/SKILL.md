---
name: infra
description: Simulate the external resources an app depends on with docker-compose for local/devcontainer development — SQL/NoSQL databases, caches, object storage, queues and streaming, SMTP capture (Mailpit), mailboxes (GreenMail), mail APIs, OAuth2/OIDC providers (mock-oauth2-server, Keycloak, Dex, stand-ins for Entra ID/Okta/Auth0/Cognito), SAML and LDAP, search, vector DBs, observability, feature flags, secrets managers and API stubs — each chosen by a vendor-sandbox / identical / substitute / stub tier procedure with the version gap documented. Use for "simulate <a database/queue/cache/storage/mail/OAuth/SSO> locally", "mock the identity provider", "catch outgoing emails", "get a real JWT for local dev", or from devcontainer:setup. Works for existing devcontainers too.
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
(§4.1 and §5.1) — the flow or transport decides the tool more than the
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
| Outbound mail (SMTP) | — | `axllent/mailpit` | always a capture sink regardless of prod's real provider — dev must never send real mail. Details in §5 |
| Inbound mailbox (IMAP/POP3) | — | `greenmail/standalone` | for apps that *read* mail; §5.4 |
| Transactional mail HTTP API (SES, SendGrid, Postmark, Mailgun) | vendor sandbox (tier 0) | LocalStack SES; WireMock stub | §5.3 |
| OAuth2 / OIDC identity provider | `quay.io/keycloak/keycloak` when prod runs Keycloak | `ghcr.io/navikt/mock-oauth2-server`, Keycloak, `ghcr.io/dexidp/dex` | details and the flow-based choice in §4 |
| SAML IdP | Keycloak (as a SAML IdP) when prod is Keycloak | Keycloak | §4.6 |
| LDAP / Active Directory | `osixia`/Bitnami OpenLDAP-style images (check current maintenance) | `lldap/lldap` for simple user/group lookups | §4.6 |
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

## 4. OAuth2 / OIDC identity

### 4.1 Classify how the app uses identity first

| App's role | What the dev IdP must provide | Lightest fit |
|---|---|---|
| **Resource server only** — the API validates bearer JWTs, never logs anyone in | an issuer, a discovery document, a JWKS, and a way to mint tokens with chosen claims | mock-oauth2-server |
| **Service-to-service** — `client_credentials` calls to another API | a token endpoint accepting any/known client id+secret | mock-oauth2-server |
| **Browser login** — the client (SPA or server-side) runs authorization code + PKCE | a real login page, redirect URI handling, id_token, usually refresh tokens and logout | mock-oauth2-server's interactive login for "type any user + claims"; Keycloak when the flow needs real users, passwords, consent, logout/session behavior |
| **Admin-side identity logic** — the app manages users/roles/groups via the IdP's admin API, or relies on realms, fine-grained roles, identity brokering | the IdP's actual admin API | Keycloak (tier 1 if prod is Keycloak, tier 2 otherwise — say which) |
| **Enterprise SSO** — SAML, LDAP/AD bind or lookup | §4.6 | Keycloak / an LDAP image |

A project with a split API + client (see `devcontainer:setup`) usually
needs **both** the resource-server row (for `api`) and the browser-login
row (for `web`) from the **same** IdP instance, so tokens the client gets
are ones the API accepts.

### 4.2 The tools

- **`ghcr.io/navikt/mock-oauth2-server`** — a real OAuth2/OIDC server with
  no users to manage. Each URL path segment is its own issuer
  (`http://auth:8080/<issuerId>` with its own
  `/.well-known/openid-configuration`, `/jwks`, `/token`, `/authorize`),
  so several audiences/issuers come from one container. Supports
  `authorization_code` (with PKCE), `client_credentials`, `refresh_token`,
  JWT-bearer and token-exchange grants. `interactiveLogin: true` shows a
  form where you type any subject plus a JSON claims blob. Deterministic
  claims per request go in `tokenCallbacks` in its JSON config
  (`JSON_CONFIG` env var or `JSON_CONFIG_PATH` mounted file). Fastest
  option by far; the gap is that it has no user store, no admin API and no
  realistic session/logout.
- **Keycloak (`quay.io/keycloak/keycloak`)** — run `start-dev
  --import-realm` with a realm export committed to the repo, mounted at
  `/opt/keycloak/data/import/`. The realm file declares clients (with the
  dev redirect URIs), roles, groups and test users, so a fresh container
  is ready to log in with no clicking. Bootstrap admin via
  `KC_BOOTSTRAP_ADMIN_USERNAME`/`KC_BOOTSTRAP_ADMIN_PASSWORD` (older
  releases used `KEYCLOAK_ADMIN*` — check the pinned version). Heavier
  (JVM, ~several hundred MB RAM, slower boot) — pick it for the rows in
  §4.1 that need it, not by default.
- **Dex (`ghcr.io/dexidp/dex`)** — small OIDC provider driven by one YAML
  file (`staticClients`, `staticPasswords`, or a connector to LDAP/GitHub).
  A middle ground when you want real login with fixed users but not
  Keycloak's weight.
- **Firebase Auth / Cognito** — Firebase Auth has its own emulator in the
  Firebase suite (tier 1); Cognito's LocalStack emulation is paid-tier —
  otherwise simulate with mock-oauth2-server (tier 2) and document it.

### 4.3 Match production's token shape, not just "a valid JWT"

The app's authorization code (role mapping, tenant checks, user lookup)
must run **unchanged** against dev tokens, so the dev IdP must emit prod's
claim names and structure. Read them from the app's security config and
any decoded prod token in a runbook — don't guess:

- **Entra ID (Azure AD) v2**: `iss` of the form
  `https://login.microsoftonline.com/<tid>/v2.0`, `aud` = the API's app ID
  URI or client id, `tid`, `oid`, `preferred_username`, `name`, app roles
  in `roles`, delegated scopes in `scp` (space-separated string).
- **Keycloak**: realm roles in `realm_access.roles`, client roles in
  `resource_access.<client>.roles`, `preferred_username`, `email`.
- **Okta/Auth0**: custom claims are often namespaced
  (`https://<your-namespace>/roles`) — copy the exact key.

Configure the dev IdP to produce exactly those (mock-oauth2-server
`tokenCallbacks`, Keycloak protocol mappers/realm roles). Put 2–3 named
test users/personas in the config (an admin, a regular user, a user with
no roles) — they're what makes authorization paths testable, not just
authentication.

### 4.4 The issuer-URL trap (browser vs. container network)

The browser runs on the host and reaches the IdP at
`http://localhost:<port>`; the API container reaches it at
`http://auth:<port>`. Tokens carry whichever URL the IdP saw as its own
in `iss`, and most JWT validators reject a token whose `iss` doesn't
match the configured issuer **exactly**. Pick one of:

- **Split issuer from JWKS in the API config** (usually simplest): validate
  `iss` = the browser-facing URL, but fetch keys from the internal alias.
  Spring: set `jwk-set-uri: http://auth:8080/<...>/jwks` and validate the
  issuer claim separately, instead of `issuer-uri` (which also fetches
  discovery from that URL at startup). Other stacks have the same two knobs.
- **Pin the IdP's public URL**: Keycloak `KC_HOSTNAME=http://localhost:8080`
  (plus `KC_HOSTNAME_BACKCHANNEL_DYNAMIC=true` so backchannel calls from
  `api` work over the alias); mock-oauth2-server derives the issuer from
  the request `Host`, so have both sides send the same host.
- **Make one name resolve everywhere**: route the browser through the
  `devcontainer:proxy` simulation so both sides use the same public URL.

Whatever you pick, write it down in the compose file comments — this is
the single most common "works in Postman, 401 in the app" cause.

### 4.5 Redirect URIs, CORS, and the client config

- Register the **exact** dev redirect URIs (the forwarded dev-server URL,
  and the proxy URL if `devcontainer:proxy` is in play) in the realm
  export / mock config; wildcard redirect URIs hide the path bugs the
  proxy simulation exists to catch.
- The SPA calls the IdP's token endpoint from the browser, so the IdP
  must allow the dev origin (Keycloak client "Web origins"; mock-oauth2-server
  allows CORS by default).
- Keep the client ids/scopes/audiences **the same names as prod** in dev
  config wherever possible; only the authority URL should differ between
  environments.

### 4.6 SAML and LDAP/AD

- **SAML SP** (the app consumes SAML assertions): Keycloak can act as a
  SAML IdP from the same realm import — one container for OIDC and SAML
  if both are needed.
- **LDAP bind/lookup** (the app authenticates against or reads from a
  directory, common with on-prem Active Directory): run an LDAP server
  seeded from a committed LDIF with the same base DN, OU layout and
  attribute names (`sAMAccountName`, `memberOf`, `userPrincipalName`) the
  app queries. `lldap/lldap` is light and fine for plain user/group
  lookups; a full OpenLDAP image is needed for custom schema. AD-only
  behavior (nested-group `LDAP_MATCHING_RULE_IN_CHAIN`, `objectGUID`
  binary handling) is a documented tier-2 gap — note it.
- If the app talks to Keycloak which federates LDAP in prod, mirror that
  topology rather than pointing the app at LDAP directly.

### 4.7 Identity credentials are dev fixtures

Test-user passwords, mock client secrets and the dev admin login are
fixtures, not secrets (§8) — commit them in the realm export /
`.env.example` and list them in the README so anyone can log in.

## 5. Mail — SMTP, mail APIs, inboxes

### 5.1 Classify how the app sends/receives mail

- **SMTP** (Spring Mail, Nodemailer, Django `EmailBackend`, PHPMailer,
  `smtplib`, …) → Mailpit, §5.2.
- **Provider HTTP API** (SES SDK, SendGrid/Postmark/Mailgun SDKs) → §5.3.
- **Reads a mailbox** (IMAP/POP3 polling, bounce processing, ticket
  ingestion) → §5.4.

### 5.2 SMTP capture with Mailpit

`axllent/mailpit`: SMTP on `1025`, web UI + REST API on `8025`. It never
delivers anywhere unless relay is explicitly configured — keep it that way.

- Point the app at `mailpit:1025` by the network alias. Match what the
  app's SMTP client insists on rather than weakening the app's config:
  - app always sends `AUTH` → `MP_SMTP_AUTH_ACCEPT_ANY=true` plus
    `MP_SMTP_AUTH_ALLOW_INSECURE=true` (any username/password accepted);
  - app requires STARTTLS/TLS (common on port 587/465 configs) → give
    Mailpit a self-signed cert via `MP_SMTP_TLS_CERT`/`MP_SMTP_TLS_KEY`
    (optionally `MP_SMTP_REQUIRE_STARTTLS=true`) and trust it / disable
    verification only in the dev profile.
- Keep the SMTP host/port/auth/TLS settings as **env vars the app already
  reads**, so dev and prod exercise the same code path with different
  values.
- Use the REST API for assertions in integration/e2e tests
  (`GET /api/v1/messages`, `GET /api/v1/search?query=to:<addr>`,
  `DELETE /api/v1/messages` to reset between tests) — don't scrape the UI.
- Healthcheck: Mailpit exposes `/livez` and `/readyz` on the HTTP port.
- Useful extras when the team will use them: `MP_MAX_MESSAGES` to cap
  retention, the built-in HTML/link checks for template work, and recent
  versions' chaos option to make SMTP return errors on demand for testing
  retry/bounce handling.
- No persistent volume by default — an empty inbox on restart is the more
  useful state.
- MailHog is the older, unmaintained equivalent; migrate to Mailpit if a
  reference project still uses it.

### 5.3 Transactional-mail HTTP APIs

Mailpit only speaks SMTP, so an app calling a provider's HTTP API needs
something else, in this order:

1. **Vendor sandbox (tier 0)** when online dev is acceptable: SendGrid
   `mail_settings.sandbox_mode.enable=true` (validates, never sends),
   Postmark's `POSTMARK_API_TEST` server token, Mailgun `o:testmode=yes`.
   Beware "sandbox domains" that *do* deliver to allow-listed addresses —
   that's real sending, not a sandbox.
2. **Emulator**: AWS SES via LocalStack (sent messages are retrievable
   from LocalStack's `/_aws/ses` endpoint) — set the SDK endpoint
   override (see Pitfalls).
3. **Stub (tier 3)**: WireMock with the provider's request/response shape
   when the vendor has no sandbox and must be offline.

Don't switch the app to an SMTP transport "just for dev" unless the
transport is already a config choice prod also uses — otherwise dev tests
a code path prod never runs.

### 5.4 Inbound mailboxes

For apps that read mail, `greenmail/standalone` provides SMTP + IMAP +
POP3 (ports `3025`/`3143`/`3110`, SSL variants `3465`/`3993`/`3995`) with
users declared via `GREENMAIL_OPTS` (e.g.
`-Dgreenmail.setup.test.all -Dgreenmail.users=inbox:secret@example.test
-Dgreenmail.hostname=0.0.0.0`). Seed test mail by sending to its SMTP
port. If the app both sends and reads, run Mailpit for outbound and
GreenMail for the mailbox — don't make one do both jobs.

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
  browser logs in through it (§4.4).
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
- Identity: log in through the **browser** as each persona (§4.3), call a
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
| Login works in the browser, but the API rejects the token with 401 "invalid issuer" | `iss` holds the browser-facing URL (`localhost`), the API is configured with the internal alias URL (or vice versa) | §4.4 — split issuer and JWKS URLs, or pin the IdP's public hostname |
| API fails to start: can't fetch OIDC discovery | `issuer-uri`-style config fetches discovery at boot from a URL only the browser can reach, or before the IdP is healthy | use the alias for JWKS/discovery from the container; gate the API on the IdP's healthcheck |
| Authenticated, but every endpoint is 403 | Dev tokens carry roles under a different claim than prod (`realm_access.roles` vs. `roles`) | emit prod's exact claim shape (§4.3) instead of changing the app's mapping |
| IdP login page says "invalid redirect_uri" | dev redirect URI not registered exactly (port, path, trailing slash, proxy prefix) | add the exact URI to the realm export / mock config |
| App errors on SMTP connect: "AUTH not supported" or TLS handshake failure | Mailpit's defaults (no auth, no TLS) don't match what the client insists on | enable accept-any auth / give Mailpit a cert (§5.2) rather than editing the app's mail config |
| Emails "sent" but nothing in Mailpit | app points at `localhost:1025` from inside its container (that's its own loopback) | use the alias `mailpit:1025` |
