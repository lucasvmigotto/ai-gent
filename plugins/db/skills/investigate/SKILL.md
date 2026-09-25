---
name: investigate
description: Investigate a data problem from a scenario and a symptom — wrong, missing or duplicated records, broken relations — telling hand edits from application bugs and integration faults with aggregate evidence, a timeline and the blast radius, and delivering the repair as a script a person runs (or a local plan). Use for "why is this record wrong", "find the bad data", "who changed this".
---

# Data investigation

Read `../../references/safety.md`, `../../references/runner.md` and the
engine's file in `../../references/engines/` first. Every statement
goes through `dbrun query` with a `--reason` naming the hypothesis it
tests. On anything but a proven local container the investigation is
read-only, and the repair is a script a person runs.

## 1. Frame it

With the user, in one round: the scenario (what should happen), the
symptom (what happened instead), where it was seen (profile, a record's
id, a screen, a report), when it started, and what "fixed" means. Pick
the profile and `dbrun classify` it. For a long investigation, `dbrun
session start <PROFILE>` keeps the client warm; stop it at the end.

## 2. Hypotheses before queries

List the plausible causes and what evidence would confirm or rule out
each, then query for that evidence only:

- **Hand edit** — values the application's validation can't produce; a
  changed value next to an unchanged `updated_at`; audit columns holding
  a database user instead of an application user; many rows sharing one
  exact timestamp; ids outside the sequence's range; entries in the
  engine's audit trail (pgaudit, Oracle audit or flashback, SQL Server
  CDC or temporal tables, the MySQL binlog) when available.
- **Application bug** — the pattern starts at a deploy (`git log` on the
  code that writes the column, the release dates); it spans many users or
  records the same way; it matches a code path (a missing transaction, a
  race, a wrong default, a timezone conversion).
- **Integration** — the bad values arrive through an import, a queue, a
  sync job or another system's writes (who else has write rights —
  `db:inspect`'s grants section).
- **Schema** — a missing or disabled constraint let it in (orphans,
  duplicates a unique index would have blocked).

## 3. Evidence

- Aggregate first: counts by day, status, source, user or version,
  filtered on indexed columns; exact counts only within small windows
  (`safety.md` §4).
- Then a few masked examples to confirm the shape; unmask only if the
  user asks.
- Build a **timeline** (first bad record, deploys, imports, manual
  sessions) and measure the **blast radius**: how many records, which
  tenants or customers, since when, what downstream data depends on them.
- Separate a one-off gap from a systemic one by counting, not by a
  single example.

## 4. Root cause and repair

- Name the class (hand edit, application bug, integration, schema), the
  evidence for it, and what is still uncertain.
- **Repair on a remote database:** `dbrun script` — pre-checks with the
  expected counts, the change in a transaction, verification queries and
  a rollback; for a person to review and run.
- **Repair on a proven local container** (reproducing the fix first):
  `dbrun plan` → the user confirms → `dbrun apply … --confirmed`, then
  `dbrun restore` if it was an experiment.
- **Prevention:** the code fix (`backend:build`), the missing constraint
  as a migration (`db:review`), a regression test (`qa:review`), or a
  process fix for hand edits (who may write where).

## Output

`docs/product/db/investigations/<date>-<slug>.md` — `Status: Draft`;
scenario and symptom, hypotheses with their verdicts, the evidence
queries (statements, not results), timeline, blast radius, root cause,
the repair script's path, prevention, open questions. Counts and masked
shapes only.

## Coverage checklist

- [ ] every hypothesis confirmed or ruled out with evidence
- [ ] timeline and blast radius measured by counts
- [ ] root cause classified, uncertainty stated
- [ ] repair delivered as a script (remote) or a confirmed local plan
- [ ] prevention handed to its owner
- [ ] no raw personal data in the report; session stopped

## Handoff

Root cause in two lines, the blast radius, the repair script and who
must run it, and the prevention handoffs.
