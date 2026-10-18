# Bootstrapping each resource (§6)

Part of `devcontainer:infra` (`../SKILL.md`): read it when a resource needs a schema, a bucket, a topic or other seed data.

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
