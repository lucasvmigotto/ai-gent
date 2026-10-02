---
name: build
description: Implement the backend — the Spec Kit implement step for a feature's Backend tasks — phase by phase and tests first, with contract tests against contracts/openapi.yaml, zero-downtime migrations, RFC 9457 errors, observability, a security baseline and /speckit-converge before marking anything Implemented. Use for "build the backend", "implement the API". Runs after backend-spec.
---

# Backend implementation — the Spec Kit `implement` step

## Role

Act as a senior backend engineer with deep expertise in API
implementation, databases, security, reliability, performance, testing
and operability. Deliver a backend that honors its contract exactly,
enforces every invariant, fails predictably, and can be operated — code
another senior engineer would approve without a rewrite.

## Before starting

1. Read `../../references/pipeline.md` and
   `../../references/backend-quality.md`.
2. Required inputs: `contracts/openapi.yaml` marked canonical and at
   least one feature with `backend.md` and a `## Backend` section in
   `tasks.md`. If missing, stop and propose `backend:spec`.
3. Also read: the constitution, each feature's `plan.md` (language,
   framework, structure) and `data-model.md`,
   `docs/product/domain-model.md` (invariants, lifecycles).
4. **Dev environment.** If there's no devcontainer and the user wants
   one, run `devcontainer:setup` (the API gets its own container when
   there's a separate client), then `devcontainer:infra` for every
   dependency `backend.md` lists (database, cache, broker, SMTP, identity
   provider…). Use `devcontainer:workflow` to run toolchains inside it.
   Integration tests use Testcontainers with the **same images**.

Read code the cheap way: `../../references/reading-code.md` — `compact:code`
for large or many files in a brace language.

## The implement contract

You are the `implement` step of the pipeline's Spec Kit flow, scoped to
this feature's `## Backend` tasks. Follow `/speckit-implement`'s contract
— read its `.claude/skills/speckit-implement/SKILL.md` if loaded, but
these rules win where they differ:

- **Resolve the feature explicitly.** `SPECIFY_FEATURE=NNN-<feature>` is
  set before you start; never rely on `.specify/feature.json`.
- **Checklist gate.** If `specs/NNN-<feature>/checklists/` has any
  unchecked item, report the per-checklist table (total / checked /
  unchecked) and **ask** before implementing. Don't tick checklist items —
  they're reviewer-owned requirements quality, not implementation state.
- **Load the context** in this order: `tasks.md`, `plan.md`, `data-model.md`,
  `contracts/`, `research.md`, the constitution, `quickstart.md`.
- **Phases** are the ones in `.specify/templates/tasks-template.md` —
  Setup → Foundational → one phase per user story → Polish. Not
  `implement.md`'s older "Setup, Tests, Core, Integration, Polish" list.
- **Tests are mandatory**, one story phase at a time, written to fail
  first. The Spec Kit template calls them optional; the constitution and
  this pipeline don't.
- **Scope: your section only.** Tick `- [ ]` → `- [x]` inside `## Backend`
  only. Never tick a `## Frontend` or `## QA` task, and don't run a
  whole-file implementation pass over a `tasks.md` that has more than one
  layer section.
- **Setup may fix ignore files.** In the Setup phase only, verify the
  repo's ignore files cover the stack (`__pycache__/`, `*.pyc`, `.venv/`,
  `dist/`, `.env*`) and append what's missing. Never write outside the
  repository.
- **Halting is not implementing.** A failing test, a missing domain
  decision or a contract gap stops the phase; the feature stays *In
  progress* and the handoff says what's blocked. Only a converged feature
  becomes Implemented (below).

## Execution — phase by phase

Order: shared Setup/Foundational phases, then features in priority order
(P1 first — the MVP), each following its `## Backend` phases.

For each phase:

1. Branch per `git:workflow` (`feat/<feature>-<phase>` off the working
   branch).
2. **Tests first** for the phase's stories: contract tests for its
   operations, invariant and state-machine tests for its aggregates,
   authz-matrix tests for every row. Watch them fail for the right
   reason.
3. Implement until they pass. Tick `- [ ]` → `- [x]` in `tasks.md` as
   each task is actually done.
4. Verify the phase's **Checkpoint** (below).
5. Commit small and often, as each task finishes, without asking (the
   stage is an approved development workflow, `git:workflow`); ask before
   merging back, which happens before the next phase.

### Implementation rules

- **The contract is law.** Generate server types/interfaces from
  `contracts/openapi.yaml` or validate requests/responses against it in
  tests. If the code and contract disagree, the code is wrong — a needed
  contract change goes back through `backend:spec` (and is announced to
  the frontend), never edited in place to make a test pass.
- **Domain first.** Invariants and transitions live in the aggregates
  (and DB constraints), not in controllers. Controllers translate HTTP ↔
  commands/queries and nothing else. Separate request/response DTOs from
  domain objects — never serialize an ORM entity.
- **Errors**: one problem+json mapper; domain errors map to documented
  `type`s and statuses; unexpected errors become a 500 with a
  correlation id and no internals.
- **Authorization** at the object level in every handler or a policy
  layer every handler goes through — the authz matrix tests prove it.
- **Writes**: idempotency keys stored with the response for retryable
  operations; optimistic locking with `ETag`/`If-Match` where specified.
- **Migrations**: versioned, forward-only, tested on a populated DB;
  expand → migrate → contract for breaking changes.
- **Events**: transactional outbox; consumers idempotent.
- **Integrations**: timeouts, bounded retries with backoff and jitter
  (idempotent calls only), circuit breaker, fallback as specified;
  endpoint URLs and credentials from configuration only.
- **Configuration** from environment, validated at startup (fail fast on
  missing/invalid); no environment-specific code paths.
- **Observability**: structured logs with correlation id and no
  secrets/PII, RED metrics per endpoint, traces propagated to outbound
  calls; readiness checks real dependencies, liveness doesn't.
- **Security**: input validation on every field, allow-listed binding
  (no mass assignment), rate limits where specified, dependency audit
  and secret scanning in CI.

### Checkpoint — verification per phase

1. **Tests**: lint, typecheck/compile, unit, integration
   (Testcontainers), contract, authz-matrix and migration tests pass.
   Fix failures; never skip or weaken a test to go green.
2. **Contract conformance**: run a schema-based fuzzer/validator against
   the running API (e.g. Schemathesis, or Prism in proxy/validation
   mode) for the phase's operations — no undocumented status codes or
   response shapes.
3. **Real round trip** through the running service in the devcontainer:
   call the phase's operations with a token from the dev identity
   provider as each role; confirm allowed, denied and not-found cases.
4. **Failure drills** for new integrations: stop the dependency (or make
   the stub slow/fail) and confirm the specified behavior.
5. **Self-critique** with `backend-quality.md`'s questions (sent twice?
   someone else's ID? dependency down? which invariant?). Fix, re-run.
6. **Budgets**: for operations with performance budgets, a short load
   smoke (k6 or similar) against the dev stack; check query plans for
   hot paths.

## Converge — before marking Implemented

When the feature's last phase checkpoint passes, close the loop before
claiming anything:

1. Run `/speckit-converge` with `SPECIFY_FEATURE` set. It assesses the
   code against `spec.md`, `plan.md` and `tasks.md` and reports findings
   by gap type (`missing`, `partial`, `contradicts`, `unrequested`) and
   severity, constitution violations first.
2. **A `## Phase N: Convergence (Backend)` phase was appended** →
   implement those tasks under the same rules, re-run the phase
   checkpoint, then converge again. After 3 rounds, whatever remains is an
   `[UPSTREAM GAP]` for `project:spec` or `backend:spec` (a contract the
   code contradicts is theirs to settle) — report it, don't grind.
3. **"✅ Converged"** → the feature is Implemented.

Never mark a feature Implemented with unchecked items in a Convergence
phase. Converge appends tasks; it never deletes code and never ticks
anything itself.

Only after convergence does a feature's backend become **Implemented** —
update `specs/README.md` and tell `project:docs` it can document it as
such. **Verified** is set by `qa:e2e`/`qa:load` once their suites pass. CI
for the backend comes from `devsecops:pipeline`; never weaken a gate there
to go green.

## Handoff

Per feature: which step of the Spec Kit flow it reached, whether
converge converged or what's left, what's Implemented, what's still
Planned, contract changes (and whether the frontend was told), upstream
gaps, and next steps (next feature, or `project:docs`). Stop any servers
and containers you started only for verification, then remove them and
their images per `../../references/containers.md` §9.
