# Forgejo, Gitea and Codeberg

No CLI in the runner: always `reporun api forgejo …`. The API is Gitea's
(`/api/v1`), which Forgejo and Codeberg keep.

## Token — `ai-gent/forgejo/pat`

An access token from Settings → Applications. Scopes: `read:issue` and
`read:repository` to read; `write:issue` to create, comment and close;
`write:repository` to open and merge pull requests. The shortest expiry
that works.

```bash
pass insert -m ai-gent/forgejo/pat
```

The host comes from the remote when its name contains `forgejo` or `gitea`,
or is `codeberg.org`; any other host needs `git config ai-gent.platform
forgejo`. The runner sends `Authorization: token <pat>`. For a second
instance, keep its token in another entry (`ai-gent/forgejo/<host>`) and
pass `--entry`.

## REST (`reporun api forgejo …`)

`{owner}` and `{repo}` come from the remote.

| Need | Request |
|---|---|
| Issues | `GET /repos/{owner}/{repo}/issues --query state=open --query 'q=<words>' --query type=issues` |
| One issue, its comments | `GET …/issues/<n>`, `GET …/issues/<n>/comments` |
| Create / comment / close | `POST …/issues` `{"title","body","labels":[<ids>]}`, `POST …/issues/<n>/comments` `{"body"}`, `PATCH …/issues/<n>` `{"state":"closed"}` ✎ |
| Link a story to its feature | `POST …/issues/<feature n>/dependencies` `{"owner","repo","index":<story n>}` (a dependency, since there are no sub-issues) ✎ |
| Pull requests | `GET /repos/{owner}/{repo}/pulls --query state=open` |
| One PR, its reviews | `GET …/pulls/<n>`, `GET …/pulls/<n>/reviews` |
| Checks | `GET …/commits/<sha>/status` (combined commit status) |
| Open / comment / merge | `POST …/pulls` `{"head","base","title","body"}`, `POST …/issues/<n>/comments` `{"body"}`, `POST …/pulls/<n>/merge` `{"Do":"merge","delete_branch_after_merge":true}` ✎ |

`Do` is `merge` (merge commit), `rebase` (rebase, then fast-forward),
`rebase-merge`, `squash` or `fast-forward-only`. Issue labels take ids, not
names (`GET …/labels`). `Closes #N` in a PR description closes the issue
when it merges into the default branch. Pages are 30 by default; use
`--paginate` (the `Link` header) or `--query limit=50`.
