# Signals — what a codebase reveals

Used by `project:introspec` (`../SKILL.md`). A signal makes a claim
`INFERRED`; confirming config (compose service, env variable name, a
client constructed with that endpoint) makes it `OBSERVED`.

## Where things live, by stack

| Stack | Routes / entry points | Models and schema | Jobs and consumers |
|---|---|---|---|
| Spring (Boot) | `@RestController`, `@RequestMapping`, `@*Mapping`; `springdoc` → `/v3/api-docs` | `@Entity`, Flyway `db/migration/V*.sql`, Liquibase changelogs | `@Scheduled`, `@RabbitListener`, `@KafkaListener`, `@JmsListener` |
| Quarkus / Micronaut | JAX-RS `@Path`; `@Controller` | Panache entities, Flyway/Liquibase | `@Scheduled`, `@Incoming` (reactive messaging) |
| Node (Express, Fastify, Nest, Hono) | `app.get/post`, route files, Nest `@Controller` | Prisma `schema.prisma` + `migrations/`, TypeORM entities, Sequelize models, Knex/Drizzle migrations | BullMQ/Agenda workers, `node-cron`, `amqplib`/`kafkajs` consumers |
| Next.js / Remix / SvelteKit | `app/**/route.ts`, `pages/api`, loaders/actions | as Node | route handlers on cron, queue workers |
| Django | `urls.py`, DRF viewsets/routers | `models.py`, `migrations/` | Celery tasks, `beat` schedule, management commands |
| Flask / FastAPI | decorators; FastAPI → `/openapi.json` | SQLAlchemy models, Alembic `versions/` | Celery, RQ, APScheduler |
| Rails | `config/routes.rb` | `db/schema.rb`/`structure.sql`, `db/migrate/` | ActiveJob, Sidekiq, `whenever` |
| Laravel / Symfony | `routes/*.php`; attributes/annotations | Eloquent models + `database/migrations`; Doctrine entities + migrations | queues, `Kernel` schedule, Messenger |
| .NET | `[ApiController]`, minimal APIs `Map*`; Swashbuckle → `/swagger` | EF Core `DbContext`, `Migrations/` | `IHostedService`, Hangfire, MassTransit consumers |
| Go | `http.HandleFunc`, chi/gin/echo routers | sqlc/GORM/ent schemas, golang-migrate/goose files | goroutine workers, `robfig/cron`, consumers |
| Rust | axum/actix routers | Diesel `schema.rs`, SQLx `migrations/`, SeaORM entities | tokio tasks, `lapin`/`rdkafka` consumers |

## Libraries and config → services

| Signal | Suggests | Confirm with |
|---|---|---|
| `amqplib`, `spring-boot-starter-amqp`, `pika`, `lapin`, MassTransit RabbitMQ | RabbitMQ (AMQP 0-9-1) | compose `rabbitmq`, `RABBITMQ_*`/`AMQP_URL`, exchange/queue declarations |
| `kafkajs`, `spring-kafka`, `confluent-kafka`, `rdkafka` | Kafka | `bootstrap.servers`, topic names |
| JMS, `activemq`, `qpid` | ActiveMQ / Artemis / AMQP 1.0 broker | broker URL scheme |
| AWS SDK `sqs`/`sns`/`s3`/`ses`/`dynamodb` | those AWS services | region, endpoint override (LocalStack?) |
| `@azure/*`, `Azure.*` | Azure Service Bus, Blob, Key Vault… | connection string variable names |
| `redis`, `ioredis`, `lettuce`, `jedis`, `spring-data-redis` | Redis (cache, sessions, locks, queues — find which) | key prefixes, TTLs, `@Cacheable` |
| `elasticsearch`, `opensearch`, `meilisearch`, `typesense` | search engine | index names, mappings |
| `nodemailer`, `spring-boot-starter-mail`, `smtplib`, SMTP host vars | SMTP mail | `MAIL_*`/`SMTP_*` |
| SendGrid, Postmark, Mailgun, SES SDKs | transactional mail API | API key variable names |
| `openid-client`, `spring-security-oauth2-*`, `next-auth`, `passport-*`, Keycloak adapters | OIDC / OAuth2 provider | issuer URL variable, realm, client id names |
| `ldapjs`, `spring-ldap`, `python-ldap` | LDAP / Active Directory | bind DN variable names |
| `stripe`, `pagseguro`, `mercadopago`, `adyen` | payment provider | webhook routes, key variable names |
| `@sentry/*`, OpenTelemetry, Datadog, New Relic agents | error tracking / tracing | DSN/endpoint variable names |
| `unleash`, `launchdarkly`, `flagsmith` | feature flags | flag names in code |
| JDBC URLs, `pg`, `mysql2`, `oracledb`, `mssql`, `sqlite3`, Mongo drivers | the database engine (and version, from the driver and compose image) | datasource config, migrations dialect |
| `Dockerfile`, compose, Helm, Terraform, `Procfile`, `app.yaml`, `vercel.json`, `fly.toml` | hosting and topology | CI deploy jobs |

## Tools

- **SBOM:** `syft dir:. -o cyclonedx-json`, or `cdxgen -o docs/product/sbom.cdx.json`;
  for images, `syft <image>`.
- **Structure and hotspots:** `git log --format= --name-only | sort | uniq -c | sort -rn`
  (churn), `cloc`/`tokei` (size), language-specific dependency graphs
  (`madge`, `jdeps`, `pydeps`, `go mod graph`).
- **Schema from a dev database:** the engine's catalog (`information_schema`,
  `pg_catalog`, `ALL_TAB_COLUMNS`/`ALL_CONSTRAINTS`, `sys.*`), or
  `pg_dump --schema-only`, `mysqldump --no-data`; ER diagrams from the
  result (mermaid `erDiagram`). Catalog queries only — no data rows.
- **Generated API descriptions:** `/v3/api-docs` (springdoc),
  `/openapi.json` (FastAPI), `/swagger/v1/swagger.json` (Swashbuckle),
  `drf-spectacular` schema command. Reconcile with the code: generated
  descriptions miss undocumented routes and error shapes.
