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
 devsecops:watch ──────► (report; fixes go through the stage) after every push, merge or promotion (rule 8)
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
has produced features and a contract skeleton — `agents:parallel` runs
them, and features with no dependency between them, each in its own
worktree, landing on `dev` one at a time. Frontend and backend meet
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
| `.specify/feature.json` | Spec Kit's scripts | Spec Kit's scripts | the "current feature" pointer, rewritten by every `/speckit-specify`; never rely on it — see `spec-kit.md` |
| `specs/NNN-<feature>/ui.md` | `frontend:spec` | frontend:build | screens, components, states, tokens used |
| `specs/NNN-<feature>/tui.md` | `frontend:spec`, only when the user asked for a terminal interface | frontend:tui, qa:e2e | screens in cells, keymap, focus, states, CLI equivalents |
| `specs/NNN-<feature>/backend.md` | `backend:spec` | backend:build | endpoints, authz, errors, persistence, jobs |
| `specs/NNN-<feature>/tracker.md` | `repo:sync` (through `specsync record`) | build stages, qa:*, git:workflow, project:status | the tracker items mirroring the feature and its stories: refs for branches, commits and PRs; only when the project tracks its specs (rule 10) |
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
2. **Ask first.** Before writing, ask the questions whose answers would
   change the output — the audience, a scope boundary, a hard
   constraint, a rule — and none the inputs already answer.
   - **At most 4 questions per prompt, no limit on rounds** (4 is what
     Claude Code's question tool takes at once). Answers
     often raise new questions, so ask in rounds, each building on what
     the last one settled. Stop only when nothing that would change the
     output is open — or when the user declines to answer ("you decide",
     "skip", no answer): then the decisions are yours. Make them, and
     record each as `[DECIDED: … — delegated by the user]` with its
     reason, so it reads as a decision, not a guess.
   - **Options first.** Give each question 2–4 concrete choices, the
     recommended one first with its reason in a line (Claude Code's
     question tool; elsewhere a numbered list answered by number). Free
     text only when the choices can't be listed.
   - **Autonomy doesn't answer questions.** Permission to run the project
     on its own covers doing the work, not deciding the product. The
     stages that shape it — `project:init`, `project:architecture`,
     `frontend:uiux`, `backend:domain`, `project:spec` (its clarify step),
     `qa:strategy` — ask their rounds before they write, also under that
     permission; once the specs are clarified, the build stages run
     without new questions, and a gap they find becomes an
     `[UPSTREAM GAP: …]` note. Delegated decisions are listed in the
     handoff, so the user can revisit any of them.
   - Inside a Spec Kit step, `/speckit-clarify` asks at most five per
     session: run it again (or ask the rest the same way) until every
     `[NEEDS CLARIFICATION]` is answered or delegated.
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
     suites and they pass there. A feature with no user interface
     (tooling: an installer, a runner, a script) has no e2e journeys: its
     integration suites, named in its `qa.md` or `tasks.md`, take their
     place — they run in CI (a scheduled run counts) and pass there.
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
   Conventional Commits made as each task or phase finishes, without
   stopping to ask (a stage the user started is an approved development
   workflow) and listed in the handoff; ask before merging; promoting
   `dev` up the chain needs its own explicit request. A **new repository**
   starts the same way every time (`project:init` does it): a single-line
   `README.md` committed alone as `chore: initial commit`, then
   `dev`/`develop`, and every stage's work on branches off that. Input
   documents the user hands in are never committed unless they say so.
   **Remote mode:** when the code platform is reachable (its CLI signed in
   or a token in pass) and the user hasn't said merges are local, every
   merge into a long-lived branch goes through a PR the agent opens and
   merges on the platform (`repo:pr`; `git:workflow` has the asks, merge
   style and cleanup).
8. **Pushed means watched.** After every push, merge to a pushed branch
   or promotion, the stage that made it follows the CI runs it started
   with `devsecops:watch` until they finish, chained runs included: done
   means green in CI, not only locally. The watch runs as the small
   `scout` role in its own context (`agent-roles.md`); the stage keeps the
   fix, on its own model. On a red run `devsecops:watch` classifies the
   cause and the stage acts on it: **this change's fault** — fix it on the
   same work branch, run the failing check locally, commit, then **ask
   before pushing the fix** (unless the user gave explicit permission to
   carry out the project on its own, in which case push and watch again);
   **flaky, runner or outage** — report it, re-run only when asked, never a
   job that deploys, publishes or tags; **already red** before the change —
   report it, don't fix it unasked. Three fix rounds that leave CI red end
   the loop: report and ask. In remote mode a red run the loop won't fix
   also becomes one issue per failing workflow (`repo:issues`;
   `devsecops:watch` has the title and dedupe rules). A feature whose CI is red
   is not Implemented or Verified — move it back (rule 6) until green.
9. **Finish with a handoff line**: what was written, what's still open,
   CI's state for anything pushed, and the next stage to run.
10. **Tracked features stay in step.** In remote mode — or when the user
   asks — the specs go on the tracker (GitHub, Azure Boards, GitLab, Jira,
   Bitbucket): `repo:sync` mirrors each feature as an item and each user
   story as a child item, and records them in the feature's `tracker.md`.
   For a feature with a `tracker.md`: a stage that ticks tasks or moves a
   status (the build stages, `qa:e2e`, `qa:load`, `project:spec` after
   changing stories) runs `repo:sync` at its hand-off — item states follow
   `tasks.md` and `specs/README.md`, never the other way round; a story's
   work goes on a branch named with its ref and its commits and PRs into
   the integration branch carry `Refs <ref>` (`git:workflow`); items close
   at **Verified**, once the work is on the integration branch. Tracker
   writes are remote changes: one confirmation per sync batch, covered by
   the user's up-front permission to run the project on its own. Without a
   `tracker.md`, nothing here applies. `repo:sync` has the states and the
   per-platform mapping.
11. **READMEs follow `readme.md`.** A stage that writes or updates a
   README — the project's or a folder's — follows the shared
   conventions: the sections in order, setting names never values, and
   GitHub alerts (`[!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`,
   `[!CAUTION]`) by default for anything a reader must not miss, picked
   by consequence and used sparingly. It updates the README in the same
   change that alters how the project runs, builds, tests or is
   configured.

## Spec Kit

The five-step feature flow — `specify → clarify → plan → tasks →
implement` — with its gates, who owns each step, and where the Spec Kit
CLI and this pipeline disagree, is in `spec-kit.md` beside this file.
Every plugin that acts on `specs/` symlinks it into its own references
directory; the stages that don't need only this one.
