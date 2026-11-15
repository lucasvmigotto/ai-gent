# Feature Specification: `ai-gent repo board` (project boards across platforms)

**Feature Branch**: `docs/specs-044-045` (specification only; the work lands on its own branches, see `plan.md`)

**Created**: 2026-11-15

**Status**: Draft (for the owner's review; nothing here is approved until the owner says so)

**Input**: owner decisions given in chat on 2026-11-14 and 2026-11-15 (quoted below); `rust/crates/ai-gent-core/src/repo/` (`platforms.rs`, `api.rs`, `batch.rs`, `graphql.rs`, `project_status.rs`); `plugins/repo/` (`skills/issues`, `skills/sync`, `references/runner.md`, `references/sync.md`); `specs/039-release-3-contract/contracts/release-gates.md`; the GitLab documentation as read on 2026-11-15 (work item Status, project issue boards API).

**Depends on**: 037 (`ai-gent repo`, the pass-store module, redaction, the parity harness), 039 (the 3.0.0 gates this spec adds rows to), the GraphQL read check that decides a POST to `/graphql` by its body (`graphql::is_read_body`, landed on `dev` in `de7604b` and `e6fb251`). Read by: 039 (gates G30-20 to G30-22), `repo:issues`, `repo:sync`.

## Owner decisions on record (chat, 2026-11-14 and 2026-11-15)

- **D1** Rust only, a 3.0.0 feature, inside `ai-gent repo`. No Python path: `reporun.py` is replaced at 3.0.0 and gets no board code.
- **D2** Five actions: `ai-gent repo board list|show|fields|set|add`.
- **D3** GitHub Projects v2 through GraphQL over HTTP with the token from pass, not `gh project`.
- **D4** GitHub board support is documented as needing a **classic** token with the `project` scope (`read:project` is enough to read) until fine-grained tokens are verified for Projects v2.
- **D5** One canonical state set, `todo`, `doing`, `review`, `done` (the states `repo:sync` already uses), mapped to each board's own names in `.ai-gent/board.yaml`, which is git-excluded.
- **D6** `ai-gent repo project-status` and the batch `project_status` step stay as aliases through the 3.x line.
- **D7** Guard: reads are free; `set` and `add` need `--write` and are asked like every other repo write.
- **D8** The `repo:issues` and `repo:sync` changes are only proposed here (section "Proposed skill changes"); nothing in `plugins/` is edited before the owner approves that text.
- **D9** Scope order (2026-11-15): GitHub and GitLab in 3.0.0 slice 1; Azure DevOps later (its adapter is a deferred contract); Jira, Forgejo and Bitbucket in the 3.x line.

## Context and ground rules

- What exists today [OBSERVED: `project_status.rs`, `batch.rs`, `references/runner.md` → *Projects v2*]: `project-status` sets one Projects v2 single-select field by shelling out to `gh project field-list` and `gh project item-edit`; it maps a sync state to a live option by an alias table (`ALIASES`, `norm`) and refuses with the option list when nothing matches. The batch `project_status` step is always a write and is GitHub-only. There is no read of a board, no listing of boards, no GitLab or Azure board path, and no per-project map.
- The GraphQL read check [OBSERVED: `graphql.rs`, `platforms.rs::is_read_post`, `api.rs::api_is_write`]: a POST to GitHub `/graphql` or GitLab `/api/graphql` is a read only when its JSON body runs a `query` operation; a mutation, a subscription or anything the lexer cannot classify is a write and needs `--write`. A GET to a GraphQL path that carries a document in the URL is a write. Board code relies on this check; it never bypasses it.
- The repo guard today matches `reporun ... --write` [OBSERVED: `rust/crates/ai-gent-core/src/guard/rules/repo.rs`, `plugins/repo/hooks/guard.sh`]; spec 037 FR-C7 extends it to `ai-gent repo ... --write` at the cutover. The board rules ride on that change (FR-030).
- Tokens follow `shared/secrets.md`: pass first, read only by the runner, never printed, every output redacted. GitHub's GraphQL API refuses anonymous calls [not verified for this draft; long-standing documented behaviour], so even a public board needs the token.
- The parity fixtures in `rust/fixtures/repo/` are blessed from `reporun.py`. Board fixtures cannot be: they are hand-written, Rust-owned, and the bless script must leave them alone (FR-041).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See a board and where each item stands (Priority: P1)

A person asks "what's on the board?" in a GitHub or GitLab repository. The agent runs `ai-gent repo board list`, then `board show <board>`, and reports each item with its canonical state and the board's own column name.

**Why this priority**: every write needs these reads first, and reads are useful alone.

**Independent Test**: fixtures `board-github-list`, `board-github-show-paginated`, `board-gitlab-labels-show` (loopback stub, fake token); no network.

**Acceptance Scenarios**:
1. **Given** a GitHub owner with two Projects v2 boards, **When** `board list --owner OWNER --json` runs, **Then** both are listed with `number`, `title`, `url`, `closed`, and every request sent was a POST to `/graphql` whose body `is_read_body` accepts.
2. **Given** a board with 230 items, **When** `board show` runs, **Then** it follows the cursor to the end within `--max-pages` (default 10) and marks `truncated: true` if the cap stopped it.
3. **Given** a GitLab project with a label-list board, **When** `board show` runs, **Then** each issue appears under its list, plus the built-in Open and Closed lists, with its canonical state from the map.

### User Story 2 - Map the board's names once (Priority: P1)

The person's board calls its columns "Backlog", "Building", "Waiting on QA", "Shipped". `board fields <board>` shows the field (GitHub) or the lists (GitLab), the options, which canonical state each option is mapped to and how (`map`, `alias`, or `unmapped`). The agent proposes a `.ai-gent/board.yaml` entry, the person agrees, the agent writes the git-excluded file, and later runs use the map.

**Independent Test**: fixtures `board-fields-mapped-by-file`, `board-fields-alias-fallback`, `board-config-refusals`.

**Acceptance Scenarios**:
1. **Given** no map and options `To do / In Progress / In Review / Done`, **Then** every state is `alias` mapped, as `project-status` does today.
2. **Given** a map naming an option the board no longer has, **Then** `fields` reports the entry as `stale` and `set` to that state is refused (FR-022).

### User Story 3 - Move an item, asked first (Priority: P1)

The person says "move #42 to review". The agent runs `board set <board> '#42' --state review --dry-run`, says which field changes from what to what, the person agrees, the agent re-runs with `--write`, and the repo guard asks once.

**Independent Test**: fixtures `board-github-set-mapped-and-written`, `board-github-set-noop`, `board-set-refused-without-write`, `board-gitlab-labels-set`.

**Acceptance Scenarios**:
1. **Given** the item is at "In Progress", **When** `set --state review --write`, **Then** one read resolves the ids, one mutation `updateProjectV2ItemFieldValue` is sent, and the output names `from` and `to`.
2. **Given** the item is already at the target, **Then** no mutation is sent and the output says `changed: false`.
3. **Given** no `--write`, **Then** exit 3 before any network call; with `--dry-run`, the reads run and the mutation is printed, not sent.

### User Story 4 - Put an item on the board (Priority: P2)

`board add <board> <item> [--state todo] --write` adds an existing issue or pull request (GitHub `addProjectV2ItemById`) and, with `--state`, sets its column in the same run. An item already on the board is reported, not added twice.

**Independent Test**: fixtures `board-github-add-then-set`, `board-github-add-already-there`, `board-gitlab-add-is-membership`.

### User Story 5 - `repo:sync` keeps a board in step (Priority: P2)

A sync batch moves stories to `doing`; with a board configured, the same batch carries `board` steps (or the old `project_status` steps), one confirmation for both.

**Independent Test**: fixtures `board-batch-step-needs-write`, `board-batch-project-status-alias`, `project-status-alias-same-output`.

### Edge Cases

Listed with their outcomes in "Error cases" below; every one of them has a fixture.

## Requirements *(mandatory)*

### Command and canonical model

- **FR-001**: `ai-gent repo board list [--platform P] [--owner LOGIN | --project PATH] [--json]`; `board show BOARD [--state S] [--max-pages N] [--json]`; `board fields BOARD [--json]`; `board set BOARD ITEM (--state S | --value NAME) [--field NAME] (--write | --dry-run)`; `board add BOARD ITEM [--state S | --value NAME] (--write | --dry-run)`. `--entry` works as on every `ai-gent repo` command. Exit codes are those of `ai-gent repo` (0 ok, 1 error, 2 usage, 3 refused).
- **FR-002**: `BOARD` is a name from `.ai-gent/board.yaml`, or a literal `github:OWNER/NUMBER` or `gitlab:GROUP/PROJECT/BOARD_ID`. With one board in the file, `BOARD` may be omitted.
- **FR-003**: `ITEM` is an issue or pull request URL on the platform's host, or `#N` for the current remote's repository. Drafts (GitHub draft issues) are shown by `show` and are never created or edited by `board`.
- **FR-004**: The model (`contracts/board-adapters.md` §1) is `Board`, `Field`, `Option`, `Item`, `State`; every `--json` output uses it, the same keys on every platform, with the platform's raw ids in `id` fields only.
- **FR-005**: `--write` on `list`, `show` or `fields` is a usage error (exit 2), so a stray flag never looks like consent.

### Mapping (`.ai-gent/board.yaml`)

- **FR-010**: The file is per clone and git-excluded (the same `.git/info/exclude` treatment as other `.ai-gent/` files). Its format (contract §2) is a small YAML subset: two nesting levels, plain or quoted scalars, `#` comments. Anything else (anchors, flow style, tabs, multi-line scalars, duplicate keys, a file over 64 KiB) is refused with the line number.
- **FR-011**: Resolution order for a state: the board's entry in the file; otherwise the alias table of `project_status.rs` (`ALIASES`, `norm`, kept unchanged); otherwise refuse, listing the live options and naming `--value`. Output says which way was used (`mapped_by`).
- **FR-012**: `board` never writes the file. The skills propose an entry from `fields` and write it with the person's agreement.

### Reads and writes

- **FR-020**: Every read is a GraphQL `query` document sent as a JSON body with variables (no value is spliced into a document), or a REST GET. Every write is a mutation (GitHub, GitLab status mode) or a REST PUT/POST (GitLab label mode). All go through `api::api_call`, so the GraphQL read check and the `--write` refusal of `api_is_write` decide; `board` adds no allow path of its own.
- **FR-021**: `set` and `add` read first (resolve board, field, option and item ids, the current value) and send at most one mutation per item change. A `set` to the current value sends nothing (`changed: false`).
- **FR-022**: A mapped option that the board no longer has is refused (`stale map`), never replaced by an alias guess; an option is never created.
- **FR-023**: GraphQL responses with HTTP 200 and an `errors` array are failures; scope errors name the token kind and scope needed (D4).
- **FR-024**: Pagination follows `pageInfo` cursors (GitHub, GitLab GraphQL) or `Link`/`x-next-page` (GitLab REST), capped by `--max-pages`; a capped result says `truncated: true`.

### Platforms

- **FR-025**: GitHub adapter: contract §3. Token entry `github/pat` (default) or `--entry`; classic, scopes `read:project` (read) and `project` (write); `repo` too for a private repository's items [not verified: whether `project` alone reads private issue content].
- **FR-026**: GitLab adapter, label mode (default): contract §4.1. Boards and lists through the REST boards API; an item's state is the mapped label it carries; `set` swaps mapped labels with `add_labels`/`remove_labels`; `done` mapped to the built-in Closed list closes the issue.
- **FR-027**: GitLab adapter, status mode (`mode: status` in the file): contract §4.2, the work item Status field (GitLab Premium and Ultimate). Its GraphQL shapes are not verified; until G30-21 passes on a real instance it is fixtures-only and documented as such.
- **FR-028**: Azure DevOps, Jira, Forgejo and Bitbucket: `board` refuses with exit 2 and names the plan (`azure-devops`: deferred; the others: 3.x). The Azure DevOps contract is kept in contract §5 so the later work starts from it.

### Guard, aliases, batch

- **FR-030**: The repo guard (Rust rule, `plugins/repo/hooks/guard.sh`, `opencode/guard/rules.mjs`, together) treats `ai-gent repo board set|add ... --write` as a repo write (`repo.write`, asked) and `board list|show|fields`, and `set|add` without `--write`, as none (the runner refuses those itself). New cases in `shared/guard-fixtures/repo.json` run against all three.
- **FR-031**: `project-status NUMBER --url U (--state S | --value V) [--owner O] [--field F] [--write]` stays, with its flags and its output keys, as an alias of `board set github:O/NUMBER U`, through 3.x (D6). Its implementation moves to the GraphQL adapter (Q1).
- **FR-032**: The batch accepts a `board` step (`{"board": {"action": "set"|"add", "board": ..., "item": ..., "state"|"value": ...}}`), always a write, on GitHub and GitLab plans; the `project_status` step stays as an alias of `{"board": {"action": "set", ...}}` on GitHub.

### Fixtures and tests

- **FR-040**: Each behaviour above has a fixture in `rust/fixtures/repo/board-*.json` in the existing harness format (stubbed HTTP, fake pass store, canary token); the list is `plan.md` → *Fixtures*.
- **FR-041**: Board fixtures carry `"derived_from": "spec 045 FR-nnn"` and `"owner": "rust"`; `scripts/test-parity-repo.py --bless` skips them, and the parity test runs them like the others.
- **FR-042**: A unit test builds every document the adapters can send and asserts `graphql::is_read_body` is true for each read and false for each write; removing `--write` from a write path makes a fixture fail (mutation check).

## Error cases

| Case | Outcome | Exit |
|---|---|---|
| No pass entry for the platform | names the `pass insert` line (`secrets:pass`); nothing sent | 1 |
| GitHub scope missing (`INSUFFICIENT_SCOPES`) or fine-grained token refused | "GitHub boards need a classic token with `read:project` (read) or `project` (write)" | 1 |
| Board, owner or GitLab project not found | message with what was looked up; for GitHub, user and organization both tried | 1 |
| Field missing or not single-select (GitHub) | lists the board's fields | 1 |
| State unmapped and no alias matches | lists live options, suggests `--value` | 1 |
| Map names a missing option (`stale map`) | refused, shows live options; no fallback | 1 |
| Item not on the board for `set` | refused, suggests `board add` | 1 |
| Item already on the board for `add` | `added: false`, nothing sent | 0 |
| `set`/`add` without `--write` or `--dry-run` | refused before any call | 3 |
| `--write` on a read action | usage error | 2 |
| GitLab list not label-based (assignee, milestone, iteration) in label mode | shown by `show`; `set` refused | 1 |
| GitLab status mode on a tier without Status | refused naming Premium/Ultimate | 1 |
| Item carries two mapped labels (GitLab) | `show` reports `state: conflict`; `set` removes every mapped label but the target | 0 |
| Rate limit (GitHub primary or secondary, GitLab 429) | one message with the reset time; no retry loop | 1 |
| `board.yaml` outside the subset, unknown key, bad platform | usage error with the line | 2 |
| Platform not supported yet | names the plan (deferred / 3.x) | 2 |
| Host not allowed for the platform | refused as `api` refuses today | 3 |

## Security notes

- `set` and `add` are mutations or REST writes and go only through the write path (FR-020); the #446 body check is what classifies them, and FR-042 proves each document's class. No board code builds a GraphQL request outside `api::api_call`.
- Documents are constants with variables; board, field, option and item names from the server are data and appear only in variables and output.
- One mutation per changed item, never several operations in one document, so each `--write` changes what `--dry-run` showed.
- The token is read by the runner only, never in argv, environment of a child or output; the canary check of the parity harness runs on every board fixture.
- `board.yaml` is local and git-excluded; it can name boards and labels, never a host or a token entry outside the platform's rules.

## Proposed skill changes (need the owner's approval; not applied)

Text written for the post-cutover command word `ai-gent repo` (037 FR-C7).

1. `plugins/repo/skills/issues/SKILL.md`, description, new value (396 characters, no `: `): `Find, read, create and update issues and work items — GitHub Issues, Azure Boards (WIQL, states, links), GitLab, Bitbucket and Jira (JQL, transitions) — and move them on a GitHub Projects or GitLab board, through the platform's CLI or REST API, tokens from pass, every change asked first. Use for "find the issue about X", "what's assigned to me", "open a work item for this", "move it to Done".`
2. Same file, after *Read — freely*, a new section: `## Boards — GitHub Projects and GitLab issue boards` / `Read with \`ai-gent repo board list\`, \`show\` and \`fields\`. To move an item: \`board set BOARD ITEM --state todo|doing|review|done --dry-run\`, say the item and the column change, then the same with \`--write\` (the repo guard asks). The board's own names come from \`.ai-gent/board.yaml\`; when \`fields\` shows a state unmapped, propose the entry and write the file only after the user agrees. GitHub needs a classic token with \`project\` (\`read:project\` to read). Azure Boards keep using states as above until board support reaches them.`
3. `plugins/repo/skills/sync/SKILL.md` step 1.4, replace with: `If the platform is GitHub or GitLab, ask whether a board should be kept in step too (never assume it); if so, \`ai-gent repo board list\` for it and \`board fields BOARD\` to check every state is mapped (propose a \`.ai-gent/board.yaml\` entry when one is not). Re-ask on later syncs; \`tracker.md\` has no field for it.`
4. Same file step 3.1, replace the Projects v2 sentences with: `When a board is kept in step (step 1.4), add a \`"board"\` step for every item whose state moves, right after its issue step, in the same batch: \`{"board": {"action": "add", ...}}\` the first time, \`"set"\` after. One confirmation covers both (\`sync.md\` → *The batch*).`
5. `plugins/repo/references/sync.md`, *GitHub Projects v2 (optional board)*: rename to *Boards (GitHub Projects, GitLab issue boards)*, replace `reporun project-status` with `ai-gent repo board set`, the `item-add` sentence with `board add`, and add: `GitLab boards move items by their mapped labels (or the Status field, when the map says \`mode: status\`).` The state table stays.
6. `plugins/repo/references/runner.md` → *Projects v2 (GitHub)*: rewrite as *Boards*, stating GraphQL over HTTP with the pass token (no `gh`), the classic-token scopes, the map file, and that `project-status` and `project_status` are aliases through 3.x.
7. `plugins/repo/references/platforms/github.md` and `gitlab.md`: one line each pointing at *Boards* in `runner.md`.

## Success Criteria

- **SC-001**: G30-20, G30-21 and G30-22 (`plan.md` → *Gates*) pass.
- **SC-002**: Every row of "Error cases" is a fixture, and each fixture fails when its rule is removed.
- **SC-003**: `scripts/check.sh` passes, with the guard fixtures run against the binary, `guard.sh` and `rules.mjs`.
- **SC-004**: The existing `project-status-*` fixtures keep their CLI output byte for byte (requests change if Q1 is taken as recommended; the difference is recorded in `plan.md`).

## Not verified (as of 2026-11-15)

- Fine-grained tokens and Projects v2: whether they read and write user-owned and organization-owned boards (the reason for D4).
- `ProjectV2Owner` as a fragment on `repositoryOwner` (the adapter tries `user` then `organization` if not).
- Whether the classic `project` scope alone exposes a private repository's item content, or `repo` is also needed.
- GitLab work item Status through GraphQL: the `workItemUpdate` `statusWidget` input (status id or name), where `allowedStatuses` is read, and whether every issue exposes the status widget. Public docs read on 2026-11-15 say Status was introduced in 18.2, generally available in 18.4, Premium and Ultimate only, separate from scoped labels; issue board lists can be status-based (`status_id`, documented as introduced in 19.6). Nothing of that was run.
- GitLab REST `add_labels`/`remove_labels` on `PUT /projects/:id/issues/:iid`: from the issues API documentation, not re-checked for this draft.
- Anonymous GitHub GraphQL being refused (the reason every GitHub read needs a token).
- Azure DevOps write scope string and whether board column edits need more than `System.State`; Jira transition bodies; Bitbucket Cloud's issue tracker removal date: all deferred, recorded so the later specs check them.

## Out of scope

- Creating, renaming or deleting boards, fields, options, lists or labels.
- Draft issues, iteration and number fields, custom fields beyond the one single-select status field.
- Azure DevOps (deferred), Jira, Forgejo, Bitbucket (3.x).
- A Python `reporun` path.
