# Data Model: Repo, spec, pass-store and compact tools

## Pass entry

`ai-gent/<area>/<name>` (prefix replaceable by `AI_GENT_PASS_PREFIX`); name regex `[A-Za-z0-9_@+][A-Za-z0-9._@+-]*` per segment, no `..`, no leading `-` or `/`. First line is the secret; later lines are notes: `key: value` with key `[A-Za-z][A-Za-z0-9_-]*`, lowercased. Store: `PASSWORD_STORE_DIR` or `~/.password-store`; presence check is `<store>/<entry>.gpg`. Explicit reference form `pass:<entry>` is accepted wherever a tool's configuration names a secret.

## Platform (repo)

`github` (cli `gh`, entry `github/pat`, env `GH_TOKEN`), `azure-devops` (`az`, `azure-devops/pat`, `AZURE_DEVOPS_EXT_PAT`), `gitlab` (`glab`, `gitlab/pat`, `GITLAB_TOKEN`), `bitbucket` (no cli, `bitbucket/token`), `forgejo` (no cli, `forgejo/pat`), `jira` (`jira`, `jira/token`, `JIRA_API_TOKEN`). READ_VERBS and BLOCKED_CLI sets copied as data from `reporun.py` lines 62-90.

## Remote facts

From `git remote` / `git config`: platform, host, owner/org, project, repo, plus forced overrides; Azure DevOps `org:` and Jira `url:`/`email:` from notes. Used to fill `{owner}`, `{repo}`, `{project}` placeholders in API paths.

## Batch plan and result

Step: `{name, platform, method, path, data?}`; result keyed by name; references `{results.<name>.<dotted.field>}` resolved by `lookup`. `--out` file: the records `spec record` reads (platform item ids and URLs per step). Exact schemas: `specs/009-repo-runner/contracts/` and `plugins/repo/references/sync.md` [not re-read here; T020/T026 copy them into fixtures].

## Spec Kit items (spec)

Feature item (title from the feature directory, body from spec.md summary, state from `specs/README.md`), Story item (`### User Story N - Title (Priority: Pn)`, tasks as a checklist `- [ ] T001 ...`, state from checked tasks), each with a marker and a digest; `tracker.md` stores platform, location and per-item `{id, url, digest}`. States: todo, doing, review, done. Status words map: planned to todo; in progress and partial to doing; implemented to review; verified to done; the least advanced side wins.

## Compact language entry

`name`, `lexer` (dropped), `exts`, `allowed` flags subset of `{blank, indent, join, spaces}`, `join` rule (`free, go, js, kotlin, css, none`), `preproc`, `ts` grammar name (or none), `hazard` regex with `hazard_allowed` flags, `indent_sensitive`, `protect` regex, `names` (file names such as `Makefile`). Levels: 0 none, 1 blank, 2 blank+indent, 3 blank+indent+join, 4 all four. Fallback order when verification fails: spaces, join, indent, blank.
