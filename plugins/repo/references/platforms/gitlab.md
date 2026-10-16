# GitLab (gitlab.com and self-managed)

## Token — `ai-gent/gitlab/pat`

A **project** (or group) access token when the work stays in one project;
a personal access token otherwise. Scope `read_api` for reading, `api` to
create, comment and merge; the shortest expiry that works.

`pass insert -m ai-gent/gitlab/pat`. A self-managed host comes from the
remote (reporun sets `GITLAB_HOST` for `glab`); a host whose name doesn't
contain "gitlab" needs `git config ai-gent.platform gitlab`. For a second
host, keep its token in another entry (`ai-gent/gitlab/<host>`) and pass
`--entry`.

## CLI (`glab`)

| Need | Command (after `reporun cli gitlab --`) |
|---|---|
| Find issues | `issue list --search "<words>" [--label x] [--assignee @me]` |
| Read one | `issue view <n> --comments` |
| Create / comment / close | `issue create --title "<t>" --description "<d>"`, `issue note <n> --message "<t>"`, `issue close <n>` ✎ |
| MRs | `mr list [--target-branch <b>]` |
| One MR, its diff, its pipeline | `mr view <n> --comments`, `mr diff <n>`, `ci view <branch>` (or `ci status`) |
| Open an MR | `mr create --source-branch <h> --target-branch <b> --title "<t>" --description "<d>" [--draft]` ✎ |
| Comment / approve / merge | `mr note <n> --message "<t>"`, `mr approve <n>`, `mr merge <n> [--squash] [--remove-source-branch]` ✎ |

## REST (`reporun api gitlab …`)

`{project_path}` is the URL-encoded `group/sub/project` the API takes as
`:id`.

| Need | Request |
|---|---|
| Issues | `GET /projects/{project_path}/issues --query state=opened --query 'search=<words>'` |
| One issue, its notes | `GET /projects/{project_path}/issues/<iid>`, `GET …/issues/<iid>/notes` |
| Create / comment / close | `POST /projects/{project_path}/issues` `{"title","description","labels"}`, `POST …/issues/<iid>/notes` `{"body"}`, `PUT …/issues/<iid>` `{"state_event":"close"}` ✎ |
| MRs | `GET /projects/{project_path}/merge_requests --query state=opened` |
| One MR, its pipelines | `GET …/merge_requests/<iid>`, `GET …/merge_requests/<iid>/pipelines`, `GET …/merge_requests/<iid>/notes` |
| Open / comment / approve / merge | `POST …/merge_requests` `{"source_branch","target_branch","title","description"}`, `POST …/merge_requests/<iid>/notes` `{"body"}`, `POST …/merge_requests/<iid>/approve`, `PUT …/merge_requests/<iid>/merge` `{"squash":false,"should_remove_source_branch":true}` ✎ |

Use the **iid** (the number shown in the project), not the global id.
`Closes #N` in an MR description closes the issue when it merges into the
default branch.
