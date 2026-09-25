# Product pipeline contract

Shared by the `project`, `frontend`, `backend`, `qa` and `devsecops`
plugins. Every skill in the chain reads this file first. It defines where
each artifact lives, who owns it, and the rules every stage follows — so
any stage can run alone, in a later session, or out of order, and still
find what the others wrote.

## The chain

```
idea text / references dir
        │
 project:init ─────────► docs/product/brief.md           what, for whom, why; the glossary
        │
 project:architecture ─► docs/product/architecture.md    topology, hosting, API style, data, capacity
        │                 docs/product/adr/NNNN-*.md       one decision record per choice
        │
 project:spec ─────────► .specify/memory/constitution.md
        │                 specs/NNN-<feature>/…            every feature, Spec Kit format
        │                 contracts/openapi.yaml           API contract skeleton
        │
        ├── frontend:uiux ──► docs/product/ux-vision.md  layout + language
        │   frontend:spec ──► specs/NNN-*/ui.md          UI layer on existing features
        │   frontend:build ─► the frontend code
        │
        ├── backend:domain ─► docs/product/domain-model.md
        │   backend:spec ───► specs/NNN-*/backend.md     backend layer on existing features
        │                     contracts/openapi.yaml     the canonical contract
        │   backend:build ──► the backend code
        │
        └── qa:strategy ────► docs/product/test-strategy.md, specs/NNN-*/qa.md
            qa:e2e, qa:load ► cross-stack journeys, load tests   (after the builds)

 devsecops:pipeline, devsecops:supply-chain ─► CI/CD config     from project:spec on
 devsecops:iac ────────► infra/                            cloud environments, once hosting is decided
 project:docs ─────────► docs/site/                        reads all of the above
 project:status ───────► (report only)                     where things stand, what to run next
 db:inspect, db:review, db:investigate ─► docs/product/db/  any time a database is involved (the `db` plugin)
```

For an existing codebase, the chain starts one step earlier:

```
existing code, schema, config, tests
        │
 project:introspec ────► the artifacts above, reconstructed    evidence-labelled; only those missing
        │                 docs/product/introspec.md           evidence report, drift, open items
        │                 docs/product/sbom.cdx.json          dependency inventory
        ├── project:retrofit ─► docs/product/retrofit.md    upgrades and CVE fixes, behavior frozen
        └── project:refactor ─► docs/product/refactor.md    target design, incremental slices
                                docs/product/bcr/NNNN-*.md  business changes, each approved by the user
```

After that, the normal chain applies: `project:architecture` (review
mode), `project:spec` for changed features, and the build and QA stages.

The frontend, backend and QA branches run in parallel once `project:spec`
has produced features and a contract skeleton. Frontend and backend meet
only at the contract and the glossary. `devsecops:audit` and
`devsecops:migrate` run whenever needed; `project:status` reads everything
and writes nothing.

## Artifacts and owners

| Path | Owner (writes) | Readers | Notes |
|---|---|---|---|
| `docs/product/brief.md` | `project:init` | everyone | vision, audiences, scope, capabilities, constraints, **glossary** |
| `docs/product/architecture.md`, `docs/product/adr/` | `project:architecture` | project:spec, backend:domain, devcontainer:setup, devsecops:*, qa:load, project:docs | drivers, capacity model, topology, hosting, API style, data stores, decisions with alternatives |
| `docs/product/ux-vision.md` | `frontend:uiux` | frontend:*, project:docs | layout, visual direction, voice, microcopy |
| `docs/product/domain-model.md` | `backend:domain` | backend:*, project:docs; project:architecture and project:spec only if it already exists | contexts, aggregates, invariants, events, lifecycles; runs after `project:spec`, so earlier stages never wait for it |
| `docs/product/references/` | the user | init, uiux, domain | screenshots, competitor notes, sketches, existing docs |
| `.specify/` | Spec Kit (`specify init`) | project:spec | templates, scripts, constitution |
| `.specify/memory/constitution.md` | `project:spec` | every spec/build stage | non-negotiable engineering principles |
| `specs/README.md` | `project:spec` (build and QA stages update only the status columns) | everyone | feature index: number, name, priority, dependencies, frontend/backend status (Planned / In progress / Implemented / Verified) |
| `specs/NNN-<feature>/spec.md`, `plan.md`, `research.md`, `data-model.md`, `quickstart.md`, `tasks.md`, `contracts/`, `checklists/` | `project:spec` (via Spec Kit) | everyone downstream | feature split, stories, requirements, stack |
| `.specify/feature.json` | Spec Kit's scripts | Spec Kit's scripts | the "current feature" pointer, rewritten by every `/speckit-specify`; never rely on it — see *Spec Kit* |
| `specs/NNN-<feature>/ui.md` | `frontend:spec` | frontend:build | screens, components, states, tokens used |
| `specs/NNN-<feature>/backend.md` | `backend:spec` | backend:build | endpoints, authz, errors, persistence, jobs |
| `specs/000-design-system/` | `frontend:spec` | frontend:build | the one feature `frontend:spec` may create itself |
| `contracts/openapi.yaml` (+ `asyncapi.yaml` if events leave the service) | skeleton by `project:spec`, canonical by `backend:spec` | frontend:*, backend:* | the frontend/backend seam |
| `docs/product/test-strategy.md` | `qa:strategy` | build stages, qa:*, devsecops:pipeline | test layers, risks, environments, test data, gates |
| `specs/NNN-<feature>/qa.md` | `qa:strategy` | build stages, qa:e2e, qa:load | story → test map, e2e journeys, load profiles, exit criteria |
| `tests/e2e/`, `tests/load/` (or the paths the plan sets) | `qa:e2e`, `qa:load` | devsecops:pipeline | cross-stack suites; unit/component/integration tests stay with the build stages |
| CI/CD config (`.github/workflows/` by default, or the platform's file) | `devsecops:pipeline` | everyone | pipelines, gates, environments; security tooling from `devsecops:supply-chain` |
| `infra/` | `devsecops:iac` | devsecops:pipeline | infrastructure as code: modules, one root per environment, state bootstrap |
| `docs/product/delivery.md` | `devsecops:pipeline` (supply-chain section by `devsecops:supply-chain`, infrastructure section by `devsecops:iac`) | everyone, project:docs | pipelines, gates, environments, promotion, required secrets by name, rollback runbook |
| `docs/site/` | `project:docs` | — | static documentation site |
| `docs/product/introspec.md`, `docs/product/sbom.cdx.json` | `project:introspec` | project:retrofit, project:refactor, project:status | evidence report (inventory, runs, drift, contradictions, open Inferred/Assumed items); SBOM |
| `docs/product/retrofit.md` | `project:retrofit` | project:refactor, devsecops:*, project:docs | level, baseline, CVE and EOL tables, ordered steps, before/after |
| `docs/product/db/<database>/inspection.md`, `schema.sql`, `diff-*.md` | `db:inspect` | project:introspec, db:review, backend:*, devcontainer:infra | schema discovery, labelled like introspec; a production schema export stays outside the repository unless the user approves |
| `docs/product/db/review-<date>.md` | `db:review` | backend:build, project:architecture, qa:review | findings on the application's database usage; fixes only as migrations or code |
| `docs/product/db/investigations/<date>-<slug>.md` | `db:investigate` | backend:build, qa:review, the person running the repair script | data problems: hypotheses, evidence, timeline, blast radius, root cause, repair script |
| `docs/product/refactor.md`, `docs/product/bcr/NNNN-*.md` | `project:refactor` | project:spec, backend:*, frontend:*, qa:* | rule classification (core / policy / accidental), findings, target, slices; business change records (proposed / accepted / rejected) |

**Ownership rules**

- Only the owner creates or restructures an artifact. Downstream stages
  *add their layer* (a `ui.md`, a `backend.md`, frontend/backend tasks
  appended to `tasks.md`, a `qa.md`) and never rewrite another stage's sections.
- If a downstream stage finds the upstream artifact wrong or missing
  something, it records the gap as an `[UPSTREAM GAP: …]` note in its own
  artifact, tells the user, and proposes the upstream fix — it doesn't
  silently patch it.
- **Reconstructed artifacts.** On an existing codebase, `project:introspec`
  may create any artifact above that is missing — brief, architecture
  (as-is only), domain model, specs, contract (`info.x-status:
  reconstructed`) — marked `Reconstructed by project:introspec` and
  evidence-labelled. The owner takes it over on its next run. When an
  artifact already exists, introspec reports drift instead of rewriting it.
- `tasks.md` stays one file per feature. Frontend tasks go under
  `## Frontend`, backend tasks under `## Backend` and QA tasks under
  `## QA` headings appended by their stages, using Spec Kit's task format
  (`- [ ] T0NN [P] [USn] Description with file path`), numbering after the
  last existing ID.

## The glossary is shared vocabulary

`brief.md` has a `## Glossary` section: one entry per domain term, with a
definition and the words **not** to use for it
(`Booking — a confirmed reservation of a slot by a member. Not: reservation, appointment.`).

- UI copy (`ux-vision.md`), domain model names, API resource names,
  database tables, and docs all use the glossary term.
- A stage that needs a new term adds it to the glossary **first** (the
  one exception to ownership — append only, never rename an existing
  entry without the user's agreement) and then uses it.
- A mismatch (UI says "Booking", API says `/reservations`) is a bug in
  whichever artifact diverged.

## The contract is the frontend/backend seam

- `contracts/openapi.yaml` (OpenAPI 3.1) is the single source both sides
  build against. Per-feature `specs/NNN/contracts/` hold only that
  feature's operations and must match it.
- `frontend:build` develops against a mock generated from it
  (`stoplight/prism mock contracts/openapi.yaml`, see `devcontainer:infra`
  tier 3), so it never waits on the backend.
- `backend:build` runs contract tests against it; CI fails on drift.
- Changing the contract is a spec change: update it through `backend:spec`
  (or record a gap), never by editing it from a build stage to make code
  pass.

## Rules every stage follows

1. **Read before writing.** Load this file, then every upstream artifact
   that exists. Run standalone only when they don't — and then say which
   inputs were missing and what you assumed instead.
2. **Clarify first, briefly.** Ask 3–5 questions that would change the
   output (audience, scope boundary, a hard constraint), in one round.
   Don't ask what the inputs already answer. Inside a Spec Kit step,
   `/speckit-clarify`'s own flow applies instead (one question at a time,
   at most five per feature).
3. **Never invent silently.** Mark every assumption `[ASSUMPTION: …]` and
   every open question `[NEEDS CLARIFICATION: …]` (Spec Kit's marker)
   inline. A reader must be able to tell a decision from a guess.
   Reconstructed claims also carry `[OBSERVED: path:line]` or
   `[INFERRED: signal → conclusion]` (see `project:introspec`).
4. **Comprehensive means complete coverage, not length.** Each skill ends
   with a coverage checklist; the artifact is done when every item is
   addressed or explicitly marked N/A with a reason. Cut filler — a
   section that says nothing specific to *this* product gets deleted.
5. **Real content.** Use the product's actual names, entities and
   scenarios throughout — no "Lorem ipsum", no "User A does Action B".
6. **Status is explicit**, per feature and side, in `specs/README.md`:
   - *Planned* — specified, no code yet (everything before a build stage);
   - *In progress* — a build stage is working through its phases;
   - *Implemented* — a `*:build` stage passed its last checkpoint;
   - *Verified* — `qa:e2e` (and `qa:load` where the feature has load
     targets) passed against the implemented feature, and CI runs those
     suites.
   Only the named stage moves a status forward; any stage moves it back
   when it finds the claim no longer holds. The one exception is
   `project:introspec` on existing code, which sets Implemented or
   Verified from evidence (code that runs; passing e2e tests). `project:docs` depends on this.
   These statuses are for features only. Documents (`brief.md`,
   `architecture.md`, `ux-vision.md`, `domain-model.md`) carry `Status:
   Draft` until the user accepts them, then `Accepted`; ADRs use MADR's
   `proposed` / `accepted` / `superseded`; Spec Kit's own `Status: Draft`
   in `spec.md` stays as Spec Kit writes it.
7. **Versioning** follows `git:workflow` (branch per stage/phase, small
   Conventional Commits, ask before committing and merging).
8. **Finish with a handoff line**: what was written, what's still open,
   and the next stage to run.

## Spec Kit

The chain uses GitHub Spec Kit (`specify` CLI, 1.x) for everything under
`specs/`. Don't reimplement its templates — they change between versions:

- Bootstrap (once per project, by `project:spec`):
  `specify init --here --force --integration claude --non-interactive`.
  `--force` is required: the directory is never empty by then (`.git`,
  `docs/product/`), and without it the CLI stops with "Current directory
  is not empty". It merges, adding only `.specify/` and
  `.claude/skills/speckit-*`. Do **not** add `--extension git`: branches
  follow `git:workflow`, not Spec Kit's numbered-branch hook.
- This installs `/speckit-*` skills into the project's `.claude/skills/`
  (`constitution`, `specify`, `clarify`, `plan`, `tasks`, `analyze`,
  `checklist`, `implement`, `converge`, `taskstoissues`). Use them for the
  steps they cover. `taskstoissues` creates issues on the remote: run it
  only when the user asks for that, like any other remote change
  (`git:workflow`). If they aren't loaded in the current session, read the
  corresponding `.claude/skills/speckit-<step>/SKILL.md` and follow it
  directly.
- Templates live in `.specify/templates/`; read the installed ones rather
  than assuming a structure from memory.
- **Pick the feature explicitly.** Spec Kit's scripts act on
  `SPECIFY_FEATURE` if set, otherwise on `.specify/feature.json` — the
  feature specified last. Before any speckit step on an existing feature,
  export `SPECIFY_FEATURE=NNN-<feature>`, so stages running in parallel
  never work on each other's feature.
- The spec template's `Feature Branch` field gets the `git:workflow`
  branch the feature is worked on (`feat/resident-access`), not a
  numbered Spec Kit branch.
