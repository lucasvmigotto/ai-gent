# Connection profiles

How `db:*` skills find databases. The agent never reads, prints or
`source`s the values of a profile file: `dbrun profiles` lists names,
engines, classes and sources, and `dbrun` passes credentials to the
client itself (a temporary mode-600 env file, never the command line).

## The profiles files

In `${AI_GENT_DB_CONFIG:-~/.config/ai-gent/db}/`, mode 600, outside
every repository: `connections.toml` is read first, then
`connections.env`; either may be missing, and a profile defined in both
is an error. `AI_GENT_DB_CONNECTIONS=<file>` names one file instead, read
by its extension (a team's shared file, say). Both formats are
**parsed**, never executed: no variable expansion, no command
substitution.

### TOML — profiles grouped in families

```toml
[defaults]                  # every profile starts from these
class = "development"

[families.shop]             # what several profiles share: one account, one password
engine = "postgresql"
host = "db-dev.internal.example"
port = 5432                 # a string or an integer
user = "shop_readonly"
password = "pass:team/db/shop"

[profiles.SHOP_DEV]
family = "shop"
database = "shop_dev"
project = "shop"

[profiles.SHOP_QA]
family = "shop"
database = "shop_qa"
host = "db-qa.internal.example"   # a profile's own keys win

[profiles.LEGACY]
engine = "oracle"
host = "ora.internal.example"
user = "app_ro"
sid = "ORCL"                # no service name: a SID
```

Keys are the ones below, lowercase. A profile's keys win over its
family's, which win over `[defaults]`. Unknown tables or keys are errors,
and errors name a table, key or line, never a value. Reading TOML needs
Python 3.11 or later (`tomllib`, no extra package); `dbrun` doesn't
rewrite TOML files.

**Families** carry one password for everything that uses one account:
`password = "pass:…"` once, or no password at all and the entry
`ai-gent/db/<family>` is used. Name a family after the **account** it
stands for, never after matching password values — two accounts that
happen to share a password get two families and two entries.

### .env — one key per line

```bash
# <PROFILE>_<KEY>=value — profile names are [A-Z][A-Z0-9_]*
SHOP_DEV_ENGINE=postgresql
SHOP_DEV_CLASS=development
SHOP_DEV_HOST=db-dev.internal.example
SHOP_DEV_PORT=5432
SHOP_DEV_DATABASE=shop
SHOP_DEV_USER=shop_readonly
SHOP_DEV_PROJECT=shop
# no _PASSWORD: it lives in pass, as ai-gent/db/shop-dev
```

One `KEY=VALUE` per line, `#` comments, optional single or double quotes.
`dbrun profiles --export-env --out <file>` writes the profiles of both
files as a new `.env` (mode 600, nothing printed) for tools that read
only `.env`, with passwords as `pass:` references; a plain value is left
out.

### Keys

| Key | Meaning |
|---|---|
| `ENGINE` | `postgresql`, `mysql`, `mariadb`, `sqlserver`, `oracle` or `sqlite` |
| `CLASS` | `production`, `staging`, `development` or `local`. **Missing = production.** `local` is only a request: the runner proves it (`safety.md` §2) or treats the target as remote |
| `HOST`, `PORT` | server address (remote profiles) |
| `DATABASE` | database name (PostgreSQL, MySQL/MariaDB, SQL Server); for Oracle, a service name |
| `SERVICE` | Oracle service name |
| `SID` | Oracle SID, when there's no service name: `dbrun` builds a connect descriptor, since Easy Connect can't name a SID. Order: `SERVICE`, then `DATABASE`, then `SID` |
| `USER` | the account — preferably a read-only one |
| `PASSWORD` | `pass:<entry>` to name a pass entry; left out, the family's or the profile's default entry is used when it exists; a plain value only as the last resort (see below) |
| `PATH` | SQLite file |
| `CONTAINER` | for a local profile: the database container's name |
| `PROJECT` | optional: the repository the profile belongs to; `dbrun` lists it first inside that repository |
| `family` | TOML only: the `[families.<name>]` the profile starts from |

Before connecting, `dbrun` checks the keys the engine needs and names
the missing ones (`HOST`, `USER`; `SERVICE|SID` for Oracle; `PATH` for
SQLite).

The files are the user's: the agent may create one with the non-secret
keys and mode 600, but never writes a credential into it, and never asks
the user to paste one into the conversation.

## Passwords: pass first

A profile's password comes from the first of these that applies
(`secrets.md`, from the `secrets` plugin, has the rules for every secret):

1. `pass:<entry>` — that pass entry, in the personal store or a mounted
   one (`pass:team/db/shop`, `pass:work/db/shop`).
2. No password — the entry `ai-gent/db/<family>` for a profile in a
   family, else `ai-gent/db/<profile>` (lowercased, `_` → `-`:
   `SHOP_DEV` → `ai-gent/db/shop-dev`), when it exists.
   `AI_GENT_PASS_PREFIX` replaces `ai-gent`.
3. A plain value — the **last resort**, for a machine without pass.
   `dbrun profiles` flags it.

Moving plain values into pass, never showing them (`.env` only):

- `dbrun secrets migrate <PROFILE>` — one profile: the value goes to
  `ai-gent/db/<profile>` and its line is removed.
- `dbrun secrets migrate --all` — every plain value, each line replaced
  in place by its `pass:` reference. Profiles sharing an account (the same
  `USER` and password) share one entry, `ai-gent/db/<user>`; anything
  else gets its own. It reports names and counts only, leaves an entry
  that already exists alone, and flags a password **reused by different
  accounts** — names only — as a hygiene finding to rotate.

For a TOML file, put the value in pass yourself (`pass insert …`) and
write `password = "pass:<entry>"`.

`dbrun` decrypts the entry only to connect (`test`, `query`, `plan`,
`apply`, `restore`), never to list or classify, and redacts it like any
password. The user adds it in their own terminal —
`pass insert ai-gent/db/shop-dev` prompts without echo. If the key is
locked and can't prompt, `dbrun` stops with the command that unlocks it;
the user runs it, then the agent retries. A team store is fetched before
reads when it's stale (`secrets.md` → *Team stores*).

## Auto-discovered local profiles

`dbrun profiles` also lists `local:<service>` for each running database
container of this repository's compose project (images of PostgreSQL,
MySQL, MariaDB, SQL Server, Oracle). Their credentials come from the
container's own environment (`POSTGRES_USER`, `MYSQL_ROOT_PASSWORD`,
`MSSQL_SA_PASSWORD`, `ORACLE_PASSWORD`, …) — dev fixtures from
`devcontainer:infra`, read by the runner, never printed. They still go
through the full local proof before any write. SQL Server's image has no
variable naming a user database, so `local:<service>` for it connects to
`master`, where writes are refused: add a profile with `CONTAINER` and
`DATABASE` for the project's database.

## Precedence

1. The profile the user names.
2. A proven local container of this project (`local:<service>`).
3. Profiles whose `PROJECT` matches this repository.
4. Other profiles.

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

pass is the credential store behind profiles, including shared team
stores. Another private plugin with its own store could plug in as a
**provider**: a descriptor pointing at its profiles file, a mapping from
its naming scheme to profiles and classes, and its own rules file.
Provider rules may only tighten `safety.md`, never loosen it; its classes
follow the same "missing = production" rule. Not implemented yet.
