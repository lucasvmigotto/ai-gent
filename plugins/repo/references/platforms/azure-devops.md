# Azure DevOps (Boards and Repos)

## Token — `ai-gent/azure-devops/pat`

A personal access token (User settings → Personal access tokens) for
**one organization**, with custom scopes and the shortest expiry that
works:

| Scope | Read only | To create, update, complete |
|---|---|---|
| Work Items | Read | Read & write |
| Code (pull requests) | Read | Read & write |
| Build (checks, runs) | Read | Read |

`pass insert -m ai-gent/azure-devops/pat`. The runner sends it as Basic
auth with an empty user. The organization and project come from the
remote (`dev.azure.com/<org>/<project>/_git/<repo>`,
`<org>.visualstudio.com`, `ssh.dev.azure.com:v3/…`); for a repository
hosted elsewhere, `git config ai-gent.azure-devops.org <org>` and name the
project in the path.

## CLI (`az` + the `azure-devops` extension)

`detect` says whether the extension is installed; if not, use REST (don't
install it unasked). `az devops` commands detect the organization and
project from the git remote (`--detect true`, the default); pass
`--org https://dev.azure.com/<org> --project <p>` when they can't.

| Need | Command (after `reporun cli azure-devops --`) |
|---|---|
| Query work items | `boards query --wiql "SELECT [System.Id],[System.Title],[System.State] FROM WorkItems WHERE [System.TeamProject] = @project AND [System.AssignedTo] = @me AND [System.State] <> 'Closed' ORDER BY [System.ChangedDate] DESC"` |
| One work item | `boards work-item show --id <n> --expand relations` |
| Create | `boards work-item create --type "User Story" --title "<t>" --description "<html>" [--area <path>] [--iteration <path>] [--assigned-to <who>]` ✎ |
| Update / comment | `boards work-item update --id <n> --state Active [--discussion "<comment>"] [--fields "Microsoft.VSTS.Scheduling.StoryPoints=3"]` ✎ |
| Link | `boards work-item relation add --id <n> --relation-type parent --target-id <m>` ✎ |
| PRs | `repos pr list --status active --repository <repo>` |
| One PR, its policies (checks) | `repos pr show --id <n>`, `repos pr policy list --id <n>` |
| Open a PR | `repos pr create --source-branch <h> --target-branch <b> --title "<t>" --description "<d>" [--work-items <n>] [--draft]` ✎ |
| Vote / complete | `repos pr set-vote --id <n> --vote approve\|reject\|wait-for-author`, `repos pr update --id <n> --status completed --squash false --delete-source-branch true` ✎ |

PR comment threads have no `az repos` command: use REST.

## REST (`reporun api azure-devops …`)

`api-version=7.1` is added unless you pass one; the comments API is still
a preview (`--query api-version=7.1-preview.4`).

| Need | Request |
|---|---|
| Query (WIQL — a read) | `POST /{project}/_apis/wit/wiql` `{"query": "SELECT [System.Id] FROM WorkItems WHERE …"}` → ids |
| Work items by id | `GET /_apis/wit/workitems --query ids=1,2,3 --query 'fields=System.Title,System.State,System.AssignedTo'` |
| One, with links | `GET /{project}/_apis/wit/workitems/<n> --query '$expand=relations'` |
| Create | `POST '/{project}/_apis/wit/workitems/$User%20Story'` with a JSON Patch body `[{"op":"add","path":"/fields/System.Title","value":"<t>"}]` ✎ |
| Update / state | `PATCH /_apis/wit/workitems/<n>` `[{"op":"add","path":"/fields/System.State","value":"Resolved"}]` ✎ (sent as `application/json-patch+json`) |
| Comment | `POST /{project}/_apis/wit/workItems/<n>/comments --query api-version=7.1-preview.4` `{"text":"<html>"}` ✎ |
| PRs | `GET /{project}/_apis/git/repositories/{repo}/pullrequests --query searchCriteria.status=active` |
| One PR, its threads | `GET …/pullrequests/<n>`, `GET …/pullRequests/<n>/threads` |
| Open / comment | `POST …/pullrequests` `{"sourceRefName":"refs/heads/<h>","targetRefName":"refs/heads/<b>","title","description","workItemRefs":[{"id":"<n>"}]}`, `POST …/pullRequests/<n>/threads` `{"comments":[{"content":"<t>"}],"status":"active"}` ✎ |
| Complete | `PATCH …/pullrequests/<n>` `{"status":"completed","lastMergeSourceCommit":{"commitId":"<sha>"},"completionOptions":{"mergeStrategy":"noFastForward","deleteSourceBranch":true}}` ✎ |

Work items link from commits and PRs with `AB#<n>` (GitHub-hosted code)
or `#<n>` mentions in Azure Repos; resolving them on completion is a PR
setting of the organization.
