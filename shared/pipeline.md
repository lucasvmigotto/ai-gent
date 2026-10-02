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
        │                 specify → clarify → plan → tasks (implement: the *:build stages)
        │
        ├── frontend:uiux ──► docs/product/ux-vision.md  layout + language
        │   frontend:spec ──► specs/NNN-*/ui.md          UI layer, added to the tasks step
        │   frontend:build ─► the frontend code           ← the implement step
        │   frontend:tui ───► a requested TUI / CLI (from tui.md)
        │
        ├── backend:domain ─► docs/product/domain-model.md
        │   backend:spec ───► specs/NNN-*/backend.md     backend layer, added to the tasks step
        │                     contracts/openapi.yaml     the canonical contract
        │   backend:build ──► the backend code            ← the implement step
        │
        └── qa:strategy ────► docs/product/test-strategy.md, specs/NNN-*/qa.md
            qa:e2e, qa:load ► cross-stack journeys, load tests   (after the builds)

 devsecops:pipeline, devsecops:supply-chain ─► CI/CD config     from project:spec on
 devsecops:iac ────────► infra/                            cloud environments, once hosting is decided
 project:docs ─────────► docs/site/                        reads all of the above
 project:status ───────► (report only)                     where things stand, what to run next
 project:recap ────────► (report only)                     where the last session stopped, what changed since
 db:inspect, db:review, db:investigate ─► docs/product/db/  any time a database is involved (the `db` plugin)
```

For an existing codebase, the chain starts earlier:

```
existing code, schema, config, tests
        │
 project:survey ───────► (report only)                        first contact: stack, history, branches, red flags
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
`devsecops:migrate` run whenever needed. `project:status` reads everything
and writes nothing. So do `project:survey`, run before `project:introspec`
on a repository nobody has explained yet, and `project:recap`, run on any
project after a pause or a stalled session; the only thing either changes
is the remote-tracking refs, through `git fetch`.

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
| `specs/NNN-<feature>/tui.md` | `frontend:spec`, only when the user asked for a terminal interface | frontend:tui, qa:e2e | screens in cells, keymap, focus, states, CLI equivalents |
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
- **A stage ticks only its own tasks.** The build stages flip `- [ ]` to
  `- [x]` inside their own section only, and `/speckit-converge`'s
  appended phase carries the layer in its heading so it has an owner too.
  Never tick a task another stage owns, and never run a whole-file
  implementation pass over a layered `tasks.md`.

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
   - *Implemented* — a `*:build` stage passed its last checkpoint **and
     `/speckit-converge` reported the feature converged**;
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
7. **Versioning** follows `git:workflow`: a branch per stage or phase,
   started from `dev`/`develop` and merged back into it; small
   Conventional Commits, made as each task or phase finishes without
   stopping to ask (a stage the user started is an approved development
   workflow) and listed in the handoff; ask before merging. Promoting
   `dev` up the chain (`homolog`/`staging`, then `main`/`master`) needs
   its own explicit request.
8. **Finish with a handoff line**: what was written, what's still open,
   and the next stage to run.

## The Spec Kit flow: specify → clarify → plan → tasks → implement

Every feature goes through these five steps, in this order. Each has a
gate: **the step does not start until the previous one has cleared it.**

```
  specify ──▶ clarify ──▶ plan ──▶ tasks ──▶ implement
   spec.md     spec.md     plan.md   tasks.md     code, - [x]
               markers     research layer-neutral
               resolved data-model + one pointer
               quickstart task per layer
               contracts/
                                           │
                                           ▼
                                       converge ── appends
                                       Phase N ──▶ back to implement
```

| Step | Performed by | Gate to enter | Writes |
|---|---|---|---|
| bootstrap, constitution | `project:spec` | `.specify/` absent | `.specify/`, `constitution.md` |
| **specify** | `project:spec` | brief and architecture exist | `spec.md` (incl. `Feature Branch`) |
| **clarify** | `project:spec` | `spec.md` written | answers written back into `spec.md` |
| **plan** | `project:spec` | **zero open `[NEEDS CLARIFICATION]`** in `spec.md` | `plan.md`, `research.md`, `data-model.md`, `quickstart.md`, `contracts/` |
| **tasks**, layer-neutral | `project:spec` | `plan.md` + Constitution Check pass | core phases + one pointer task per layer |
| **tasks**, Frontend / Backend / QA layers | `frontend:spec`, `backend:spec`, `qa:strategy` | the `tasks` step ran | `ui.md`, `backend.md`, `qa.md` + the matching `## Frontend` / `## Backend` / `## QA` section |
| **implement** | **`frontend:build`, `backend:build`** (`qa:e2e`/`qa:load` for `## QA`, `frontend:tui` for `[TUI]`/`[CLI]`) | the layer's section exists in `tasks.md` | the code; `- [ ]` → `- [x]` in **its own section only** |
| **converge** | end of each build skill, before Implemented | the feature's last checkpoint passed | a `## Phase N: Convergence` phase, or "converged" |

Notes:

- **clarify is a step, not a footnote.** It resolves every
  `[NEEDS CLARIFICATION]` with the user — one question at a time, at most
  five per feature — and writes the answers into `spec.md`. A feature with
  an open marker is *not* planned; `project:status` reports it and
  recommends resolving it first.
- **The `tasks` step runs twice.** `project:spec` writes the layer-neutral
  phases and one pointer task per layer; each `*:spec` stage appends its
  own section. That is why nothing is planned twice.
- **implement is the build stages, not the bare command.**
  `/speckit-implement`'s contract — load the context, go phase by phase,
  tests before code, respect `[P]`, tick `- [x]` as each task lands, halt
  on failure, validate at the end — is what `frontend:build` and
  `backend:build` execute, each against **its own section** of
  `tasks.md`. The command itself is never run on a feature with more than
  one layer section, because it has no scoping flag and would tick another
  stage's tasks (ownership rule 1). Where a feature has a single layer —
  a frontend-only project, or no backend in scope — running
  `/speckit-implement` directly is fine.
- **converge closes the loop.** After its last checkpoint, a build skill
  runs `/speckit-converge`: it assesses the code against `spec.md`,
  `plan.md` and `tasks.md` and appends whatever is still unbuilt as a new
  phase, which the same skill then implements. Only "converged" makes the
  feature Implemented.
- **Halting is not implementing.** Stopping mid-phase — a failing test, a
  missing decision — leaves the feature *In progress* and says what is
  blocked. No status moves forward on a halt.
- `/speckit-analyze` and `/speckit-checklist` are cross-cutting, not
  steps: `project:spec` runs analyze once per feature at the end of the
  flow, each `*:spec` stage runs it again after adding its section.

### Where the CLI and this pipeline disagree

Spec Kit's own command files drift from its templates and from this
pipeline. The pipeline wins; read the installed templates, not the
command's prose.

| Upstream says | This pipeline |
|---|---|
| `implement.md` parses phases as "Setup, Tests, Core, Integration, Polish" | the installed `tasks-template.md` — and every task list here — uses **Setup → Foundational → one phase per story → Polish**. Read the template. |
| the tasks template says "Tests are OPTIONAL — only include them if explicitly requested" | tests are **required** for every story (constitution principle II). `project:spec` says so in the spec so the tasks step generates them. |
| `implement.md` ticks every `- [ ]` in `tasks.md` | each stage ticks only its own section. |
| `implement.md` step 4 creates ignore files (`.gitignore`, `.dockerignore`, …) | adopted, but only in the Setup phase and only inside the repository. |
| `converge` appends `## Phase N: Convergence` | in a layered file the heading carries the layer — `## Phase N: Convergence (Frontend)` — so the phase has an owner. Task IDs stay sequential and are never renumbered. |

## Spec Kit

The chain uses GitHub Spec Kit (`specify` CLI, 1.x) for everything under
`specs/`. Don't reimplement its templates — they change between versions:

- Bootstrap (once per project, by `project:spec`):
  `specify init --here --force --integration claude --non-interactive
  --ignore-agent-tools` (the agent running it is the integration, so
  the CLI's check for a `claude` binary on PATH is skipped).
  `--force` is required: the directory is never empty by then (`.git`,
  `docs/product/`), and without it the CLI stops with "Current directory
  is not empty". It merges, adding only `.specify/` and
  `.claude/skills/speckit-*`. Do **not** add `--extension git`: branches
  follow `git:workflow`, not Spec Kit's numbered-branch hook.
- This installs `/speckit-*` skills into the project's `.claude/skills/`
  (`constitution`, `specify`, `clarify`, `plan`, `tasks`, `analyze`,
  `checklist`, `implement`, `converge`, `taskstoissues`). Use them for the
  steps they cover — `implement` and `converge` being the flow's last step
  and the loop back into it, both executed by the build stages as described
  above. `taskstoissues` creates issues on the remote: run it only when the
  user asks for that, like any other remote change (`git:workflow`). If
  they aren't loaded in the current session, read the corresponding
  `.claude/skills/speckit-<step>/SKILL.md` and follow it directly.
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
