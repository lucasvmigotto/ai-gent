# Tasks: `ai-gent repo board`

Format: `- [ ] Tnnn [P] [USn] description. Files. Verify.` Each slice is one pull request into `dev`, reviewed by an agent other than its author. Nothing is built or checked off from this draft. Precondition for all slices: the GraphQL body check (`graphql::is_read_body`, on `dev` since `de7604b`, `e6fb251`) stays in force.

## Slice 1: GitHub and GitLab reads (`list`, `show`, `fields`), 3.0.0

- [x] T001 [US1] Model and dispatch: `Board`, `Field`, `Option`, `Item`, `Change`, `BoardRef`, `ItemRef` (contract §1), platform dispatch, the exit-2 refusal for azure-devops, jira, forgejo, bitbucket (FR-028). Files: `rust/crates/ai-gent-core/src/repo/board/mod.rs`, `repo/mod.rs`. Verify: `cargo test -p ai-gent-core repo::board::model`.
- [x] T002 [US2] Map file: the `.ai-gent/board.yaml` subset parser with line-numbered refusals, resolution order map → alias → refuse, `stale` detection (FR-010, FR-011, FR-022); `ALIASES`/`norm` reused from `project_status.rs`. Files: `repo/board/map.rs`. Verify: `cargo test -p ai-gent-core repo::board::map`.
- [x] T003 Harness: board fixtures as Rust-owned (`"owner": "rust"`, `derived_from` the FR), skipped by `scripts/test-parity-repo.py --bless`; the loopback stub matches GraphQL requests by `operationName` if it cannot already. Files: `rust/crates/ai-gent/tests/parity_support/`, `scripts/test-parity-repo.py`. Verify: `cargo test -p ai-gent --test parity_repo` with one board fixture; `--bless` leaves it unchanged.
- [x] T004 [P] [US1] GitHub reads: documents `BoardsOfUser/Org`, `BoardOfUser/Org`, `BoardFields`, `BoardItems` (contract §3; `ItemOnBoard` comes with `locate` in slice 2), user-then-org owner lookup, cursor pagination with cap, `errors[]` mapping (scope, not found, rate limit). Files: `repo/board/github.rs`, `repo/board/docs.rs`. Verify: slice 1 GitHub fixtures (`plan.md` → *Fixtures*).
- [x] T005 [P] [US1] GitLab reads, label mode: boards, lists with Open and Closed, issues per list, `conflict` and non-label lists (contract §4.1). Files: `repo/board/gitlab.rs`. Verify: `board-gitlab-labels-*`, `board-gitlab-non-label-list`.
- [x] T006 [US1] GitLab reads, status mode, fixtures only (contract §4.2); the read-only introspection query for G30-21 (run as `board fields BOARD --introspect`). Files: `repo/board/gitlab.rs`, `repo/board/docs.rs`. Verify: `board-gitlab-status-show`.
- [x] T007 [US1] CLI: `ai-gent repo board list|show|fields` with `--json`, text tables, `--entry`, `--max-pages`, `--write` refused on reads (FR-001 to FR-005). Files: `rust/crates/ai-gent/src/cmd/repo.rs`. Verify: CLI fixtures; `ai-gent repo board --help`.
- [x] T008 [US1] Classification test (FR-042) for every read document. Files: `repo/board/docs.rs` tests. Verify: `cargo test -p ai-gent-core repo::board::docs`.
- [ ] T009 Owner read check on GitLab (G30-21): the owner runs `list`, `fields`, `show` and the introspection query against the instance they choose (gitlab.com, free tier), token `ai-gent/gitlab/<name>/pat`; findings correct §4.2 and the status fixtures. Evidence: redacted transcript in the PR.

## Slice 2: writes (`set`, `add`), 3.0.0

- [x] T010 [US3] Runner: `plan_set`/`plan_add` requests sent through `api::api_call` with the caller's `write`; refusal before any call without `--write`; `--dry-run` prints planned requests; no-op short-cut (FR-020, FR-021). Files: `repo/board/run.rs`. Verify: `board-set-refused-without-write`, `board-set-dry-run-sends-nothing`.
- [x] T011 [P] [US3] [US4] GitHub writes: `BoardSet`, `BoardAdd`, `add --state` as two requests; not-on-board and unmapped refusals. Files: `repo/board/github.rs`. Verify: slice 2 GitHub fixtures; T008's test extended to writes.
- [x] T012 [P] [US3] [US4] GitLab writes, label mode: label swap, close, reopen, `add` as membership (contract §4.1). Files: `repo/board/gitlab.rs`. Verify: `board-gitlab-labels-set*`, `board-gitlab-add-is-membership`.
- [x] T013 [US3] GitLab writes, status mode, fixtures only, behind `mode: status`; tier refusal. Files: `repo/board/gitlab.rs`. Verify: `board-gitlab-status-set`, `board-gitlab-status-tier-refused`.
- [x] T014 [US5] Aliases and batch: `project-status` → `board set` (Q1: GraphQL), the batch `board` step and `project_status` as its alias, both always writes (FR-031, FR-032); existing `project-status-*` fixtures keep their CLI output, request differences recorded in `plan.md`. Files: `repo/project_status.rs`, `repo/batch.rs`, `cmd/repo.rs`. Verify: `cargo test -p ai-gent --test parity_repo`.
- [ ] T014b Follow-up: the Python oracle (`scripts/test-parity-repo.py`) no longer covers the `project-status` write path (six cases became Rust-owned, `plan.md` slice 2 notes); nothing to do before the cutover deletes `reporun.py`, but G30-22 is then the only check of the GraphQL path against a real board.
- [ ] T015 Mutation pass (plan *Tests*) and the full `scripts/check.sh`. Verify: each removed rule fails a fixture.
- [ ] T016 Owner real run on GitHub (G30-22): on the repository and test board the owner chooses (Q6), classic token under pass. Before any write, the exact titles are confirmed with the owner. Evidence: redacted transcript; captured response shapes replace hand-written fixture bodies where they differ.

## Slice 3: guard rules, skills, docs, 3.0.0

- [ ] T020 [US3] Guard: `ai-gent repo board set|add ... --write` is `repo.write` (asked); reads and flagless writes are none; Rust rule, `guard.sh`, `rules.mjs` together, with cases in `shared/guard-fixtures/repo.json` (FR-030). Files: `rust/crates/ai-gent-core/src/guard/rules/repo.rs`, `plugins/repo/hooks/guard.sh`, `opencode/guard/rules.mjs`, `shared/guard-fixtures/repo.json`. Verify: `scripts/test-repo-guard.sh`; `scripts/check.sh` parity. Depends on 037 FR-C7.
- [ ] T021 Skills and references: apply the approved text of `spec.md` → *Proposed skill changes* (items 1-7). Files: `plugins/repo/skills/{issues,sync}/SKILL.md`, `plugins/repo/references/{sync,runner}.md`, `plugins/repo/references/platforms/{github,gitlab}.md`. Blocked on the owner's approval of that text (D8). Verify: `scripts/check.sh` (descriptions, links); `scripts/run-evals.py repo`.
- [ ] T022 Evals: trigger cases for "move it to review on the board", "what's on the board" (reach `repo:issues`, not `repo:sync`), a sync case with a board. Files: `plugins/repo/evals/`. Verify: `scripts/run-evals.py repo`.
- [ ] T023 Docs: README and docs-site reference rows for `ai-gent repo board`, the classic-token note (D4), CHANGELOG `## Unreleased` line (`skill-docs`). Files: `README.md`, `docs/site/`, `CHANGELOG.md`. Verify: `scripts/check.sh`; docs CI.
- [ ] T030 Gates: add G30-20, G30-21, G30-22 (`plan.md` → *Gates*) to `specs/039-release-3-contract/contracts/release-gates.md`. Blocked on the owner's approval of this spec.

## Deferred: Azure DevOps (after 3.0.0)

- [ ] T040 Azure DevOps adapter from contract §5: reads, then `set` by `System.State`, column edits only behind the capability check; real runs read-only by default as §5 says. Blocked on its own spec amendment and the owner's go-ahead.

## 3.x

- [ ] T050 Jira, Forgejo, Bitbucket: each its own contract section and spec amendment before any code.
