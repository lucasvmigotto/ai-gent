# GitHub

## Token — `ai-gent/github/pat`

A **fine-grained** personal access token (Settings → Developer settings →
Personal access tokens → Fine-grained), limited to the repositories it
serves, with the shortest expiry that works:

| Permission | Read only | To create, comment, merge |
|---|---|---|
| Issues | Read | Read and write |
| Pull requests | Read | Read and write |
| Contents | Read | Read and write (merging) |
| Actions, Commit statuses | Read | Read |

`pass insert -m ai-gent/github/pat` — the token, then notes (scopes,
expiry). Without an entry, `reporun cli github` uses `gh`'s own login.
GitHub Enterprise Server needs `GH_ENTERPRISE_TOKEN`, which reporun sets
from the same entry when the remote's host isn't github.com.

## CLI (`gh`)

| Need | Command (after `reporun cli github --`) |
|---|---|
| Find issues | `issue list --search "<words> is:open" --json number,title,state,labels` |
| Read one | `issue view <n> --comments` |
| Create | `issue create --title "<t>" --body-file <file> [--label x] [--assignee @me]` ✎ |
| Comment / edit / close | `issue comment <n> --body-file <f>`, `issue edit <n> --add-label x`, `issue close <n> --reason completed` ✎ |
| PRs | `pr list --state open --json number,title,headRefName,baseRefName,isDraft` |
| One PR, its checks, its diff | `pr view <n> --json title,body,state,reviewDecision,mergeable,statusCheckRollup`, `pr checks <n>`, `pr diff <n>` |
| Review comments | `api repos/{owner}/{repo}/pulls/<n>/comments` |
| Open a PR | `pr create --base <b> --head <h> --title "<t>" --body-file <f> [--draft]` ✎ |
| Review / merge | `pr review <n> --approve\|--request-changes\|--comment --body "<t>"`, `pr merge <n> --merge\|--squash\|--rebase --delete-branch` ✎ |

✎ = a write: `reporun cli github --write -- …`, only when the user asked.

## REST (`reporun api github …`)

| Need | Request |
|---|---|
| Issues | `GET /repos/{owner}/{repo}/issues --query state=open` (PRs come back too; they have a `pull_request` key) |
| Search | `GET /search/issues --query 'q=repo:<o>/<r> is:issue is:open <words>'` |
| One issue and its comments | `GET /repos/{owner}/{repo}/issues/<n>`, `GET …/issues/<n>/comments` |
| Create / comment / update | `POST /repos/{owner}/{repo}/issues` `{"title","body","labels"}`, `POST …/issues/<n>/comments` `{"body"}`, `PATCH …/issues/<n>` `{"state":"closed","state_reason":"completed"}` ✎ |
| PRs | `GET /repos/{owner}/{repo}/pulls --query state=open`, `GET …/pulls/<n>`, `GET …/pulls/<n>/comments` |
| Checks for a PR's head | `GET /repos/{owner}/{repo}/commits/<sha>/check-runs` |
| Open / review / merge | `POST …/pulls` `{"title","head","base","body","draft"}`, `POST …/pulls/<n>/reviews` `{"event":"APPROVE"\|"REQUEST_CHANGES"\|"COMMENT","body"}`, `PUT …/pulls/<n>/merge` `{"merge_method":"merge"\|"squash"\|"rebase"}` ✎ |

Closing keywords (`Closes #N`) in a PR body close the issue when the PR
merges into the **default branch** only.
