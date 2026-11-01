# Compose mechanics

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
- **Limits and profiles on every service** (`../../../references/containers.md`
  §7): `cpus`, `mem_limit`, `pids_limit`, sized so the whole stack stays
  within about half the host; each optional resource behind a compose
  profile (`db`, `mail`, `auth`, `search`) so a task starts only what it
  needs. Heavy engines (Elasticsearch/OpenSearch, Kafka, Keycloak, SQL
  Server) get their memory settings capped explicitly (`ES_JAVA_OPTS`,
  `KAFKA_HEAP_OPTS`, `JAVA_OPTS_KC_HEAP`, `MSSQL_MEMORY_LIMIT_MB`) —
  the JVM or engine default sizes itself from the host, not the limit.
- Pin images by digest with the tag as a comment, preferring hardened
  images where the catalog has the service (`containers.md` §3). Run the
  stack with `$CONTAINER_ENGINE compose`; the Podman notes in
  `containers.md` §1 (host gateway, `:Z`, UID mapping) apply here too.
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
  browser logs in through it (§4.4 of `identity.md`).
- **Optional companion UI containers** for resources that don't bundle one
  (Kafka/Redpanda has none built in — add `redpandadata/console` or
  `kafka-ui` only if the team will actually use it).
