---
name: status
description: Report where a project stands in the product pipeline — which artifacts exist, each feature's frontend, backend and QA status, open gaps, assumptions and clarifications, status claims the repo contradicts — and recommend the next stage to run. Read-only. Use for "where are we", "what's next", "project status", "which stage should I run", or at the start of a session on a pipeline project.
---

# Pipeline status

A read-only snapshot of a project that uses the product pipeline, ending
in a recommendation of what to run next. It never writes or fixes
anything: every problem it finds goes into the report, with the stage that
owns the fix.

Read `../../references/pipeline.md` first — the artifact table and the
status rules there are what this skill checks against.

## 1. Artifacts

Check each path in the pipeline's artifact table and report it as
present, missing, or stale:

- **Stale** means an upstream artifact changed after a downstream one
  was written (compare `git log -1 --format=%cs -- <path>`). A
  `brief.md` newer than `architecture.md`, or an `architecture.md` newer
  than the specs, is worth flagging; say which sections changed
  (`git diff <downstream commit>.. -- <upstream path>`, summarized).
- Missing artifacts are only a problem when a later stage already ran
  (for example `specs/` without `architecture.md`).

## 2. Features

From `specs/README.md`, one row per feature: number, name, priority,
dependencies, and frontend / backend / QA status. Then check each claim
against the repository rather than trusting the table:

| Claimed | Evidence expected |
|---|---|
| Planned | `spec.md`, `plan.md`, `tasks.md` exist |
| In progress | some tasks checked in the matching `## Frontend` / `## Backend` section of `tasks.md` |
| Implemented | all of that side's tasks checked, code and tests for them present |
| Verified | `qa.md` journeys exist as tests under `tests/e2e/` (and `tests/load/` where the feature has load targets), and CI runs them |

A claim without its evidence is a **contradiction**. Report it, and name
the stage that should move the status back (pipeline rule 6).

Also flag features whose dependencies are less advanced than they are,
and features with a `ui.md` or `backend.md` missing although the other
side has one.

## 3. Open items

Count and list, with `file:line`:

- `[UPSTREAM GAP: …]` notes — each names the upstream artifact that
  needs a fix;
- `[NEEDS CLARIFICATION: …]` markers — questions for the user;
- `[ASSUMPTION: …]` markers — group them by artifact; many assumptions
  in one artifact is a sign it needs a review with the user.

Check the glossary too: a term used in `contracts/openapi.yaml`, the
domain model or the UX vision that differs from `brief.md`'s glossary is a
mismatch (the pipeline's glossary rule).

## 4. Recommend the next stage

Pick from the chain in `pipeline.md`, in this order:

1. An open `[NEEDS CLARIFICATION]` or a contradiction that blocks
   others → resolve it first (name the question or the owning stage).
2. The earliest missing or stale artifact that later stages depend on.
3. For features: the highest-priority feature whose dependencies are
   done, at the least advanced side (frontend, backend or QA).
4. Once features are Implemented: `qa:e2e` / `qa:load` to reach
   Verified; then `project:docs` for the docs site; `devsecops:pipeline`
   whenever CI is missing, and `devsecops:iac` when the architecture has
   cloud environments with no infrastructure code.

Give one primary recommendation and at most two alternatives that could
run in parallel (the frontend, backend and QA branches can).

## Output

A short report in the chat, in this order:

```
Pipeline — <product name>
Artifacts   brief ✓ · architecture ✓ (stale: brief changed 2026-05-02) · specs ✓ · ux-vision ✗ · …
Features    table: NNN name | priority | frontend | backend | QA | notes
Open items  N gaps, N clarifications, N assumptions — the ones that matter, with file:line
Contradictions  each with its evidence and the owning stage
Next        /<stage> — why; alternatives in parallel
```

Keep it to what someone needs to decide the next step. No file is
written. If the project has no pipeline artifacts at all, say so and
recommend `project:init` (or `project:architecture` for an existing
system that needs a review).
