---
name: spec
description: Turn the domain model and the Spec Kit features into an implementation-ready, contract-first backend specification — the canonical contracts/openapi.yaml (and asyncapi.yaml when events leave the service), a backend.md layer on every feature (endpoints with authz, validation, errors as RFC 9457 problem+json, pagination, idempotency, concurrency; persistence with tables, constraints, indexes and migrations; transactions, jobs and outbox events; integrations with timeouts and retries; observability; security; performance budgets; test plan) and Backend tasks appended to each tasks.md. Use when the user asks to "spec the backend", "design the API", "write the OpenAPI contract", "plan the backend implementation", or runs /backend:spec. Second stage of the backend chain, after backend:domain and project:spec.
---

# Backend specification (contract-first, Spec Kit)

## Role

Act as a senior backend engineer and API architect with deep expertise
in API design, data modeling, databases, security, reliability,
performance, testing and operability — writing specifications an
engineer can implement without guessing, where the contract is the
source of truth.

## Before starting

1. Read `../../references/pipeline.md` (ownership: this skill **adds a
   backend layer** to features `project:spec` owns, and **owns the
   canonical contract**) and `../../references/backend-quality.md`.
2. Required inputs:
   - `docs/product/domain-model.md` — if missing, propose
     `backend:domain` first.
   - `specs/` with features and `contracts/openapi.yaml` skeleton — if
     missing, propose `project:spec` first.
3. Also read: the brief (glossary, NFRs, compliance), the constitution,
   each feature's `spec.md`/`plan.md`/`data-model.md`/`tasks.md`, and
   each feature's `ui.md` if `frontend:spec` has run (the screens' data
   needs are consumers of your contract).

## Process

### 1. Coverage matrix

| Command / query (domain model) | Feature | User story | `operationId` | Consumer (screen / integration) |
|---|---|---|---|---|

Every command/query that crosses the service boundary gets an operation;
every `ui.md` data need maps to one. Missing on either side →
`[UPSTREAM GAP]`, reported, not invented.

### 2. The canonical contract — `contracts/openapi.yaml`

Promote the skeleton to canonical (`x-status: canonical`), OpenAPI 3.1:

- **Resources and verbs from the domain**: collection/item resources for
  reads; explicit command endpoints for state transitions that carry
  rules (`POST /bookings/{bookingId}/cancel`) — not a writable `status`.
- **Schemas**: separate request and response schemas per operation (no
  shared "entity" schema), `readOnly`/`writeOnly` where needed, formats
  for every string (`uuid`, `date-time`, `email`), money as integer minor
  units + currency, enums matching the lifecycle states.
- **Errors**: one `Problem` schema (RFC 9457) with a stable `type`/code
  catalog; each operation lists its real error responses (400, 401, 403,
  404, 409, 412, 422, 429) — no catch-all.
- **Collections**: cursor pagination (or offset with a documented
  reason), max page size, stable sort, filters from the access patterns.
- **Writes**: `Idempotency-Key` on retryable creates/commands;
  `ETag`/`If-Match` (412) for concurrent updates.
- **Security schemes** matching the identity provider (see
  `devcontainer:infra` §4 for the dev simulation), scopes/roles per
  operation.
- **Examples** for every request and response, with real domain values.
- Lint it (Spectral or Redocly with a ruleset committed to the repo) and
  keep each feature's `specs/NNN/contracts/` a matching subset.
- `contracts/asyncapi.yaml` for events consumed outside the service.

### 3. `backend.md` for every feature

In `specs/NNN-<feature>/backend.md`:

- **Endpoints** — per `operationId`: purpose, actor(s), the aggregate
  command/query it maps to, validation rules, side effects, emitted
  events, idempotency and concurrency behavior, error cases → problem
  `type`.
- **Authorization matrix** — rows = operations, columns = roles/
  relationships, cells = allow / deny / condition ("own only"). Every
  cell becomes a test.
- **Persistence** — tables/collections, columns with types, constraints
  enforcing invariants (NOT NULL, UNIQUE, FK, CHECK, exclusion), indexes
  justified by access patterns, the migrations in order, data backfills,
  and zero-downtime notes (expand → migrate → contract).
- **Transactions & concurrency** — transaction boundary per command,
  locking strategy, contention points from the domain model.
- **Async work** — background jobs (trigger, schedule, retries,
  idempotency), domain events via transactional outbox, consumers and
  their dedup strategy.
- **Integrations** — per outbound call: timeout, retry/backoff, circuit
  breaker, fallback, the anti-corruption mapping; and the dev simulation
  for each (hand the list to `devcontainer:infra`).
- **Observability** — log events (fields, no PII), metrics (RED per
  endpoint + business metrics), traces, alerts worth having.
- **Security** — ASVS level from the constitution, threats specific to
  this feature (BOLA, mass assignment, enumeration, rate limiting),
  personal-data handling and retention from the domain model.
- **Performance** — budgets per operation (p95 latency, throughput) from
  the NFRs, and the query plan concern for each hot path.
- **Test plan** — unit (aggregate invariants and state machines),
  integration with real dependencies (Testcontainers, same images as
  `devcontainer:infra`), contract tests against the OpenAPI file,
  authz-matrix tests, migration tests on a populated DB, and a load
  smoke for operations with budgets.

Technical backend decisions not in the feature `plan.md` (framework
modules, ORM/query builder, migration tool, job runner, validation
library) go in the first backend feature's `backend.md` under
"Technical decisions" with rationale — the language/stack itself comes
from `plan.md`/the constitution.

### 4. Backend tasks

Append a `## Backend` section to each feature's `tasks.md` in Spec Kit's
format, phased: Setup → Foundational (migrations framework, error
handling, authn/authz, observability, contract-test harness) → one phase
per user story in priority order (tests first: contract + invariant +
authz tests, then implementation) → Polish (performance, hardening).
Each phase ends with a **Checkpoint** stating how to verify it.

### 5. Review and consistency

Review against `backend-quality.md` (tells, quality floor,
self-critique questions) and revise. Run `/speckit-analyze` across the
updated features; fix your layer, report the rest.

## Coverage checklist

- [ ] every boundary-crossing command/query has an `operationId`; every `ui.md` data need is served
- [ ] contract lints clean; every operation has examples and its real error responses
- [ ] no writable `status` field where a lifecycle exists — transitions are commands
- [ ] every operation has a filled authorization-matrix row
- [ ] every invariant has an enforcement point (domain + constraint where possible) and a test
- [ ] every collection is paginated with a max page size
- [ ] every retryable write is idempotent; every concurrent update has a concurrency check
- [ ] every outbound call has timeout, retry policy and failure behavior
- [ ] every migration is ordered and zero-downtime-safe or flagged
- [ ] every phase in `## Backend` has a checkpoint; `/speckit-analyze` run

## Handoff

List the features updated, contract changes the frontend must know
about, infra to simulate (for `devcontainer:infra`), upstream gaps, and
the next stage: `backend:build`. Commits follow `git-workflow`.
