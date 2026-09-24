# Backend quality reference

Shared by `backend:domain`, `backend:spec` and `backend:build` — the
backend counterpart of the frontend's `visual-direction.md`. Generated
backends cluster around recognizable defaults, just as generated UIs do.
Each is legitimate for *some* design, but they show up regardless of the
domain; treat every one as a choice you must be able to justify for this
product, not a starting point.

## AI-default backend tells

**Modeling**

1. **CRUD for every table** — `POST/GET/PUT/DELETE /things` with no
   domain operations. Real domains have verbs: `confirm`, `cancel`,
   `reschedule`, `approve`. State changes that carry rules deserve their
   own command (`POST /bookings/{id}/cancel`), not a `PUT` with a
   `status` field anyone can set.
2. **Anemic model** — entities are bags of getters/setters, all rules
   live in "service" classes, invariants enforced nowhere in particular.
3. **Every noun is an aggregate / a microservice** — boundaries drawn by
   nouns instead of by consistency needs. Default to a modular monolith;
   split a service only for a named reason (independent scaling, team
   ownership, a different availability requirement).
4. **Status as a free string** — no explicit state machine, so illegal
   transitions (`cancelled → confirmed`) are possible.

**API**

5. **ORM entities returned as API responses** — internal fields,
   relations and lazy-loading leak into the contract; any schema change
   becomes a breaking API change.
6. **`200 OK` with an error in the body**, or every failure as `500`.
   Use proper status codes and one error format (RFC 9457
   `application/problem+json`) with a stable machine-readable `type`/code.
7. **Unbounded collections** — no pagination, no maximum page size, no
   stable sort.
8. **No idempotency** on retryable writes (payments, bookings, anything
   a client might retry after a timeout) — use `Idempotency-Key`.
9. **No concurrency control** — last write wins silently. Use optimistic
   locking (version column, `ETag`/`If-Match`).
10. **Stringly-typed everything** — dates without time zones, money as
    floats, IDs as sequential integers exposed publicly when enumeration
    matters.

**Security**

11. **Authorization only at the gateway / only "is logged in"** — no
    object-level checks (BOLA, OWASP API #1). Every handler checks *this*
    actor may do *this* action on *this* resource.
12. **Mass assignment** — request body bound straight onto the entity.
13. **Secrets or personal data in logs**, error messages or URLs.
14. **Trusting client-supplied identity/tenant fields** instead of the
    token.

**Data**

15. **Hand-edited schemas** instead of versioned migrations; migrations
    that can't run twice or on a populated table.
16. **Invariants only in code** that the database could also enforce
    (NOT NULL, UNIQUE, FK, CHECK, exclusion constraints).
17. **N+1 queries** and missing indexes for the documented access
    patterns.
18. **Dual writes** (DB + message broker) without an outbox — events lost
    or duplicated on failure.

**Operations**

19. **`catch (e) { log(e) }`** — swallowed errors, no context, no
    correlation id.
20. **No timeouts, retries or circuit breakers** on outbound calls; one
    slow integration takes the whole service down.
21. **Configuration hardcoded**, or different code paths per environment
    instead of different values.
22. **Health check that always returns 200** — doesn't check the DB or
    critical dependencies (separate liveness from readiness).

## Quality floor (never below this)

- Every endpoint: authn, object-level authz, input validation, documented
  errors, and a test for each.
- Every invariant: enforced in the domain model **and**, where possible,
  by a database constraint; covered by a test that tries to break it.
- Every collection endpoint paginated with a max page size.
- Every outbound call: timeout, bounded retries with backoff (idempotent
  calls only), and a defined behavior when the dependency is down.
- Every schema change: a forward migration tested on a populated DB.
- Structured logs with correlation id, no secrets/PII; RED metrics
  (rate, errors, duration) per endpoint; traces across outbound calls.
- Contract tests against `contracts/openapi.yaml` pass; CI fails on
  drift.
- Security baseline: OWASP ASVS level agreed in the constitution
  (default L2 for apps handling personal data), dependency audit and
  secret scanning in CI.

## Self-critique questions

Before calling a design or an implementation done:

- Could this API belong to any other product with the nouns swapped? If
  yes, the domain verbs are missing.
- What happens if this request is sent twice? Concurrently? After a
  timeout?
- Who can call this with someone else's ID?
- What happens when each dependency is slow, down, or returns garbage?
- Which invariant would a bug here violate, and what stops it?
- Can this migration run on production data without downtime?
