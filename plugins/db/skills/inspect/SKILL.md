---
name: inspect
description: Discover a database in depth through the audited runner — settings, tables with estimated sizes, columns, keys and constraints, indexes (unused, duplicate, invalid), views, routines, triggers, grants, links, legacy tables — into an inspection report, a schema-only export and an ERD, or diff two databases. Read-only, masked. Use for "inspect the database", "map this schema", "compare schemas".
---

# Database inspection

Read `../../references/safety.md`, `../../references/runner.md` and the
target's engine file in `../../references/engines/` first. Every statement
goes through `dbrun query` with a `--reason`; inspection is read-only on
every class, local included.

## Before starting

1. Pick the profile with the user (`db:connect` if unsure) and run
   `dbrun classify` — the class decides the cost rules and where the
   schema export may live.
2. Read the project's own view of the database when there is one:
   migrations, ORM models, `docs/product/domain-model.md`,
   `docs/product/architecture.md` — the inspection reports where the real
   schema differs.

## Process

Run the engine file's catalog queries, cheapest first, bounded:

1. **Server** — engine, version, settings that change behavior
   (isolation, time zone, character set, SQL mode, compatibility level).
2. **Size** — tables with estimated rows and bytes from statistics, and
   how fresh those statistics are (never a full count).
3. **Structure** — per table: columns (type, nullable, default,
   identity/generated), primary key, foreign keys with their actions,
   unique and check constraints, partitions.
4. **Implicit relations** — columns named like keys (`customer_id`,
   `*_fk`) with no constraint: listed as `[INFERRED: naming → FK to …;
   confirm with …]`, checked cheaply for orphans only when the user wants
   it.
5. **Indexes** — definitions; invalid/unusable/disabled; unused (with the
   window the statistics cover); duplicates; foreign keys without an
   index.
6. **Hidden logic** — views, routines, triggers (with their status),
   sequences and their headroom (`last_value` vs. `max_value`), synonyms.
   Triggers and routines often hold business rules the application
   doesn't: summarize what each does.
7. **Access** — grants per role and the account's own rights; outbound
   links (a finding in itself).
8. **Hygiene** — legacy and backup tables (name patterns and statistics
   freshness), tables with no primary key, disabled or unvalidated
   constraints, columns that are always null (from statistics, not
   scans).

Label every claim like `project:introspec`: `[OBSERVED: <catalog view>]`,
`[INFERRED: …]`, `[ASSUMPTION: …]`.

## Outputs

- `docs/product/db/<database>/inspection.md` — `Status: Draft`; profile
  and class (never connection details), server, size overview, per-area
  findings, a mermaid ERD of the main tables (full ERD in a separate
  section when large), risks, and differences from the project's
  migrations and models.
- `schema.sql` — schema only, no data, no grants/users/links. From
  **development or local** it goes next to the report. From
  **production or staging** it goes to the runner's state directory
  (`…/state/…/exports/`) and is committed only if the user says the
  repository is private and approves.
- Examples in the report are shapes and counts, masked (`safety.md` §5).

## Diff mode

Two profiles (e.g. development vs. staging): compare tables, columns,
constraints, indexes, routines and triggers; report each difference with
which side has it. Compare counts only from statistics. Output
`docs/product/db/<database>/diff-<a>-<b>.md`.

## Coverage checklist

- [ ] class and its evidence recorded; cost rules followed for that class
- [ ] every table: size estimate, keys, constraints, indexes
- [ ] implicit relations listed as Inferred
- [ ] invalid, unused and duplicate indexes; FKs without an index
- [ ] triggers, routines and views summarized; disabled ones flagged
- [ ] grants, account rights and outbound links
- [ ] legacy/backup tables and tables without a primary key
- [ ] schema export stored where the class allows; no data, no credentials

## Handoff

The report's headline findings, then: `devcontainer:infra` can build a
local copy from `schema.sql` (a sandbox where writes are allowed);
`project:introspec` reads the report instead of querying again;
`db:review` checks the application against it.
