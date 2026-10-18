# Reference catalog of stand-ins (§3)

Part of `devcontainer:infra` (`../SKILL.md`): read it when choosing the image or emulator for a resource category; names and tags drift, so confirm the current ones before pinning.

| Category | Prod-identical option (tier 1) | Common tier-2 substitute | Notes |
|---|---|---|---|
| Relational DB | `postgres`, `mysql`/`mariadb` official images | `gvenzl/oracle-free` (Oracle); `mcr.microsoft.com/mssql/server` is itself a real free edition, not actually a gap | SQLite usually needs no container — it's a file |
| Document / key-value DB | `mongo` official image | `amazon/dynamodb-local` (DynamoDB) | |
| Cache | `redis` official image | `docker.dragonflydb.io/dragonflydb/dragonfly` — RESP-compatible, worth it when prod itself runs Dragonfly, or as a faster local swap once compatibility is confirmed against the app's actual command usage | rarely needs a persistent volume — a clean slate on restart is usually the point |
| Object/blob storage | `mcr.microsoft.com/azure-storage/azurite` (Azure — Microsoft's own emulator, tier 1) | `localstack/localstack` (broad AWS emulation: S3/SQS/SNS/DynamoDB/Lambda) or the lighter S3-only `minio/minio`, for AWS; `fsouza/fake-gcs-server` for GCS | |
| Queue / streaming | `rabbitmq` (the `-management` tag adds the web UI), `nats` | `apache/kafka` in KRaft mode (no ZooKeeper needed since Kafka 3.3+) for full Kafka; `redpandadata/redpanda` when only the Kafka wire protocol matters and a lighter single-binary footprint is worth it | confirm KRaft env vars (`KAFKA_PROCESS_ROLES=broker,controller`, a node/cluster ID) — older Kafka compose examples still show the legacy two-container ZooKeeper setup |
| Outbound mail (SMTP) | — | `axllent/mailpit` | always a capture sink regardless of prod's real provider — dev must never send real mail. Details in §5 of `mail.md` |
| Inbound mailbox (IMAP/POP3) | — | `greenmail/standalone` | for apps that *read* mail; §5.4 of `mail.md` |
| Transactional mail HTTP API (SES, SendGrid, Postmark, Mailgun) | vendor sandbox (tier 0) | LocalStack SES; WireMock stub | §5.3 of `mail.md` |
| Browser automation (Chrome, Firefox, Edge, Chromium) | `selenium/standalone-chrome`, `selenium/standalone-firefox`, `selenium/standalone-edge`, `selenium/standalone-chromium` — the vendor's own browser+driver bundles, tier 1 | — | behind a `browser` profile, endpoint from env, `shm_size` for Chrome; WebKit has no standalone flavor (Playwright container only) — full rules in the qa plugin's references/browsers.md |
| OAuth2 / OIDC identity provider | `quay.io/keycloak/keycloak` when prod runs Keycloak | `ghcr.io/navikt/mock-oauth2-server`, Keycloak, `ghcr.io/dexidp/dex` | details and the flow-based choice in §4 of `identity.md` |
| SAML IdP | Keycloak (as a SAML IdP) when prod is Keycloak | Keycloak | §4.6 of `identity.md` |
| LDAP / Active Directory | `osixia`/Bitnami OpenLDAP-style images (check current maintenance) | `lldap/lldap` for simple user/group lookups | §4.6 of `identity.md` |
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
