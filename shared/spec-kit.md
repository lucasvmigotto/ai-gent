# The Spec Kit contract

The Spec Kit half of the pipeline contract: the five-step feature flow,
its gates, who owns each step, and where the `specify` CLI and this
pipeline disagree. Read `pipeline.md` first — it holds the chain, the
artifact owners and the rules every stage follows; this file holds the
steps.

Stages that act on `specs/` read this. Stages that don't — delivery,
IaC, docs, refactoring, retrofitting — need `pipeline.md` only.

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
  `tasks.md`. The command itself is never run on a feature with more
  than one layer section, because it has no scoping flag and would tick
  another stage's tasks (ownership rule 1 in `pipeline.md`). Where a
  feature has a single layer — a frontend-only project, or no backend in
  scope — running `/speckit-implement` directly is fine.
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
