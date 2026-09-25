---
name: review
description: Review how an application uses its database — ORM mappings vs. the real schema, migrations and drift, domain invariants vs. constraints, N+1 and unbounded queries, indexes, pooling, timeouts, isolation, account rights, credentials in config, backups — with ranked, evidenced findings and fixes as migrations or code on agreement. Use for "review our database usage", "check our queries".
---

# Database usage review

Read `../../references/safety.md`, `../../references/runner.md`, the
engine's file in `../../references/engines/` and
`../../../backend/references/backend-quality.md` (items on persistence,
migrations, queries and transactions — cite them instead of repeating
them). Database reads go through `dbrun query`; nothing is written to any
database during a review.

## Inputs

1. The application code: ORM models and mappings, repositories and raw
   SQL, migrations (in order), connection and pool configuration,
   transaction boundaries, jobs.
2. The real schema: `docs/product/db/<database>/inspection.md` if it
   exists, otherwise run `db:inspect` first (a development or local
   profile is enough for most checks).
3. `docs/product/domain-model.md` (invariants, lifecycles),
   `specs/*/backend.md` (persistence decisions), `architecture.md`
   (RPO/RTO, data stores).

## Checks

1. **Mapping vs. schema** — every entity against its table: types,
   lengths, nullability, defaults, enum values, relations the ORM assumes
   but no constraint enforces.
2. **Migrations** — ordered, repeatable, reversible where possible; drift
   between the migrations' result and the real schema (hand-made
   changes); zero-downtime practice for large tables.
3. **Invariants** — each "must always / must never" in the domain model
   enforced by a constraint, a unique index or a transaction — or only
   by application code (a finding when concurrency can break it).
4. **Queries** — N+1 patterns, unbounded reads (no `LIMIT`, no
   pagination), `SELECT *`, filters and sorts with no supporting index
   (check with plain `EXPLAIN` on development or local), implicit
   conversions that defeat indexes.
5. **Connections and transactions** — pool sizes against the server's
   limits and the capacity model, statement and lock timeouts,
   isolation level per use case, long transactions, retries without
   idempotency.
6. **Security** — the application's account rights (owner/DDL rights in
   production is a finding), credentials committed in config or examples
   (report the file and key, never the value), injection-prone string
   concatenation.
7. **Operations** — backups and point-in-time recovery against the RPO
   (from IaC or docs; missing evidence is an `[UPSTREAM GAP]`),
   monitoring of slow queries, retention of old data.

## Output

`docs/product/db/review-<date>.md` — `Status: Draft`; a findings table
(severity · finding · evidence `file:line` or catalog view · impact ·
fix · effort), then the details per finding.

## Fixes

Only when the user agrees, and only as code or **new migrations** in the
project's migration tool — never a direct change to any database.
Verify each fix locally (a local container through `dbrun`, and the
project's tests). Code defects go to `backend:build`, RPO and backup gaps
to `project:architecture` or `devsecops:iac`, missing tests to
`qa:review`.

## Coverage checklist

- [ ] every entity compared with its table
- [ ] migration drift checked against the real schema
- [ ] every domain invariant mapped to its enforcement
- [ ] N+1, unbounded and unindexed queries listed with evidence
- [ ] pool, timeouts and isolation against limits and load
- [ ] account rights and committed credentials checked
- [ ] backup/PITR evidence or an upstream gap

## Handoff

The top findings by severity, what was fixed and how it was verified,
and the handoffs above.
