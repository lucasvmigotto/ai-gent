# Data model — live acceptance testing (`qa:uat`)

Three stores: the **roadmap source** (local, written by the agent, the truth for the page's text), the **register** (the Artifact's `db`, written by validators through the page and by the owner), and the **session folder** (local, the agent's records). Terms as in the spec's glossary.

## Roadmap source — `.ai-gent/uat/<slug>/roadmap.json`

| Entity | Fields | Notes |
|---|---|---|
| Roadmap | `slug`, `title`, `version` (integer, +1 per publish that changes text), `environment` (`{name, class}`; class never `production`), `mode` (`A` whole application, `B` features), `features[]` (Spec Kit numbers, mode B), `changelog[]` (`{version, at, summary, steps[]}`), `review[]` (`{version, by, at, result, findings[]}`), `artifact_url`, `settings` (`{attachments: {on: false, budget_mb}, follow_up: {on: false, every_minutes}}`) | One per session. Settings default off (Decisions D4, D6). |
| Role | `key`, `label`, `can[]` (plain-language capabilities), `privilege` (ordinal, for the coverage view) | From `qa.md` personas, the authorization matrix, or the owner. |
| Validator | `key`, `label` (shown on the page), `assigned_role` (role key, required), `invite` (account or e-mail, kept local, never on the page or in the register), `expected_scripts[]`, `uid` (filled when the owner links the account in the roster panel) | The owner's list (FR-091). |
| Script | `id` (`r0` … `rN`), `title`, `kind` (`prep`, `flow`, `rule`, `complement`), `test_data[]` (test records, prefix `<script>-<yyyymmdd>-…`), `exclusive` (bool), `cleanup_step` | Ids never reused. R0 holds the write-check and role-confirmation steps (FR-002). |
| Step | `id` (`r<N>-<M>`), `order` (display rank), `rev` (integer, +1 when text changes after a publish), `status` (`active`, `retired`), `who` (target role key, optional validator key), `action`, `expected`, `pre`, `exclusive_note`, `wait` (`{seconds, look_at}`), `evidence_expected`, `traces[]` (`specs/NNN-*/spec.md#US<n>-AS<m>`), `claims[]` (`{text, label, ref}`), `kind` (`normal`, `write-check`, `role-confirmation`) | `M` is the next free number in the script; never renumbered; a removed step is `retired` and keeps its marks (FR-005). |
| Known limitation | `text`, `steps[]` | Shown on the page; triage class `known-limitation` points here. |

Evidence labels in `claims[].label`: `OBSERVED`, `INFERRED`, `ASSUMPTION` (as in `project:introspec`); `ref` is a path and line, a spec anchor, a query description or a run, never shown on the page.

Caps (FR-098): observation 1500 characters, improvement 1500, proposal note 500, links 3 per mark of 500 characters each. `lint` warns when `active steps × (1500 + 1500 + 3 × 500)` plus overhead exceeds the per-document budget the skill takes from the runtime contract.

## Register — Artifact `db`

| Path | Document | Fields | Read | Write |
|---|---|---|---|---|
| `marks/{uid}` | one per validator | `steps: {<stepId>: {r: "OK"\|"NOK", obs, at, rev, links: [url], ev: [{asset_id, at}]}}`, `improvements: {<stepId>: {txt, at, rev}}`, `updated` | `view` | `owner`; `{self}`: `interact` |
| `roster/{uid}` | one per account the owner placed | `validator` (validator key), `role` (assigned role key), `at` | `view` | `owner` only |
| `proposals/{uid}` | one per validator who proposed | `role` (proposed role key), `note`, `at`, `decision` (`pending`, `approved`, `rejected`; written by the owner) | `view` | `owner`; `{self}`: `interact` |
| `meta/roadmap` | one | `version`, `published_at` | `view` | `owner` (agent, on each approved publish) |
| `triage/{stepId}~{uid}` | optional, agent on request | `class`, `reply`, `status` (`open`, `answered`, `fixed-in-test`, `decision`), `at` | `view` | `owner` |

`ev[]` exists only when attachments are on (FR-051); the page writes nothing there otherwise.

Rules declaration (first publish, FR-044):

```
db: { rules: [
  { path: "marks",           read: "view", write: "owner" },
  { path: "marks/{self}",                  write: "interact" },
  { path: "roster",          read: "view", write: "owner" },
  { path: "proposals",       read: "view", write: "owner" },
  { path: "proposals/{self}",              write: "interact" },
  { path: "meta",            read: "view", write: "owner" },
  { path: "triage",          read: "view", write: "owner" }
] },
user: { scopes: ["profile"] }
```

With attachments on, the set is restated with `assets: {}` (Editors upload) or `assets: {upload: "interact"}` (Contributors upload; their files show only inside the owner's organization), as the owner chooses.

- `uid` is the opaque id from `user.id()`; names and photos come from `user.profiles(ids)` at render and are never stored.
- `at` is an ISO-8601 timestamp from the writer's clock (display only; ordering by it is best-effort).
- `rev` in a mark is the step's revision the validator saw; `rev < step.rev` makes the mark **stale**.
- A whole-document `set` per save keeps each person's document consistent; one document per person makes the last-writer-wins rule harmless (FR-041).
- A proposal note is shown only to the owner on the page (FR-026); the register's rules still let any signed-in viewer read it, so the note's label says it is visible to the session.
- Owner clean-up of a stale or wrong mark is an edit of that person's document by the owner, done only when asked (FR-071), and reported to the validator.

## Preflight test read (FR-093)

Expected result of `ArtifactData` `list` on each collection at each level, after the first publish:

| Collection | `view` | `interact` | `admin` |
|---|---|---|---|
| `marks`, `roster`, `proposals`, `meta`, `triage` | readable | readable | readable |

What the read proves: each collection exists under the declared rules and is readable at the lowest level a validator holds; no collection is unexpectedly hidden. What it does not prove: the write side of the rules (reads cannot exercise it; the declaration is checked against this file by `ai-gent uat preflight verify`), and each person's sharing level (the agent cannot read it). Each validator's `kind: write-check` mark in R0 is the proof of their own write access.

## Session folder — `.ai-gent/uat/<slug>/`

| File | Content |
|---|---|
| `roadmap.json` | the roadmap source |
| `page.html` | the last page the agent rendered (compared before each republish, FR-097) |
| `artifact.url` | the Artifact link |
| `snapshots/<at>.json` | register snapshots, one or more pages each |
| `preflight.md` | checklist lines with the owner's confirmations, the test read result, and what is and is not proven |
| `follow-up.json` | `{on, every_minutes, started_at, expires_at, last_check}` |
| `triage.md` | one entry per NOK (`contracts/reports.md`) |
| `decisions.md` | the decision list |
| `report.md` | the consolidated report (Decision D2) |

## Derived values (page and `ai-gent uat digest`)

| Value | Rule |
|---|---|
| Step result | NOK if any current mark is NOK; OK if any current mark and none NOK; unmarked otherwise. Stale marks are shown, not counted. Retired steps are reported, not counted. |
| Assigned role of an account | `roster/{uid}.role`; none if absent. A pending or rejected proposal changes nothing. |
| Covered by role | a current mark from an account whose assigned role equals the step's `who`. |
| Script panel | count of steps, OK, NOK, unmarked, stale; distinct markers. |
| Write check | an account has a mark on the R0 step of kind `write-check`. |
| Left the session | an account whose profile no longer resolves; its marks stay (FR-099). |
| New since last check | marks, improvements and proposals with `at` later than the previous snapshot's, plus documents whose content hash changed. |

## State machine — a NOK through triage

```
raised ──(agent classifies)──▶ classified ──(owner approves reply)──▶ answered
classified(change-request) ──▶ decision ──(owner decides)──▶ closed
classified(defect) ──(issue opened, asked first)──▶ fixed-in-test ──(validator re-marks OK)──▶ closed
any ──(step text rewritten)──▶ stale ──(validator re-marks)──▶ raised | closed
```

## State machine — a role proposal

```
none ──(validator proposes in R0)──▶ pending ──(owner approves)──▶ approved (roster updated by the owner)
                                     pending ──(owner rejects)──▶ rejected (roster unchanged)
```
