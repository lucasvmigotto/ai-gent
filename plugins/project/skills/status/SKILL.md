---
name: status
description: Report where a project stands in the product pipeline — which artifacts exist, each feature's frontend, backend and QA status, open gaps, status claims the repo contradicts — and recommend the next stage. Read-only. Use for "where are we", "what's next", "project status", "which stage should I run", or at a session start on a pipeline project.
---

# Pipeline status

A read-only snapshot of a project that uses the product pipeline, ending
in a recommendation of what to run next. It never writes or fixes
anything: every problem it finds goes into the report, with the stage that
owns the fix.

Read `../../references/pipeline-digest.md` (the short form; the full `../../references/pipeline.md` only when the digest conflicts with or is silent on your case) first — the artifact table and the
status rules there are what this skill checks against — and
`../../references/spec-kit.md` for the steps a feature moves through.

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
- **Not applicable** artifacts are the rows of `specs/README.md`'s
  `## Applicability` table (`| Artifact | Status | Reason |`, status `N/A`,
  reason required). List them apart as "not applicable (reason)", never as
  missing or as gaps, and never recommend their stage. Report a row that
  names an unknown artifact or has no reason.

## 2. Features

From `specs/README.md`, one row per feature: number, name, priority,
dependencies, and frontend / backend / QA status. Then check each claim
against the repository rather than trusting the table:

| Claimed | Evidence expected |
|---|---|
| Planned | `spec.md` exists with no open `[NEEDS CLARIFICATION]`, plus `plan.md` and `tasks.md` |
| In progress | some tasks checked in the matching `## Frontend` / `## Backend` section of `tasks.md` |
| Implemented | all of that side's tasks checked, code and tests for them present, and no unchecked `## Phase N: Convergence` phase |
| Verified | `qa.md` journeys exist as tests under `tests/e2e/` (and `tests/load/` where the feature has load targets), and CI runs them and passed on the last run. For tooling without a UI: the integration suites its `qa.md` or `tasks.md` names run in CI (a scheduled run counts) and passed on the last run |

A claim without its evidence is a **contradiction**. Report it, and name
the stage that should move the status back (pipeline rule 6). Two that
matter most: a feature claimed Implemented whose Convergence phase still
has unchecked tasks (the build stage hasn't closed its loop), and a
feature whose `spec.md` still carries an open `[NEEDS CLARIFICATION]`
marker — that one blocks `plan`, so everything downstream of it is
stalled.

Read CI's latest result on `dev`/`develop` and `main`/`master` with the
platform commands in `devsecops:watch` (read-only: list and view, never
re-run). A red default branch is an open item, and a feature claimed
Verified while its suites fail there is a contradiction.

**Tracker.** For features with a `tracker.md` (pipeline rule 10),
`specsync features` from `repo:sync` says which are out of step with the
tracker (pending creates, updates or state moves) — an open item that
`repo:sync` clears. Reading the items' remote state is optional and
read-only; an item closed there while the specs say otherwise is drift
to report.

**Tokens.** Where tasks carry `Tokens:` tags or a feature has a `Tokens`
block in its `tracker.md` (`../../references/token-tracking.md`), show per
feature the projected and used billable tokens and the estimation error
(used against projected, signed), the coordinator's share, and the budget
left: `.ai-gent/budget.yaml`'s caps less the session's and the week's use
(`python3 <agents plugin>/scripts/token_report.py budget --projected 0`
prints them). Read-only; a feature with no figures says so, it isn't an
error.

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
   done, at the least advanced side (frontend, backend or QA) — following
   the flow in `../../references/spec-kit.md`, that means the first step
   that feature
   hasn't cleared (`specify`, `clarify`, `plan`, `tasks`, `implement`).
4. Once features are Implemented: `qa:e2e` / `qa:load` to reach
   Verified; then `project:docs` for the docs site; `devsecops:pipeline`
   whenever CI is missing, and `devsecops:iac` when the architecture has
   cloud environments with no infrastructure code.

Give one primary recommendation and at most two alternatives that could
run in parallel (the frontend, backend and QA branches can; when several
are ready, `agents:parallel` runs them side by side in worktrees).

## Output

A short report in the chat, in this order:

```
Pipeline — <product name>
Artifacts   brief ✓ · architecture ✓ (stale: brief changed 2026-05-02) · specs ✓ · ux-vision ✗ · …
Features    table: NNN name | priority | frontend | backend | QA | notes
CI          dev ✓ · main ✗ <workflow> — the failing job, if any
Open items  N gaps, N clarifications, N assumptions — the ones that matter, with file:line
Contradictions  each with its evidence and the owning stage
Tokens      per feature: projected → used (error), coordinator share; budget left
Next        /<stage> — why; alternatives in parallel
```

Keep it to what someone needs to decide the next step. No file is
written. If the project has no pipeline artifacts at all, say so and
recommend `project:init` (or `project:architecture` for an existing
system that needs a review).
