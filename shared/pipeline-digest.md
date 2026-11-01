# Pipeline digest

A short form of `pipeline.md`, the contract. Enough for a stage that reads
upstream artifacts, writes its own and updates statuses. If anything here
conflicts with, or is silent on, your case, read `pipeline.md`: it wins.
A pointer with the section sign names the heading and rule holding the full text.

## Artifacts, owners, readers

| Path | Owner (writes) | Readers |
|---|---|---|
| `docs/product/brief.md` | `project:init` | everyone; holds the glossary |
| `docs/product/architecture.md`, `docs/product/adr/` | `project:architecture` | spec, domain, devcontainer, devsecops, qa:load, docs |
| `docs/product/ux-vision.md` | `frontend:uiux` | frontend:*, docs |
| `docs/product/domain-model.md` | `backend:domain` | backend:*, docs; runs after `project:spec` |
| `docs/product/references/` | the user | init, uiux, domain |
| `.specify/` | Spec Kit (`specify init`) | project:spec |
| `.specify/memory/constitution.md` | `project:spec` | every spec and build stage |
| `specs/README.md` | `project:spec` (build and QA stages update only the status columns) | everyone |
| `specs/NNN-<feature>/spec.md`, `plan.md`, `research.md`, `data-model.md`, `quickstart.md`, `tasks.md`, `contracts/`, `checklists/` | `project:spec` (via Spec Kit) | everyone downstream |
| `.specify/feature.json` | Spec Kit's scripts | never rely on it |
| `specs/NNN-<feature>/ui.md` | `frontend:spec` | frontend:build |
| `specs/NNN-<feature>/tui.md` | `frontend:spec`, only when a terminal interface was asked | frontend:tui, qa:e2e |
| `specs/NNN-<feature>/backend.md` | `backend:spec` | backend:build |
| `specs/NNN-<feature>/tracker.md` | `repo:sync`; its `Tokens` block by token tracking | build stages, qa:*, git:workflow, project:status |
| `specs/000-design-system/` | `frontend:spec` | frontend:build |
| `contracts/openapi.yaml` (+ `asyncapi.yaml`) | skeleton by `project:spec`, canonical by `backend:spec` | frontend:*, backend:* |
| `docs/product/test-strategy.md` | `qa:strategy` | build stages, qa:*, devsecops:pipeline |
| `specs/NNN-<feature>/qa.md` | `qa:strategy` | build stages, qa:e2e, qa:load |
| `tests/e2e/`, `tests/load/` | `qa:e2e`, `qa:load` | devsecops:pipeline |
| CI/CD config (`.github/workflows/` by default) | `devsecops:pipeline` | everyone |
| `infra/` | `devsecops:iac` | devsecops:pipeline |
| `docs/product/delivery.md` | `devsecops:pipeline` (supply-chain section by `devsecops:supply-chain`, infrastructure section by `devsecops:iac`) | everyone, project:docs |
| `docs/site/` | `project:docs` | none |
| `docs/product/introspec.md`, `docs/product/sbom.cdx.json` | `project:introspec` | retrofit, refactor, status |
| `docs/product/retrofit.md` | `project:retrofit` | refactor, devsecops:*, docs |
| `docs/product/db/<database>/inspection.md`, `schema.sql`, `diff-*.md` | `db:inspect` | introspec, db:review, backend:* |
| `docs/product/db/review-<date>.md` | `db:review` | backend:build, architecture, qa:review |
| `docs/product/db/investigations/<date>-<slug>.md` | `db:investigate` | backend:build, qa:review |
| `docs/product/security/scope-<date>.md` | `security:scope` | red, blue, purple |
| `docs/product/security/red-<date>.md` | `security:red` | blue, purple, devsecops:pipeline |
| `docs/product/security/blue-<date>.md` | `security:blue` | purple, devsecops:pipeline |
| `docs/product/security/purple-<date>.md` | `security:purple` | devsecops:pipeline, project:status |
| `docs/product/refactor.md`, `docs/product/bcr/NNNN-*.md` | `project:refactor` | spec, backend:*, frontend:*, qa:* |

Full table with notes: `pipeline.md` §"Artifacts and owners".

## Ownership

- Only the owner creates or restructures an artifact; downstream stages add
  their own layer (`ui.md`, `backend.md`, `qa.md`, a `## Frontend`,
  `## Backend` or `## QA` section in `tasks.md`) and never rewrite another's.
- An upstream artifact that is wrong or missing something: record
  `[UPSTREAM GAP: …]` in your own artifact, tell the user, propose the fix.
- A stage ticks only its own tasks in `tasks.md`.
- `project:introspec` may create missing artifacts on existing code
  (marked `Reconstructed by project:introspec`); owners take over later.
- Glossary: append-only exception to ownership; UI, domain, API and docs use
  its terms (§"The glossary is shared vocabulary").
- `contracts/openapi.yaml` is the frontend/backend seam; change it through
  `backend:spec`, never from a build stage (§"The contract is the frontend/backend seam").

## Applicability

`specs/README.md` may hold `## Applicability` with `| Artifact | Status | Reason |`
rows (Artifact: the pane's short name; Status: `N/A`; Reason: required text).
An N/A artifact is skipped and said so, never a gap; propose a row and ask the
user, never decide alone (§"Applicability: an artifact can be not applicable").

## Statuses

- Feature (per side, in `specs/README.md`): Planned, In progress,
  Implemented, Verified. Only the named stage moves one forward (a `*:build`
  stage after `/speckit-converge`; `qa:e2e` and `qa:load` plus green CI for
  Verified); any writing stage moves it back when the claim fails; read-only
  stages report, never write (§"Rules every stage follows" #6).
- Documents (`brief.md`, `architecture.md`, `ux-vision.md`,
  `domain-model.md`): Draft until the user accepts, then Accepted. ADRs:
  proposed, accepted, superseded.

## Labels

- `[ASSUMPTION: …]`, `[NEEDS CLARIFICATION: …]`, `[DECIDED: … — delegated by the user]`
- Reconstructed claims: `[OBSERVED: path:line]`, `[INFERRED: signal → conclusion]`
- Research: `[RESEARCHED: S<n>]`, `[KNOWN]`, defined in `research.md`.

## Rules, one line each

1. Read before writing: this contract, then every upstream artifact that exists; say what was missing (§"Rules every stage follows" #1).
2. Ask first: up to 4 questions per prompt, any number of rounds, none the inputs answer; if the user delegates, record `[DECIDED]` (§"Rules every stage follows" #2, `asking.md`).
3. Never invent silently: mark assumptions and open questions inline (§"Rules every stage follows" #3).
4. Comprehensive means complete coverage via the skill's checklist, not length (§"Rules every stage follows" #4).
5. Real content: the product's own names and scenarios, no placeholders (§"Rules every stage follows" #5).
6. Status is explicit and moves as above (§"Rules every stage follows" #6).
7. Versioning follows `git:workflow`; releases are automatic, no tags by hand (§"Rules every stage follows" #7).
8. Pushed means watched: follow CI with `devsecops:watch`; a red feature is not Implemented or Verified (§"Rules every stage follows" #8).
9. Finish with a handoff line: written, open, CI state, next stage (§"Rules every stage follows" #9).
10. Tracked features (with a `tracker.md`) stay in step through `repo:sync` at hand-off (§"Rules every stage follows" #10).
11. READMEs follow `readme.md` (§"Rules every stage follows" #11).
12. Language: code, comments and commits in English; documents per `language-policy.md` (§"Rules every stage follows" #12).

Spec Kit flow and where it differs from this pipeline: `spec-kit.md` (§"Spec Kit").
