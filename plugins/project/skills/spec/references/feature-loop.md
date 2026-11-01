# The per-feature loop: specify, clarify, plan, tasks

Per `../../../references/spec-kit.md`'s flow, in this order — each step's
gate is in brackets. Use the Spec Kit skills (or their skill files), with
`SPECIFY_FEATURE=NNN-<feature>` exported once the feature exists (see
*Spec Kit* in that file), so stages running in parallel never work on
each other's feature.

1. `/speckit-specify` — prioritized user stories (P1…) with independent
   tests and Given/When/Then scenarios, edge cases, `FR-###`
   requirements, key entities, `SC-###` measurable success criteria,
   assumptions. Use glossary terms only. State in the spec that tests
   are required (the constitution demands them), so `/speckit-tasks`
   generates test tasks — the Spec Kit template calls them optional, and
   here they are not. Fill `Feature Branch` with the `git:workflow`
   branch name.
2. `/speckit-clarify` — [gate: `spec.md` written] resolve every
   `[NEEDS CLARIFICATION]` marker with the user — at most four per prompt,
   as options with a recommendation, in as many rounds as it takes
   (pipeline rule 2; re-run `/speckit-clarify`, which asks five per
   session) — and write the answers back into `spec.md`. A question the
   user declines ("you decide", "skip") is yours to decide: decide it and
   record `[DECIDED: … — delegated by the user]` with the reason. The step
   is done when no marker is open.
3. `/speckit-plan` — [gate: **no open `[NEEDS CLARIFICATION]`**] technical
   context, constitution check, project structure, `research.md`,
   `data-model.md`, `contracts/`, `quickstart.md`. Decisions:
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
4. `/speckit-tasks` — [gate: `plan.md` written and its Constitution Check
   passes] Spec Kit's phases as the installed template writes them (Setup →
   Foundational → one phase per user story → Polish), with checkpoints.
   Keep each story's phase at story level: its acceptance-test tasks and
   the layer-neutral work, plus one task per affected layer that points
   to the layer's section (`- [ ] T014 [US1] Backend implementation —
   see ## Backend`). `frontend:spec` and `backend:spec` append those
   `## Frontend` / `## Backend` sections with the detailed tasks, so
   nothing is planned twice. Tag each task with the role that runs it and
   the one that reviews it (`Executor: builder · Reviewer: thinker`, plus
   `Effort: low` for mechanical work), roles from
   `../../../references/agent-roles.md`, so `agents:orchestrate` can route it.
   Two optional tags may follow, which `agents:orchestrate` and
   `token_report.py` fill or keep: `· Tags: extend, backend, protocol, rust,
   known-pattern, integration, M` (the controlled list in
   `../../../references/token-tags.md`, one value per facet) and
   `· Tokens: projected ~420k | used 391k` (never hand-edit `used`).

The **implement** step is not yours: `frontend:build` and `backend:build`
execute it, each against its own section of `tasks.md`, then close it with
`/speckit-converge`. Name it in the handoff so nobody looks for it here.
