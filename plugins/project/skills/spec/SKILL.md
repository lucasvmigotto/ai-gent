---
name: spec
description: Turn docs/product/brief.md into a phased GitHub Spec Kit specification — bootstraps Spec Kit, writes the constitution, splits the product into specs/NNN-* features taken through specify, clarify, plan and tasks, and adds specs/README.md and the contracts/openapi.yaml skeleton. Use for "create the speckit specs", "break the project into features".
---

# Phased specification (Spec Kit)

## Role

Act as a senior full-stack engineer with deep expertise across frontend,
backend, APIs, databases, architecture, security, performance, testing
and maintainable software development. You own the feature split, the
technical plan and the contract skeleton — the decisions every later
stage builds on — so make them explicit, justified and consistent.

## Before starting

1. Read `../../references/pipeline.md` — especially the Spec Kit and
   ownership sections.
2. Required input: `docs/product/brief.md`. If missing, propose
   `project:init`; if the user wants to skip it, run its clarifying
   questions yourself and record the answers as the feature inputs.
3. Read `docs/product/architecture.md` and `docs/product/adr/` — the
   stack, topology, API style and data stores are decided there; plans
   follow them and never silently diverge. If missing, propose
   `project:architecture` first.
4. Also read, when present: `docs/product/domain-model.md` (it normally
   comes later, from `backend:domain` — don't wait for it),
   `docs/product/ux-vision.md`, existing `specs/`, existing code.
   Existing features are **updated**, never duplicated.

## Process

### 1. Bootstrap Spec Kit

If `.specify/` doesn't exist, check that no `.claude/skills/speckit-*`
directories exist yet (if they do, ask before overwriting), then run
`specify init --here --force --integration claude --non-interactive`.
`--force` is required because the repository isn't empty; no
`--extension git`, since branches follow `git:workflow`. Confirm with
`specify version` that the CLI is 1.x, and read the installed templates
in `.specify/templates/`. If `specify` isn't installed, stop and tell the
user (`uv tool install specify-cli --from git+https://github.com/github/spec-kit.git`).

### 2. Constitution

Run `/speckit-constitution` (or follow its skill file) to write
`.specify/memory/constitution.md` from:

- the brief's NFRs (performance, security, accessibility, privacy
  regime, supported platforms) turned into testable principles;
- the user's standing practices: `git:workflow` (Conventional Commits,
  branch per context), tests required for every user story, WCAG 2.2 AA,
  no secrets in the repo, contract-first API (`contracts/openapi.yaml` is
  the source of truth), separate frontend/backend modules and
  devcontainers when both exist;
- the decisions in `docs/product/architecture.md` and its ADRs.

Keep principles few and enforceable — each one should be checkable in a
plan's Constitution Check.

### 3. Feature split — `specs/README.md`

From the brief's candidate feature map, decide the final features:

- each **independently shippable and testable** (Spec Kit's user-story
  independence applies at feature level too);
- ordered by dependency, MVP first; numbering follows that order
  (`001-…`). `000-design-system` is reserved for `frontend:spec`;
- cross-cutting foundations (auth, tenancy, audit) become their own early
  feature rather than being smeared across others.

Write `specs/README.md`: number, name, one-line scope, priority,
depends on, frontend status, backend status (all `Planned`).

### 4. Each feature, in order

For each feature, using the Spec Kit skills (or their skill files), with
`SPECIFY_FEATURE=NNN-<feature>` exported once the feature exists (see the
pipeline's *Spec Kit* section):

1. `/speckit-specify` — prioritized user stories (P1…) with independent
   tests and Given/When/Then scenarios, edge cases, `FR-###`
   requirements, key entities, `SC-###` measurable success criteria,
   assumptions. Use glossary terms only. State in the spec that tests
   are required (the constitution demands them), so `/speckit-tasks`
   generates test tasks. Fill `Feature Branch` with the `git:workflow`
   branch name.
2. `/speckit-clarify` — resolve `[NEEDS CLARIFICATION]` markers with the
   user, following its flow (one question at a time, up to five per
   feature); record answers in the spec.
3. `/speckit-plan` — technical context, constitution check, project
   structure, `research.md`, `data-model.md`, `contracts/`,
   `quickstart.md`. Decisions:
   - **Stack**: from `architecture.md`'s Stack decision. Only where it
     leaves a choice open — then record it as an `[UPSTREAM GAP]` for
     `project:architecture` — ask the user, suggesting their usual web
     client stack (Bun · React · TypeScript · Vite · Tailwind CSS ·
     Biome).
   - **Structure**: Spec Kit's "web application" option (`backend/` +
     `frontend/`) whenever there's a separate client — matching
     `devcontainer:setup`'s split.
   - **Dev environment**: note that `devcontainer:setup` (and
     `devcontainer:infra` for each external resource) provides it.
   - **Delivery**: CI/CD comes from `devsecops:pipeline`; the test layers
     from `qa:strategy`.
4. `/speckit-tasks` — Spec Kit's phases as it generates them (Setup →
   Foundational → one phase per user story → Polish), with checkpoints.
   Keep each story's phase at story level: its acceptance-test tasks and
   the layer-neutral work, plus one task per affected layer that points
   to the layer's section (`- [ ] T014 [US1] Backend implementation —
   see ## Backend`). `frontend:spec` and `backend:spec` append those
   `## Frontend` / `## Backend` sections with the detailed tasks, so
   nothing is planned twice.

### 5. Contract skeleton — `contracts/openapi.yaml`

OpenAPI 3.1, assembled from every feature's `contracts/`: one tag per
feature, resources named with glossary terms, every operation with an
`operationId`, summary, auth requirement, and request/response schema
outlines. Mark it `x-status: skeleton` in the `info` object (`info.x-status`) —
`backend:spec` makes it canonical. Add `asyncapi.yaml` only if events leave the service.

### 6. Consistency pass

`/speckit-analyze` works on one feature and is read-only: run it once per
feature, with `SPECIFY_FEATURE` set to that feature. Fix what's in your
artifacts and report upstream gaps (brief, architecture) to the user.
Requirements whose only tasks are the layer pointers are expected at this
stage; analyze again after `frontend:spec` and `backend:spec` have added
their sections. Optionally `/speckit-checklist` for requirement quality
on the MVP features.

## Coverage checklist

- [ ] every MVP capability of the brief is covered by a feature
- [ ] every feature has spec, plan, tasks, and no open `[NEEDS CLARIFICATION]`
- [ ] every plan passes its Constitution Check (or justifies violations in Complexity Tracking)
- [ ] every user story has an independent test and success criteria
- [ ] every data-touching story has entities in `data-model.md`
- [ ] every client–server interaction has an `operationId` in the contract skeleton
- [ ] only glossary terms used in specs and contract
- [ ] `specs/README.md` lists all features with dependencies
- [ ] `/speckit-analyze` run per feature; findings fixed or reported

## Handoff

Summarize features (MVP first), key technical decisions, open risks, and
the next stages — in parallel: `frontend:uiux` → `frontend:spec` →
`frontend:build`, `backend:domain` → `backend:spec` → `backend:build`, and
`qa:strategy`; plus `devcontainer:setup` for the environment and
`devsecops:pipeline` for CI/CD. Commits
follow `git:workflow`.
