---
name: introspec
description: Reverse-engineer an existing codebase into the pipeline's full specification — brief, as-is architecture, domain model, Spec Kit features with evidence-based statuses, and an OpenAPI contract from the real routes — labelling each claim Observed (file:line), Inferred or Assumed. May read a dev database, never production. Use for "reverse-engineer the spec", "document what this legacy app does".
---

# Introspective specification

Turns an existing project into the same artifacts the pipeline produces
for a new one, so every later stage (`project:retrofit`,
`project:refactor`, the frontend, backend and QA chains, `project:docs`)
works on it unchanged. The quality bar is a brand-new project's spec; the
difference is that every statement is traced to evidence.

## Role

Act as a senior software archaeologist and full-stack engineer: read code,
schemas, configuration, tests and history the way an auditor reads books.
Describe what the system **does**, not what its README claims or what it
should do. Behavior the code contradicts is a finding, not a typo to fix.

## Before starting

1. Read `../../references/pipeline.md` and `references/signals.md` (what
   each manifest, library and config file reveals).
2. Read existing pipeline artifacts. **If they exist, don't rewrite
   them** — compare them with the code and report drift in
   `docs/product/introspec.md` (and as `[UPSTREAM GAP]` notes). Reconstruct
   only what's missing.
3. Agree the boundaries in one round (pipeline rule 2):
   - what may run: the build and the test suite inside the project's
     devcontainer (`devcontainer:workflow`), the app itself, its dev
     database — each only with the user's yes;
   - which database is the **dev** one. Local containers
     (`devcontainer:infra`) and shared dev databases are fine for
     read-only catalog queries; **never production**, and never a
     database whose environment you can't confirm. When the `db` plugin is
     installed, delegate to `db:inspect` instead of querying yourself;
   - which parts are in scope (a monorepo may be several products).

## Evidence labels

Every claim in every artifact carries one:

- `[OBSERVED: path:line]` — seen in code, config, schema, tests or a run
  (for a run: the command and its output file).
- `[INFERRED: signal → conclusion; confirm with …]` — deduced, e.g. an
  AMQP client library → a message broker; `rabbitmq` in compose and a
  `RABBITMQ_URL` variable → RabbitMQ.
- `[ASSUMPTION: …]` — the pipeline's marker, for what only the user can
  confirm (why a rule exists, who the audiences are, business goals).

Prefer the cheapest strong evidence; a test that asserts a rule beats the
code implementing it, which beats a comment or a README.

## Process

1. **Inventory.** Repository map, languages and versions, manifests and
   lockfiles, build tools, containers, CI/CD, IaC, docs, test suites,
   licenses. Generate an SBOM (`references/signals.md` names the tools) at
   `docs/product/sbom.cdx.json` — `project:retrofit` starts from it.
2. **Entry points.** HTTP routes and controllers, GraphQL/gRPC schemas,
   CLIs, scheduled jobs, queue consumers, webhooks, UI routes. Each
   becomes an operation, a job or a screen in the inventory.
3. **Data.** ORM models, migrations (in order — they are the schema's
   history), raw SQL, views, triggers and stored procedures, seeds.
   Relations, constraints and indexes as the database enforces them
   (which may differ from the models). From the dev database, catalog
   queries only: tables, columns, keys, indexes, row-count estimates —
   no data rows, no personal data in any artifact.
4. **Integrations.** Every outbound call, SDK, queue, bucket, mail
   provider, identity provider and third-party API, from the signals
   table; each Observed or Inferred, with the config keys that select it
   (names only, never values).
5. **Rules.** Validation, guards, status fields and their transitions,
   authorization checks, calculations, scheduled policies, and test
   assertions. Status fields become state machines; guards become
   invariants; checks become the authorization matrix.
6. **Qualities.** Timeouts, retries, caching, rate limits, pagination,
   logging, metrics, tracing, i18n, accessibility, security headers,
   secrets handling — what exists and what's missing.
7. **Run it** (if agreed). Build and test in the project's containers
   (`devcontainer:workflow`, `../../references/containers.md`); record pass,
   fail and skip per suite. Start the app and fetch its generated API
   description if it has one. Every result becomes `OBSERVED` evidence.
8. **Confirm.** One round with the user on the Inferred and Assumed claims
   that change the picture most (the product's purpose, audiences, why an
   odd rule exists, which integrations are live). Update the labels.

## Outputs

Only the missing artifacts, each with `Status: Draft` and a first line
`Reconstructed by project:introspec on <date> from <commit>`:

- `docs/product/brief.md` — the `project:init` structure, from observed
  capabilities; purpose, audiences and goals are Assumed until confirmed.
  The glossary uses the code's own names and lists the synonyms found
  (`Order` in the API, `pedido` in the tables).
- `docs/product/architecture.md` — `project:architecture` review mode's
  **as-is** sections (topology, hosting, stack, data stores, integrations,
  capacity where metrics exist); ADRs only for decisions the history or
  docs explain, labelled accordingly. No to-be: that is
  `project:architecture` or `project:refactor`.
- `docs/product/domain-model.md` — contexts, aggregates, invariants and
  lifecycles as implemented, in `backend:domain`'s structure.
- `specs/README.md` and `specs/NNN-<feature>/` — one feature per coherent
  capability. Bootstrap Spec Kit as `project:spec` does; write `spec.md`
  (stories and acceptance scenarios from behavior and tests), `plan.md`
  (as-is technical context), `data-model.md` and `contracts/`. No
  `tasks.md` — nothing is planned yet. Statuses from evidence only:
  **Implemented** when the code path exists and runs (note "no tests"
  where true); **Verified** only when end-to-end tests cover the stories
  and pass; **Partial** is written as Implemented plus the missing
  stories listed as Planned.
- `contracts/openapi.yaml` — rebuilt from the real routes (or the app's
  generated description, reconciled with the code), with
  `info.x-status: reconstructed`; `backend:spec` makes it canonical.
- `docs/product/introspec.md` — the evidence report: scope and
  boundaries, inventory, what ran and its results, drift against
  existing docs, dead code and unreachable features, contradictions
  (code vs. tests vs. docs), risks spotted (security, data, operability),
  and every open Inferred/Assumed item.

Never copy a secret, a connection string, a real email or any data row
into an artifact; name the variable instead.

## Coverage checklist

- [ ] every entry point (route, job, consumer, CLI, screen) is in a feature or listed as dead/internal
- [ ] every table and collection is in the domain model or marked infrastructure-only
- [ ] every integration has a label and the config keys that select it
- [ ] every status field has a state machine; every authorization check is in the matrix
- [ ] every feature status is backed by evidence; nothing is Verified without passing e2e tests
- [ ] SBOM generated; stack and versions in `architecture.md`
- [ ] drift against existing docs reported, not silently fixed
- [ ] no secrets, connection strings or data rows in any artifact
- [ ] the confirmation round ran; remaining Inferred/Assumed items listed in `introspec.md`

## Handoff

What was reconstructed, the confidence mix (how much is Observed), the
riskiest findings, and the next stage: `project:retrofit` to upgrade in
place, `project:refactor` for a redesign, `project:architecture` (review
mode) for a to-be, or `project:docs`. Commits follow `git:workflow`.
