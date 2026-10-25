# reporun — the path to code platforms

Every `repo:*` skill reaches GitHub, Azure DevOps, GitLab, Bitbucket and
Jira through `scripts/reporun.py` (Python 3.9+, stdlib only):

```bash
REPORUN="python3 <plugin>/scripts/reporun.py"     # <plugin> = this plugin's directory
```

## Commands

| Command | What it does |
|---|---|
| `detect [--json]` | the remote's platform and coordinates (owner/repo, org/project, namespace, workspace), whether the CLI is installed and signed in, and whether the token entry is in pass — nothing decrypted. Jira shows up when `ai-gent.jira.url` is set or its entry exists |
| `cli <platform> [--write] [--entry E] -- <args>` | run **that platform's own CLI** (`gh`, `az`, `glab`, `jira`) with the token from pass in its environment only; output redacted |
| `api <platform> METHOD PATH [--data FILE\|-] [--query K=V]… [--write] [--paginate] [--max-pages N] [--entry E]` | call the REST API: the runner builds the auth header; JSON out; errors redacted |
| `project-status NUMBER --url URL (--state todo\|doing\|review\|done \| --value NAME) [--owner LOGIN] [--field NAME] [--write] [--entry E]` | GitHub Projects v2 only: read a board's single-select field (default `Status`), map a sync state to the live option that matches it, and set that item's field — see *Projects v2* below |

`<platform>` is `github`, `azure-devops`, `gitlab`, `bitbucket`, `forgejo` (Forgejo, Gitea, Codeberg) or `jira`.

## CLI first, REST as the fallback

1. Run `detect`. When the platform's CLI is installed, use `cli`; when it
   isn't (and always for Bitbucket, which has no official CLI), use
   `api`. Don't install a CLI unasked.
2. **Token:** the pass entry `ai-gent/<platform>/<name>` when it exists
   (`shared/secrets.md`, linked here as `secrets.md`). With no entry, `cli`
   falls back to the CLI's own login, and `api` works without a token only
   for public GitHub data. Missing both, give the user the
   `pass insert -m <entry>` to run in their terminal (`secrets:pass`).
3. `--entry` picks another entry under the prefix (a second GitLab host,
   a second Jira site); `ai-gent/db/` entries are refused — they are
   `dbrun`'s.

## Reads and writes

- **Reads run freely**: list, view, show, status, checks, diff, search,
  query, and `GET`. A `POST` that only queries counts as a read: GitHub
  and GitLab GraphQL without `mutation`, Azure Boards WIQL, Jira search.
- **Everything else is a write** — create, comment, edit, label, assign,
  transition, approve, merge, close, re-run — and unknown subcommands are
  writes too. A write needs `--write`, which reporun requires and the
  `repo` guard turns into a question for the user. Pass it only when the
  user asked for **that specific change** in this conversation
  (`git:workflow`: never change a remote issue or open a PR unasked).
- Put a CLI's options **after** its subcommand (`gh pr list -R o/r`, not
  `gh -R o/r pr list`); options first are treated as a write.

## What reporun refuses

- CLI subcommands that print a token, run arbitrary code or change the
  CLI's own login: `gh auth|alias|extension|secret|config|codespace`,
  `glab auth|alias|config|variable|token`,
  `az login|logout|account|extension|config`, `jira init`; and
  `--debug` / `--show-token` (they can log the auth header).
- An absolute URL on another host than the platform's API (pagination
  links included): the token goes nowhere else.
- A `pass` entry outside the prefix, or under `db/`.

## Paths

`api` paths are relative to the platform's API base and may use
placeholders filled from the remote: `{owner}`, `{repo}` (GitHub),
`{org}`, `{project}` (Azure DevOps), `{project_path}` (GitLab, URL-encoded
as the API wants), `{workspace}` (Bitbucket), `{owner}`, `{repo}` (Forgejo). Single-quote paths with `$`
in them (`'/{project}/_apis/wit/workitems/$Bug'`).

| Platform | Base | Auth the runner sends |
|---|---|---|
| GitHub | `https://api.github.com`; Enterprise `https://<host>/api/v3`, `https://api.<x>.ghe.com` | `Bearer <token>` |
| Azure DevOps | `https://dev.azure.com/<org>` (from the remote, or `git config ai-gent.azure-devops.org`); `api-version=7.1` added unless given | `Basic :<pat>` |
| GitLab | `https://<host>/api/v4` | `Bearer <token>` |
| Bitbucket Cloud | `https://api.bitbucket.org/2.0` | `Basic <email>:<token>` with an `email:` line in the entry (Atlassian API token), else `Bearer` (access token) |
| Forgejo / Gitea | `https://<host>/api/v1` | `token <pat>` |
| Jira | `git config ai-gent.jira.url`, `JIRA_URL`, or a `url:` line in the entry | `Basic <email>:<token>` with an `email:` line (Cloud), else `Bearer` (Server/Data Center PAT) |

`--paginate` follows `Link: rel="next"` (GitHub, GitLab) and Bitbucket's
`next`, merging `values`/`value` arrays, up to `--max-pages` (10). Azure
DevOps and Jira page by parameters (`$top`/`$skip`, continuation tokens,
`startAt`/`nextPageToken`): pass them with `--query`.

## Projects v2 (GitHub)

GitHub's REST API has no Projects v2 support — it's GraphQL-only. Rather
than hand-roll GraphQL, `project-status` shells out to `gh project`
(`field-list`, `item-edit`), which resolves field and option names to
their live node IDs itself, right before the mutation it sends — the
"look it up live" that `sync.md`'s Projects v2 mapping needs. There's no
REST fallback and no `api github POST /graphql` path for this: without
`gh` signed in with the `project` scope (`gh auth refresh -s project`),
`project-status` has nothing to call.

A project's `NUMBER` and whether it's owned by a user or an org
(`--owner LOGIN`) come from `gh project list --owner <owner>` (a plain
read, run with `cli github -- project list --owner <owner>`); with no
`--owner`, `project-status` uses the remote's owner, which is right only
when the board is owned by the repository's own owner. `project-status`
maps a sync state (`sync.md` → *States*) to whichever live option on the
board's field (default `Status`) matches it by alias, so it works
whether the board calls it "To do" or "Backlog"; when nothing matches,
it lists the board's real options and asks for `--value <exact name>`
instead of guessing one.

## Settings that aren't secrets

Kept in git config, never in a file of ai-gent's: `ai-gent.platform`
(a self-hosted GitLab whose host doesn't say "gitlab", or Forgejo-like
hosts you mean as one of the five), `ai-gent.azure-devops.org`,
`ai-gent.jira.url` (usually `--global`). Entry notes (`email:`, `url:`,
`org:` lines after the token) work too.

Platform specifics — the token to create, CLI commands and REST endpoints
for issues, work items and pull requests — are in `platforms/<name>.md`.
