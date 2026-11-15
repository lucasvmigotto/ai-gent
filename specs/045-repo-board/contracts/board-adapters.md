# Contract: board adapters

Status: Draft (spec 045). Owner of the contract: `rust/crates/ai-gent-core/src/repo/board/`. Every adapter maps its platform onto §1 and sends only the requests listed in its section; a new request is a change to this file.

## 1. Canonical model

```text
State   = "todo" | "doing" | "review" | "done"
Board   { platform, id, key, title, url, closed, mode }          # key: github:OWNER/NUMBER | gitlab:GROUP/PROJECT/ID
Field   { id, name, kind: "single_select" | "lists" | "status", options: [Option] }
Option  { id, name, state: State | null, mapped_by: "map" | "alias" | "unmapped" | "stale" }
Item    { id, kind: "issue" | "pull_request" | "draft", number, url, title, open,
          value: Option.name | null, state: State | "conflict" | null }
Change  { board, item, from: Option.name | null, to: Option.name, state: State | null,
          changed: bool, added: bool, sent: [request summary], dry_run: bool }
```

`--json` prints these objects (`list` → `[Board]`, `show` → `{board, field, items: [Item], truncated}`, `fields` → `{board, field, unmapped: [State]}`, `set`/`add` → `Change`). Text output is a table of the same fields. Raw platform ids appear only in `id`.

The adapter interface, in Rust terms:

```rust
pub trait BoardAdapter {
    fn list(&self, ctx: &Ctx, scope: &Scope) -> R<Vec<Board>>;
    fn fields(&self, ctx: &Ctx, board: &BoardRef) -> R<(Board, Field)>;
    fn items(&self, ctx: &Ctx, board: &BoardRef, field: &Field, pages: usize) -> R<(Vec<Item>, bool)>;
    fn locate(&self, ctx: &Ctx, board: &BoardRef, item: &ItemRef) -> R<Located>;   // on the board or not, current value
    fn plan_set(&self, located: &Located, to: &Option) -> R<Vec<Request>>;          // empty when already there
    fn plan_add(&self, located: &Located) -> R<Vec<Request>>;                       // empty when already on it
}
```

`plan_*` only build requests; one shared runner sends them through `api::api_call` with the caller's `write` flag, so no adapter can send a write by itself. `--dry-run` prints the planned requests (body with variables, token redacted) and sends none.

## 2. `.ai-gent/board.yaml`

```yaml
# per clone, git-excluded; written only with the person's agreement
version: 1
default: app
boards:
  app:
    platform: github          # github | gitlab
    owner: OWNER              # github: user or organization login
    number: 3                 # github: project number
    field: Status             # github: single-select field, default Status
    states:
      todo: Backlog
      doing: "In progress"
      review: "In review"
      done: Done
  web:
    platform: gitlab
    project: GROUP/PROJECT    # path or numeric id
    board: 12                 # board id
    mode: labels              # labels (default) | status
    states:
      todo: "workflow::todo"
      doing: "workflow::doing"
      review: "workflow::review"
      done: closed            # the built-in Closed list
```

Rules: keys are exactly those above; `states` may leave a state out (it is then `alias` or `unmapped`); values are option names (GitHub, GitLab status mode) or label names (GitLab label mode), compared exactly after trimming; the subset and the refusals are spec FR-010. A host is never read from this file: it comes from the remote and the platform's settings as for `api`.

## 3. GitHub Projects v2 (slice 1 and 2)

Endpoint: `POST https://api.github.com/graphql` (or the configured GitHub API base), `Authorization: Bearer <token>`, body `{"query": DOC, "variables": {...}, "operationName": NAME}`. Token: classic, `read:project` for reads, `project` for writes (spec D4).

Reads (each document is a named `query`):

| Name | Purpose | Root |
|---|---|---|
| `BoardsOfUser` / `BoardsOfOrg` | `list` | `user(login)` / `organization(login)` → `projectsV2(first: 50, after)` `{ nodes { id number title url closed } pageInfo }` |
| `BoardOfUser` / `BoardOfOrg` | resolve `github:OWNER/N` | `…projectV2(number)` `{ id title url closed field(name: $field) { … on ProjectV2SingleSelectField { id name options { id name } } } }` |
| `BoardFields` | `fields` (field not found) | `node(id: $project)` → `fields(first: 50) { nodes { … on ProjectV2FieldCommon { id name dataType } } }` |
| `BoardItems` | `show` | `node(id: $project)` → `items(first: 100, after)` `{ nodes { id content { … on Issue { number url title state } … on PullRequest { number url title state } … on DraftIssue { title } } fieldValueByName(name: $field) { … on ProjectV2ItemFieldSingleSelectValue { name optionId } } } pageInfo }` |
| `ItemOnBoard` | `locate` | `resource(url: $url)` → `… on Issue { id projectItems(first: 50) { nodes { id project { id } fieldValueByName(name: $field) { … } } } }` (same for `PullRequest`) |

The owner is tried as `user` first, then `organization`, when the file does not say which; a `NOT_FOUND` on both is the "not found" error. If `repositoryOwner { … on ProjectV2Owner }` is verified (spec "Not verified"), one query replaces the pair.

Writes (each a named `mutation`, one per request):

| Name | Action | Document |
|---|---|---|
| `BoardAdd` | `add` | `addProjectV2ItemById(input: {projectId: $project, contentId: $content}) { item { id } }` |
| `BoardSet` | `set` | `updateProjectV2ItemFieldValue(input: {projectId: $project, itemId: $item, fieldId: $field, value: {singleSelectOptionId: $option}}) { projectV2Item { id } }` |

`add --state S` is `BoardAdd` then `BoardSet` on the returned item id: two requests, one command, one `--write`.

Errors: a 200 with `errors[]` is a failure; `type: INSUFFICIENT_SCOPES` maps to the scope message; `NOT_FOUND` to "not found"; `RATE_LIMITED` or HTTP 403 with `x-ratelimit-remaining: 0` to the rate-limit message with `x-ratelimit-reset`.

`project-status` alias: its flags map to `owner`, `number`, `url`, `field`, `state`/`value`; its output keys (`owner`, `number`, `url`, `field`, `state`, `value`) are produced from `Change`.

## 4. GitLab (slice 1 and 2)

Base: the GitLab API base `api_base` gives today (`/api/v4`, `/api/graphql`); token `PRIVATE-TOKEN` header as today; default entry `gitlab/pat`, an instance-scoped `ai-gent/gitlab/<name>/pat` with `--entry`.

### 4.1 Label mode (default)

| Request | Action | Read or write |
|---|---|---|
| `GET /projects/:id/boards` | `list` | read |
| `GET /projects/:id/boards/:board_id` and `…/lists` | `fields` (lists by position; built-in Open and Closed added by the adapter) | read |
| `GET /projects/:id/issues?labels=L&state=opened&per_page=100` per mapped list, plus `state=opened` minus mapped labels (Open) and `state=closed` (Closed) | `show` | read |
| `GET /projects/:id/issues/:iid` | `locate` | read |
| `PUT /projects/:id/issues/:iid` `{"add_labels": T, "remove_labels": [other mapped labels it has]}` | `set` to a label list | write |
| `PUT /projects/:id/issues/:iid` `{"state_event": "close", "remove_labels": [...]}` | `set` to `closed` | write |
| `PUT /projects/:id/issues/:iid` `{"state_event": "reopen", "add_labels": T}` | `set` from closed to a label list | write |

A list based on an assignee, milestone or iteration is shown with `state: null` and is never a `set` target. `add`: every open issue of the project is on a label board (in Open or a list); `add` without `--state` reports `added: false` and sends nothing; with `--state` it is `set`. A board scoped by labels or milestone (Premium) is reported and `add` refuses when the issue is outside the scope.

### 4.2 Status mode (`mode: status`, fixtures only until G30-21)

Reads: `POST /api/graphql`, named queries over `project(fullPath) { workItems(iids: [...]) { nodes { id iid widgets { … on WorkItemWidgetStatus { status { id name } } } } } }` and the namespace's allowed statuses. Write: named `mutation BoardStatus { workItemUpdate(input: {id: $item, statusWidget: {status: $status}}) { workItem { id } errors } }`; a non-empty `errors` is a failure. Every field and input name in this section is not verified (spec "Not verified"); slice 1 confirms them with a read-only introspection query on a real instance (G30-21) before any status-mode write path is enabled outside fixtures.

## 5. Azure DevOps (deferred contract)

Kept here so the later work starts from it; nothing below is built in 3.0.0.

- Board = a team's backlog board; `list`: `GET {org}/{project}/{team}/_apis/work/boards`; `fields`: `GET …/boards/{board}` (columns with their `stateMappings`).
- `set` moves an item by its `System.State` (JSON Patch `PATCH _apis/wit/workitems/{id}`, `application/json-patch+json`), chosen from the column's state mapping; the board column itself (`System.BoardColumn`, a team-specific `WEF_…_Kanban.Column` field) is set only as an opt-in behind a capability check that reads the board's fields first.
- `add`: a work item of a type the backlog shows is on the board; no request.
- Deferred note: a real run against an Azure DevOps organization is read-only by default; a write, if one is truly needed, touches only a work item created in that run, whose wording the owner approves first, and removes it afterwards; no area, iteration or board configuration is edited. The write scope string and whether a column move needs more than `System.State` are not verified.

## 6. 3.x platforms

Jira (board via the Agile API, moves through transitions), Forgejo (project boards), Bitbucket (no boards; issue tracker status not verified) refuse with exit 2 in 3.0.0 and get their own sections when specified.
