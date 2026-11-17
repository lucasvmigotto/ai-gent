# Implementation Plan: `ai-gent repo board`

**Spec**: `spec.md` · **Contract**: `contracts/board-adapters.md` · **Branches**: one per slice (`feat/repo-board-read`, `feat/repo-board-write`, `feat/repo-board-guard-skills`) · **Crate**: `ai-gent-core` (`repo/board/`), `ai-gent` (`cmd/repo.rs`)

## Summary

A `board` module beside `api`, `batch` and `project_status` in `rust/crates/ai-gent-core/src/repo/`: the canonical model, the map loader, one adapter per platform (GitHub, GitLab), and one runner that sends what the adapters plan through `api::api_call`. `cmd/repo.rs` gets the `board` subcommand; `project-status` and the batch `project_status` step become aliases.

## Technical context

- Transport, auth, redaction, host checks, pagination helpers and the GraphQL read check exist (`api.rs`, `http.rs`, `graphql.rs`, `platforms.rs`). The board adds no transport.
- JSON is `crate::tokens::pyjson::J`; documents are `const &str` with variables.
- YAML: the crate parses flat `.ai-gent/parallel.yaml` by hand (`wt/mod.rs`); the board map needs two levels. Recommended: a small subset parser in `repo/board/map.rs` with refusals (FR-010), no new dependency (Q3).
- `project_status.rs` keeps `ALIASES`, `norm`, `resolve_option` (moved or re-exported for the board).

## Module layout

```text
repo/board/mod.rs       model (§1), Scope, BoardRef, ItemRef, Change; dispatch by platform
repo/board/map.rs       .ai-gent/board.yaml subset parser and resolution (FR-010, FR-011)
repo/board/run.rs       sends planned requests through api_call; --dry-run; Change assembly
repo/board/github.rs    §3 documents and response mapping
repo/board/gitlab.rs    §4.1 label mode, §4.2 status mode
repo/board/docs.rs      every GraphQL document, named, for the FR-042 classification test
```

## Decisions (owner decision (chat 2026-11-14 and 2026-11-15))

D1-D9 of `spec.md`. Implementation consequences:

- D3: no `gh` in any board path; `detect` still reports `gh` for other uses.
- D6: `project-status` maps to `board set github:OWNER/NUMBER`; the batch `project_status` step to a `board` step with `action: set`.
- D7: the runner refuses `set`/`add` without `--write` before any request (exit 3), as `api_call` does; the guard asks on `--write`.
- D9: Azure DevOps, Jira, Forgejo, Bitbucket refuse with exit 2 and the plan.

## Owner answers (chat 2026-11-15)

- **Q1** `project-status` and the batch `project_status` step move onto the GraphQL adapter: one path, CLI output unchanged; their requests differ from the blessed `gh` fixtures, recorded as an intended difference and the fixtures are re-blessed.
- **Q2** (default, not asked) GitLab label mode is the default on every tier; status mode is opt-in.
- **Q3** the board map is read by a small hand-written YAML subset parser in `repo/board/map.rs`, no new dependency; anything outside the subset is refused.
- **Q4** (default, not asked) `--write` on a read action is a usage error (FR-005).
- **Q5** G30-21 runs against gitlab.com on the free tier: label mode only, read-only, token `ai-gent/gitlab/<name>/pat`. Status mode stays fixtures-only and is disclosed as untested in the release notes.
- **Q6** G30-22: the owner asked to try it on the ai-gent repository itself with real, contexted content. Before any write the exact item titles and the board are confirmed with the owner; reads run first; only items the run created are touched.

## Fixtures (`rust/fixtures/repo/board-*.json`, Rust-owned, FR-040/FR-041)

Slice 1 (reads): `board-github-list` (user and org, pagination), `board-github-owner-user-then-org`, `board-github-show-paginated` (230 items, cap and `truncated`), `board-github-fields-alias-fallback`, `board-fields-mapped-by-file`, `board-fields-stale-map`, `board-github-scope-error` (200 + `INSUFFICIENT_SCOPES`), `board-github-not-found`, `board-github-rate-limited`, `board-missing-entry`, `board-config-refusals` (anchors, tabs, duplicate key, unknown key, size), `board-write-flag-on-read-refused`, `board-platform-deferred` (azure-devops, jira, forgejo, bitbucket), `board-gitlab-labels-list`, `board-gitlab-labels-show` (Open, lists, Closed, a `conflict` item), `board-gitlab-non-label-list`, `board-gitlab-status-show` (status mode, fixture shapes from §4.2).

Slice 2 (writes): `board-set-refused-without-write`, `board-set-dry-run-sends-nothing`, `board-github-set-mapped-and-written`, `board-github-set-noop`, `board-github-set-not-on-board`, `board-github-set-unmapped-lists-options`, `board-github-add-then-set`, `board-github-add-already-there`, `board-gitlab-labels-set` (swap), `board-gitlab-labels-set-closed` and `-reopen`, `board-gitlab-add-is-membership`, `board-gitlab-status-set` (fixture only), `board-gitlab-status-tier-refused`, `board-batch-step-needs-write`, `board-batch-step-written`, `board-batch-project-status-alias`, `project-status-alias-same-output`.

Each fixture records the exact requests (`seen`), the canary token, and stdout/stderr; the loopback stub answers by method, path and, for GraphQL, `operationName` (T003 extends the stub if it cannot match on the body).

## Slice 2 notes: `project-status` and the batch step on the board path (T014)

`project-status` and the batch `project_status` step call `board::run::run_change` (a `set` on `github:OWNER/NUMBER`) and print the old keys; their CLI output and exit codes are unchanged (`project-status-refused-without-write`, `-needs-state-or-value`, `-no-owner`, `-batch-step-needs-write`, `-batch-rejects-other-platforms` stay blessed from the Python oracle). Six fixtures sent `gh` requests and are now Rust-owned with scripted GraphQL routes (`state-mapped-and-written`, `explicit-value-bypasses-mapping`, `owner-defaults-to-remote`, `unknown-field-fails`, `alias-and-punctuation-match`, `batch-step-written-and-resolves-references`); the Python generator no longer produces them. Intended request differences:

- `gh project field-list` + `gh project item-edit` become `BoardOf{User,Org}` + `ItemOnBoard` + `BoardSet` (and `BoardFields` when the field is missing); no `gh` is used, so the token is the pass entry's Bearer header, not `GH_TOKEN`.
- A read of the item (`ItemOnBoard`) is new: it resolves the project item id and the current value, and a `set` to the current value now sends no mutation.
- `--value` must be an option the board has (exact name): `gh` took any string and failed late; the `explicit-value` fixture's board now has a `Blocked` option.
- An item that is not on the board is refused (`board add` first); `gh item-edit` failed the same way later.
- The order of refusals is kept: state/value usage error, owner usage error, then the `--write` refusal, before any request. The field name is matched by GitHub (`field(name:)`), not case-insensitively by the tool; the printed `field` is GitHub's name. Not verified until G30-22.
- Alias matching now assigns each state to a distinct option (`map::apply`), as `board fields` shows it; the old per-call search could map two states to one option.
- GitLab label-mode `set` removes only the mapped labels (options with a state) the issue carries; unmapped list labels stay. `Open` is a valid `set` target (reopens, removes mapped labels).
- A URL item is checked before any request: an issue or pull request on `github.com` for GitHub; an `issues` or `work_items` URL on the GitLab host (the remote's, else `gitlab.com`) for GitLab, never a merge request, and never on a board named by numeric project id (use `#N` or name the board by path). Merge request URLs would otherwise be PUT to `issues/N`.
- A batch `board` step is resolved against `.ai-gent/board.yaml` and the plan's platform before any step runs; the refusal and `--dry-run` text name the board key. A board given by a `{{reference}}` is checked when its step runs.
- `add --state` whose `BoardSet` fails after `BoardAdd` went through prints the Change (`added: true`, `sent`, `error`) and exits 1.
- GitHub field lookup stays exact; when the field is missing, a single case-insensitive match among `BoardFields` is retried under the board's spelling (keeps `project-status --field status`).
- GitLab status mode reads the board (`GET /projects/:id/boards/:id`) before a write and refuses a scoped board: a work item carries no labels or milestone in the status document, so the scope cannot be checked.
- GitLab label mode: the current column counts every label list the issue carries; moving to a list it already carries, or to `Open` with nothing to remove, sends nothing. Multi-column case: an issue in two mapped lists is a `conflict` on reads; `set` removes the other mapped labels.
- Python oracle coverage: the six `project-status-*` cases that sent `gh` requests are no longer run through `reporun.py` (Rust-owned now). The oracle still runs the five that send none. Follow-up (tasks.md): once `reporun.py` is deleted at the cutover nothing else changes; until then the `gh`-based write path of `reporun.py` is untested by the fixtures.
- Status-mode names (`BoardStatusLocate`, `BoardStatus`, `WorkItemID`, `WorkItemsStatusesStatusID`) are unverified, fixtures-only like the reads.

## Tests

- `cargo test -p ai-gent-core repo::board` (unit: map parser, resolution order, response mapping, `plan_set`/`plan_add` empty when nothing changes).
- `docs.rs` classification test (FR-042): every read document `is_read_body == true`, every write `false`, and `api_is_write` true for every planned GitLab PUT.
- `cargo test -p ai-gent --test parity_repo` runs the board fixtures with the blessed ones.
- Guard: new cases in `shared/guard-fixtures/repo.json` run by `scripts/check.sh` against the binary, `guard.sh`, `rules.mjs`; `scripts/test-repo-guard.sh`.
- Mutation pass: drop the `--write` check in `run.rs`, the stale-map refusal, the GraphQL `errors[]` check, the no-op short-cut; each makes a fixture fail.

## Gates (proposed rows for `specs/039-release-3-contract/contracts/release-gates.md` → G30, added by T030 after approval)

| Id | Criterion | Measure | Evidence |
|---|---|---|---|
| G30-20 | `ai-gent repo board` on GitHub: every slice 1 and 2 GitHub fixture green; `project-status` and the batch `project_status` step give the same CLI output as before (aliases, spec 045 FR-031/FR-032); the FR-042 classification test green | `cargo test -p ai-gent --test parity_repo`; `cargo test -p ai-gent-core repo::board` | |
| G30-21 | `ai-gent repo board` on GitLab: every GitLab fixture green, and a read-only real check (`list`, `fields`, `show`, plus the §4.2 introspection query) against an instance the owner chooses, with the owner's token under pass at `ai-gent/gitlab/<name>/pat`; status mode stays fixtures-only and is disclosed if the instance lacks it | fixtures as G30-20; the owner's run transcript (redacted) | |
| G30-22 | Owner real-repository run on GitHub: `list`, `fields`, `show`, `set --dry-run`, `set --write`, `add --write` on a test board the owner chooses, classic token under pass; the guard asked for each write | the owner's transcript (redacted) and statement | |

## Risks

- GitHub's GraphQL schema names (fragments, `resource(url:)`, `fieldValueByName`) are from documentation knowledge, not run; G30-22 is the first real proof. Mitigation: fixtures built from real response shapes captured during G30-22 replace hand-written ones where they differ.
- GitLab status mode may change shape between releases (introduced 18.2, GA 18.4); kept opt-in and fixtures-only until G30-21.
- Label-mode `show` costs one request per list; the page cap bounds it.
- The repo guard learning `ai-gent repo` (037 FR-C7) is a precondition of FR-030; if it slips, slice 3 carries it.

## Verification

Per slice: `cargo test` for the two crates, `cargo clippy -D warnings`, `scripts/check.sh --quick --quiet`; the full `scripts/check.sh` once before each PR. Slice 3 also runs the `repo` plugin's evals for the changed skills (`scripts/run-evals.py repo`), recorded in `docs/product/evals.md`.
