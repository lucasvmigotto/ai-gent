# Bitbucket Cloud

No official CLI: always `reporun api bitbucket …`. Bitbucket Server/Data
Center (`/rest/api/1.0`) is a different API and isn't covered.

## Token — `ai-gent/bitbucket/token`

Either:

- a **repository, project or workspace access token** (Repository
  settings → Access tokens) — the token alone; sent as `Bearer`; or
- an **Atlassian API token with scopes** (id.atlassian.com → Security →
  API tokens) — the token, then an `email: <your Atlassian email>` line;
  sent as Basic `email:token`.

App passwords are deprecated; don't create new ones. Scopes:
`read:pullrequest:bitbucket` / `read:repository:bitbucket` to read, plus
`write:pullrequest:bitbucket` to comment, approve and merge, and
`read:issue:bitbucket` / `write:issue:bitbucket` for the issue tracker.
(Access tokens use the short names: `pullrequest`, `pullrequest:write`,
`issue`, `issue:write`.)

```bash
pass insert -m ai-gent/bitbucket/token     # token, then: email: me@example.com
```

## REST (`reporun api bitbucket …`)

`{workspace}` and `{repo}` come from the remote. Filters use BBQL in `q`
(`--query 'q=state="OPEN"'`); lists page with `next`, so add
`--paginate`.

| Need | Request |
|---|---|
| PRs | `GET /repositories/{workspace}/{repo}/pullrequests --query state=OPEN` |
| One PR, its diff, its comments | `GET …/pullrequests/<id>`, `GET …/pullrequests/<id>/diff`, `GET …/pullrequests/<id>/comments` |
| Its build statuses | `GET …/pullrequests/<id>/statuses` |
| Open | `POST …/pullrequests` `{"title","description","source":{"branch":{"name":"<h>"}},"destination":{"branch":{"name":"<b>"}},"close_source_branch":true}` ✎ |
| Comment / approve / merge | `POST …/pullrequests/<id>/comments` `{"content":{"raw":"<t>"}}`, `POST …/pullrequests/<id>/approve`, `POST …/pullrequests/<id>/merge` `{"merge_strategy":"merge_commit"\|"squash"\|"fast_forward","close_source_branch":true}` ✎ |
| Issues (when the tracker is on) | `GET /repositories/{workspace}/{repo}/issues --query 'q=state="new" OR state="open"'`, `POST …/issues` `{"title","content":{"raw":"<d>"}}`, `POST …/issues/<id>/comments` ✎ |

Most Bitbucket teams track work in **Jira** (`jira.md`); the repository's
issue tracker is often disabled (a 404 on `/issues`).
