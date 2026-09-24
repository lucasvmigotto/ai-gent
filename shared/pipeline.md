# Product pipeline contract

Shared by the `project`, `frontend` and `backend` plugins. Every skill in
the chain reads this file first. It defines where each artifact lives, who
owns it, and the rules every stage follows — so any stage can run alone,
in a later session, or out of order, and still find what the others wrote.

## The chain

```
idea text / references dir
        │
 project:init ───────► docs/product/brief.md            what, for whom, why; the glossary
        │
 project:spec ───────► .specify/memory/constitution.md
        │               specs/NNN-<feature>/…            every feature, Spec Kit format
        │               contracts/openapi.yaml           API contract skeleton
        │
        ├── frontend:uiux ──► docs/product/ux-vision.md  layout + language
        │   frontend:spec ──► specs/NNN-*/ui.md          UI layer on existing features
        │   frontend:build ─► the frontend code
        │
        └── backend:domain ─► docs/product/domain-model.md
            backend:spec ───► specs/NNN-*/backend.md     backend layer on existing features
                              contracts/openapi.yaml     the canonical contract
            backend:build ──► the backend code

 project:docs ──────► docs/site/                         reads all of the above
```

The frontend and backend branches run in parallel once `project:spec` has
produced features and a contract skeleton. They meet only at the contract
and the glossary.

## Artifacts and owners

| Path | Owner (writes) | Readers | Notes |
|---|---|---|---|
| `docs/product/brief.md` | `project:init` | everyone | vision, audiences, scope, capabilities, constraints, **glossary** |
| `docs/product/ux-vision.md` | `frontend:uiux` | frontend:*, project:docs | layout, visual direction, voice, microcopy |
| `docs/product/domain-model.md` | `backend:domain` | backend:*, project:spec, project:docs | contexts, aggregates, invariants, events, lifecycles |
| `docs/product/references/` | the user | init, uiux, domain | screenshots, competitor notes, sketches, existing docs |
| `.specify/` | Spec Kit (`specify init`) | project:spec | templates, scripts, constitution |
| `.specify/memory/constitution.md` | `project:spec` | every spec/build stage | non-negotiable engineering principles |
| `specs/README.md` | `project:spec` (build stages update only the Status column) | everyone | feature index: number, name, priority, dependencies, frontend/backend status (Planned / In progress / Implemented) |
| `specs/NNN-<feature>/spec.md`, `plan.md`, `research.md`, `data-model.md`, `quickstart.md`, `tasks.md`, `contracts/` | `project:spec` (via Spec Kit) | everyone downstream | feature split, stories, requirements, stack |
| `specs/NNN-<feature>/ui.md` | `frontend:spec` | frontend:build | screens, components, states, tokens used |
| `specs/NNN-<feature>/backend.md` | `backend:spec` | backend:build | endpoints, authz, errors, persistence, jobs |
| `specs/000-design-system/` | `frontend:spec` | frontend:build | the one feature `frontend:spec` may create itself |
| `contracts/openapi.yaml` (+ `asyncapi.yaml` if events leave the service) | skeleton by `project:spec`, canonical by `backend:spec` | frontend:*, backend:* | the frontend/backend seam |
| `docs/site/` | `project:docs` | — | static documentation site |

**Ownership rules**

- Only the owner creates or restructures an artifact. Downstream stages
  *add their layer* (a `ui.md`, a `backend.md`, frontend/backend tasks
  appended to `tasks.md`) and never rewrite another stage's sections.
- If a downstream stage finds the upstream artifact wrong or missing
  something, it records the gap as an `[UPSTREAM GAP: …]` note in its own
  artifact, tells the user, and proposes the upstream fix — it doesn't
  silently patch it.
- `tasks.md` stays one file per feature. Frontend tasks go under
  `## Frontend` and backend tasks under `## Backend` headings appended by
  their spec stages, using Spec Kit's task format
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
   Don't ask what the inputs already answer.
3. **Never invent silently.** Mark every assumption `[ASSUMPTION: …]` and
   every open question `[NEEDS CLARIFICATION: …]` (Spec Kit's marker)
   inline. A reader must be able to tell a decision from a guess.
4. **Comprehensive means complete coverage, not length.** Each skill ends
   with a coverage checklist; the artifact is done when every item is
   addressed or explicitly marked N/A with a reason. Cut filler — a
   section that says nothing specific to *this* product gets deleted.
5. **Real content.** Use the product's actual names, entities and
   scenarios throughout — no "Lorem ipsum", no "User A does Action B".
6. **Status is explicit.** Everything produced before code exists is
   *Planned*. Only `*:build` stages turn something *Implemented*, and only
   once it's verified. `project:docs` depends on this.
7. **Versioning** follows `git-workflow` (branch per stage/phase, small
   Conventional Commits, ask before committing and merging).
8. **Finish with a handoff line**: what was written, what's still open,
   and the next stage to run.

## Spec Kit

The chain uses GitHub Spec Kit (`specify` CLI, 1.x) for everything under
`specs/`. Don't reimplement its templates — they change between versions:

- Bootstrap (once per project, by `project:spec`):
  `specify init --here --integration claude --non-interactive`.
  Do **not** add `--extension git`: branches follow `git-workflow`, not
  Spec Kit's numbered-branch hook.
- This installs `/speckit-*` skills into the project's `.claude/skills/`
  (`constitution`, `specify`, `clarify`, `plan`, `tasks`, `analyze`,
  `checklist`, `implement`, `converge`). Use them for the steps they
  cover. If they aren't loaded in the current session, read the
  corresponding `.claude/skills/speckit-<step>/SKILL.md` and follow it
  directly.
- Templates live in `.specify/templates/`; read the installed ones rather
  than assuming a structure from memory.
