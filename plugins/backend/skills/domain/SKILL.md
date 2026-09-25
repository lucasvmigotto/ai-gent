---
name: domain
description: Model the backend domain from the product brief — bounded contexts, aggregates and invariants, events, lifecycles, policies, consistency boundaries, data ownership and personal-data classification — in docs/product/domain-model.md. Use for "model the domain", "design the backend", "define entities and business rules". Runs after project:init.
---

# Domain model

## Role

Act as a senior backend engineer and software architect specialized in
domain-driven design, data modeling, distributed systems, API design,
security and reliability. Model **behavior and rules**, not tables: what
the business does, what must never happen, and where consistency really
matters. Prefer the simplest model and deployment shape that honors the
rules and the non-functional requirements.

## Before starting

1. Read `../../references/pipeline.md` and
   `../../references/backend-quality.md` (the modeling tells apply here).
2. Read `docs/product/brief.md` — glossary, business rules, NFRs,
   integrations, volumes. Without it, work from the user's text or
   references (default `docs/product/references/`) and say so.
3. Read `specs/` if `project:spec` has run (features, user stories, key
   entities, `data-model.md` drafts) and `docs/product/ux-vision.md`
   (flows reveal commands and read models).
4. If code exists, extract the current model (entities, tables, status
   fields, services) and mark each element keep / evolve / replace.

Clarify (pipeline rule 2) with questions targeted at rules that change
the model: what must never happen, who may do what, what happens
on conflict (two people booking the last slot), what must be audited,
how long data is kept, which numbers are money/time/quantities.

## Process

1. **Event storming, on paper.** Walk the brief's key journeys and list,
   in order: actors → commands → domain events (past tense:
   `BookingConfirmed`) → policies ("when X happens, do Y") → external
   systems → read models the actors need. This list drives everything
   below.
2. **Bounded contexts.** Group by language and consistency needs; draw a
   context map (ASCII/mermaid) with relationships (customer/supplier,
   conformist, anti-corruption layer for external systems). Map contexts
   onto the deployment shape `docs/product/architecture.md` decided (one
   module or service per context); if the model argues for a different
   shape, record it as an `[UPSTREAM GAP]` for `project:architecture`
   rather than deciding it here.
3. **Aggregates.** For each: root, members, identity strategy (UUIDv7/
   ULID vs. natural key, and whether IDs are exposed), **invariants**
   stated as "must always / must never", which commands it handles,
   which events it emits. Keep aggregates small — consistency inside,
   eventual between.
4. **Value objects.** Money (amount + currency, integer minor units),
   time (instants in UTC, local dates with an explicit zone, durations),
   quantities, contact details, identifiers — with validation rules.
5. **Lifecycles.** A state machine for every entity with a status:
   states, allowed transitions, the command causing each, guards, the
   event emitted, and what's terminal. Illegal transitions listed.
6. **Policies & business rules.** Each with its source (brief line,
   user answer, or `[ASSUMPTION]`), where it's enforced (aggregate,
   policy handler, DB constraint), and its test idea.
7. **Authorization model.** Roles/relationships (owner, member, admin,
   tenant), and a first **actor × command/query** matrix — the base for
   `backend:spec`'s authz matrix. Note object-level rules ("a member sees
   only their own bookings").
8. **Read models & access patterns.** The queries screens and
   integrations need, their filters/sorts, expected volume and latency —
   these decide indexes and projections later.
9. **Consistency & concurrency.** Where strong consistency is required,
   where eventual is acceptable, known contention points (last seat,
   stock), and the strategy (optimistic locking, unique constraints,
   reservations with expiry).
10. **Data ownership & lifecycle.** Per entity: owning context,
    personal-data classification (none / personal / sensitive under
    LGPD/GDPR), retention and deletion/anonymization rules, audit needs.
11. **Integrations.** Per external system: direction, sync vs. async,
    the anti-corruption translation, idempotency, failure behavior.
12. **Volumes.** Expected counts and rates per aggregate and hot path,
    from the brief's NFRs — numbers, not adjectives.

Review the model against `backend-quality.md`'s modeling tells (CRUD
everywhere, anemic model, noun-per-service, free-string status) and
revise; state what changed.

## Output — `docs/product/domain-model.md`

Sections: Summary · Inputs used (and missing) · Event storming list ·
Context map · Per context: aggregates, value objects, lifecycles,
policies · Authorization model · Read models & access patterns ·
Consistency & concurrency · Data ownership, privacy & retention ·
Integrations · Volumes · Open questions · Decision log.

Use glossary names exactly; any new term goes into the brief's glossary
first (pipeline rule). Mark the file `Status: Draft` (`Accepted` once the user agrees).

## Coverage checklist

- [ ] every brief journey appears in the event-storming list
- [ ] every entity with a status has a state machine with illegal transitions listed
- [ ] every aggregate has invariants stated as must always / must never
- [ ] every business rule has a source and an enforcement point
- [ ] every command has at least one actor in the authorization matrix
- [ ] money, time and identifiers use value objects with explicit rules
- [ ] every entity has an owner, a privacy classification and a retention rule
- [ ] every integration has a failure behavior
- [ ] reviewed against the modeling tells; revisions stated

## Handoff

Summarize contexts, the riskiest invariants and contention points, open
questions, and the next stage: `backend:spec` (after `project:spec` has
created features). Commits follow `git:workflow`.
