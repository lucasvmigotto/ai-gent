# Specs ↔ tracker: the sync contract

How `repo:sync` mirrors a feature's specs into the tracker — GitHub
Issues, Azure Boards, GitLab issues, Jira, Bitbucket issues — and keeps
the items' state in step with the pipeline. The repository is the source
of truth; the tracker mirrors it.

## Items

| Spec | Item | Body |
|---|---|---|
| a feature `specs/NNN-<name>/` | one **feature item** — `[NNN] <title>` | link to `spec.md`, statuses, the stories, tasks outside any story |
| a user story `USn` in `spec.md` | one **story item**, a child of the feature item — `[NNN] USn: <title>` | the story text and acceptance scenarios, and its tasks as a checklist |

A task belongs to a story by its `[USn]` tag, else by the
`Phase N: User Story N` heading it sits under. Tasks aren't items: they
are the story's checklist, updated on each sync. Retired features (a
README status or note saying retired, superseded, removed) get no items.

The first line of each body is a marker —
`<!-- ai-gent:sync specs/012-export US1 -->` — and the body is written by
`repo:sync` from the specs: **edits belong in the spec, discussion in the
item's comments**. Each sync overwrites the body when the specs changed.

## States

`specsync` derives four states from the repository; nobody sets them by
hand:

| State | Feature item | Story item |
|---|---|---|
| `todo` | both sides Planned | no task checked |
| `doing` | a side In progress (or Partial) | some tasks checked |
| `review` | Implemented (the least advanced side counts) | all its tasks checked |
| `done` | Verified | the feature is Verified |

`done` closes items, and only once the work is on the **integration
branch** — `reporun chain`'s `integrated_on`: `dev`/`develop`, or the
production branch when there's no integration level. Check it before a
batch that closes anything: the feature's merges are in that branch
(`git log <integrated_on> --oneline --grep '<ref>'`, or the work branch
in `git branch --merged <integrated_on>`). If `chain` says `ask` — two
candidates at a level, or no production branch — ask the user for the
flow before closing; assume work branches → `dev`/`develop` →
[`staging`/`homolog`] → `main`/`master`.

Closing keywords (`Closes #N`) act only when merged into the **default
branch** (`keywords_close_on`), so with a `dev` flow they can't close at
Verified. Commits and pull requests into the integration branch say
`Refs <ref>`; the promotion pull request to production lists
`Closes <ref>` for every story it carries — a backstop that's harmless
when the item is already closed.

## Per platform

The state names differ; map by **category**, never by a guessed name.

**GitHub** — feature: an issue; story: an issue added as a **sub-issue**
(`POST /repos/{owner}/{repo}/issues/{{feature.number}}/sub_issues`
`{"sub_issue_id": {{US1.id}}}` — the child's `id`, not its number).
`todo`/`doing`/`review` are open; `done` is
`PATCH …/issues/<n>` `{"state":"closed","state_reason":"completed"}`. If
the repository already has status labels (`in progress`, `in review`),
keep them in step; don't create labels unasked. Ref: `#<number>`.

**Azure Boards** — the process decides the types: read
`GET /{project}/_apis/wit/workitemtypes` once. Feature item: `Feature`
(Agile, Scrum, CMMI) or `Epic` (Basic). Story item: `User Story`
(Agile), `Product Backlog Item` (Scrum), `Requirement` (CMMI), `Issue`
(Basic). Create with a JSON Patch body
(`POST '/{project}/_apis/wit/workitems/$User%20Story'`): `System.Title`,
`System.Description` = the plan's `body_html`, and for a story the parent
link `{"op":"add","path":"/relations/-","value":{"rel":"System.LinkTypes.Hierarchy-Reverse","url":"https://dev.azure.com/{org}/_apis/wit/workItems/{{feature.id}}"}}`.
States: `GET /{project}/_apis/wit/workitemtypes/<type>/states` gives each
state's **category** — `todo` → Proposed, `doing` → In Progress,
`review` → Resolved (In Progress when the type has none), `done` →
Completed; set `System.State` to that state's name. Ref: `AB#<id>` when
the code is on GitHub, `#<id>` in Azure Repos.

**GitLab** — feature and story: issues; link each story to the feature
(`POST /projects/{project_path}/issues/<story iid>/links`
`{"target_project_id": <id>, "target_issue_iid": <feature iid>}`;
epics and child items only where the tier has them). Open for
`todo`/`doing`/`review` (scoped `workflow::` labels if the project uses
them); `done` is `PUT …/issues/<iid>` `{"state_event":"close"}`. Ref:
`#<iid>`.

**Jira** — feature: an `Epic`; story: a `Story` with
`"parent": {"key": "{{feature.key}}"}` (`POST /rest/api/2/issue`, plain
text `description`). Project key: `git config ai-gent.jira.project`, else
ask. States: `GET /rest/api/2/issue/<key>/transitions` and pick the
transition whose target status's `statusCategory.key` is `new` (todo),
`indeterminate` (doing; a status named like "In Review" for review when
the workflow has one), or `done`. Ref: the issue key (`KEY-12`).

**Forgejo / Gitea** — feature and story: issues; link each story to the
feature as a dependency (`POST …/issues/<feature n>/dependencies`, no
sub-issues). Open for `todo`/`doing`/`review`; `done` is `PATCH …/issues/<n>`
`{"state":"closed"}`. Ref: `#<n>`.

**Bitbucket issues** — no hierarchy: the story body names the feature
item. States: `new` (todo), `open` (doing, review), `resolved` (done).
Ref: `#<id>`. When the team uses Jira, sync to Jira instead.

## The batch

`reporun batch FILE` runs steps in order, stops at the first failure, and
asks once (`--write`, confirmed by the `repo` guard) for the whole set:

```json
{
  "platform": "github",
  "steps": [
    {"name": "feature",
     "api": {"method": "POST", "path": "/repos/{owner}/{repo}/issues",
             "data": {"title": "[012] CSV export", "body": "…"}},
     "record": {"key": "feature", "ref": "#{{self.number}}", "id": "{{self.id}}",
                "url": "{{self.html_url}}", "state": "todo", "hash": "3f9a1c77e0"}},
    {"name": "US1",
     "api": {"method": "POST", "path": "/repos/{owner}/{repo}/issues",
             "data": {"title": "[012] US1: Export a timesheet", "body": "…"}},
     "record": {"key": "US1", "ref": "#{{self.number}}", "id": "{{self.id}}",
                "url": "{{self.html_url}}", "state": "doing", "hash": "…"}},
    {"name": "US1-parent",
     "api": {"method": "POST", "path": "/repos/{owner}/{repo}/issues/{{feature.number}}/sub_issues",
             "data": {"sub_issue_id": "{{US1.id}}"}}}
  ]
}
```

- `{owner}`-style placeholders come from the remote; `{{step.field}}`
  from an earlier step's response (a string that is only a reference
  keeps its type — ids stay numbers); `{{self.field}}` in `record` is
  this step's response.
- Steps can also be `"cli": [...]` (the platform's CLI, output captured
  as `stdout`), but API steps return structured results — prefer them
  in batches.
- `--dry-run` lists the steps; `--out FILE` keeps the result, whose
  `records` feed `specsync record`. A failed batch still reports what it
  created: record that, fix the cause, and the next plan picks up the
  rest — never re-create.

## tracker.md

`specs/NNN-<feature>/tracker.md`, written only by `specsync record`:

```markdown
- Platform: github (o/r)
- Last sync: 2026-10-16

| Key | Item | Id | State | Hash | URL |
|---|---|---|---|---|---|
| feature | #120 | 2345678 | doing | 1a2b3c4d5e | https://github.com/o/r/issues/120 |
| US1 | #121 | 2345679 | review | 9f8e7d6c5b | https://github.com/o/r/issues/121 |
```

`Item` is the ref commits and pull requests use; `Hash` is what the body
was when last written, so an unchanged story isn't touched. Commit it
with the stage's work (`docs(specs): sync 012 with the tracker`).

## Never

- Delete an item, or close one the specs don't say is done. An item whose
  story left the specs is an **orphan**: report it and ask.
- Sync a feature the user didn't ask to track, or create items for every
  feature of an existing project in one go without showing the list.
- Reopen an item someone closed on the tracker without asking: read the
  remote state first; a remote `done` that the specs don't agree with is
  drift to report.
