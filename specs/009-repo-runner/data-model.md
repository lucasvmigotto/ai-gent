Reconstructed by project:introspec on 2026-10-18 from 384c771

# Data model — reporun

| Entity | Fields | Evidence |
|---|---|---|
| Platform | id ∈ {github, azure-devops, gitlab, bitbucket, jira}; cli (`gh`, `az`, `glab`, none, `jira`); default entry (`github/pat`, `azure-devops/pat`, `gitlab/pat`, `bitbucket/token`, `jira/token`); token env vars | `plugins/repo/scripts/reporun.py:52-58` |
| Repository facts | url, host, platform, owner, repo, org, project, namespace, workspace — parsed from the remote, overridable by `git config ai-gent.platform` | `reporun.py:163-216` |
| Token entry | pass entry `<prefix>/<platform>/<name>`; first line token; notes `email:`/`username:`/`user:`, `url:` | `reporun.py:96-138` |
| Batch plan | `{platform, steps: [{name, api: {method, path, data, query} \| cli: [...], record?}]}` | `reporun.py:562-660` |
| Batch report | `{ok, results{name: …}, records[]}` — records feed `specsync record` | `reporun.py:625-660` |
| Chain | default, integration, staging, production, order[], integrated_on, keywords_close_on, ambiguous[], ask | `reporun.py:665-701` |

No persistence of its own.
