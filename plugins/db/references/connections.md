# Connection profiles

How `db:*` skills find databases. The agent never reads, prints or
`source`s the values of a profile file: `dbrun profiles` lists names,
engines, classes and sources, and `dbrun` passes credentials to the
client itself (a temporary mode-600 env file, never the command line).

## The profiles file

`${AI_GENT_DB_CONFIG:-~/.config/ai-gent/db}/connections.env`, mode 600,
outside every repository. It is **parsed**, never executed: one
`KEY=VALUE` per line, `#` comments, optional single or double quotes, no
variable expansion, no command substitution.

```bash
# <PROFILE>_<KEY>=value — profile names are [A-Z][A-Z0-9_]*
SHOP_DEV_ENGINE=postgresql
SHOP_DEV_CLASS=development
SHOP_DEV_HOST=db-dev.internal.example
SHOP_DEV_PORT=5432
SHOP_DEV_DATABASE=shop
SHOP_DEV_USER=shop_readonly
SHOP_DEV_PASSWORD=<set by the user>
SHOP_DEV_PROJECT=shop
```

| Key | Meaning |
|---|---|
| `_ENGINE` | `postgresql`, `mysql`, `mariadb`, `sqlserver`, `oracle` or `sqlite` |
| `_CLASS` | `production`, `staging`, `development` or `local`. **Missing = production.** `local` is only a request: the runner proves it (`safety.md` §2) or treats the target as remote |
| `_HOST`, `_PORT` | server address (remote profiles) |
| `_DATABASE` | database name (PostgreSQL, MySQL/MariaDB, SQL Server) |
| `_SERVICE` | Oracle service name |
| `_USER`, `_PASSWORD` | credentials — preferably a read-only account |
| `_PATH` | SQLite file |
| `_CONTAINER` | for a local profile: the database container's name |
| `_PROJECT` | optional: the repository the profile belongs to; `dbrun` lists it first inside that repository |

The file is the user's: the agent may create it with placeholder keys
(`<set by the user>`) and mode 600, but never writes a real credential
into it, and never asks the user to paste one into the conversation.

## Auto-discovered local profiles

`dbrun profiles` also lists `local:<service>` for each running database
container of this repository's compose project (images of PostgreSQL,
MySQL, MariaDB, SQL Server, Oracle). Their credentials come from the
container's own environment (`POSTGRES_USER`, `MYSQL_ROOT_PASSWORD`,
`MSSQL_SA_PASSWORD`, `ORACLE_PASSWORD`, …) — dev fixtures from
`devcontainer:infra`, read by the runner, never printed. They still go
through the full local proof before any write.

## Precedence

1. The profile the user names.
2. A proven local container of this project (`local:<service>`).
3. Profiles in the file whose `_PROJECT` matches this repository.
4. Other profiles in the file.

A remote target is never chosen automatically: when the user hasn't named
one, `dbrun` suggests, the agent asks. The same name from two sources is
an error to resolve with the user, not a tie to break.

## Project configuration is evidence, not a profile

`DATABASE_URL`, `.env.example`, `application*.yml`, `appsettings.*.json`,
`database.yml`, `schema.prisma`, `alembic.ini`, `flyway.conf`,
`liquibase.properties` and CI service definitions tell which engine and
database a project uses — `db:inspect` and `db:review` read them for
that. They never become a connection: a real password in a repository is
a finding (`db:review`), not a credential to use.

## Providers (later)

A private plugin with its own credential store (such as a work-specific
one) could plug in as a **provider**: a descriptor pointing at its
profiles file, a mapping from its naming scheme to profiles and classes,
and its own rules file. Provider rules may only tighten `safety.md`,
never loosen it; its classes follow the same "missing = production" rule.
Not implemented yet.
