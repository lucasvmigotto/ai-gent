Reconstructed by project:introspec on 2026-10-18 from 384c771

# Data model — pass entries

| Entity | Shape | Evidence |
|---|---|---|
| Store | `PASSWORD_STORE_DIR`, default `~/.password-store`; one `<entry>.gpg` file per entry | `shared/passstore.py:28-29,38-40` |
| Entry name | `<prefix>/<area>/<name>`, prefix `AI_GENT_PASS_PREFIX` (default `ai-gent`); matches `[A-Za-z0-9_@+][A-Za-z0-9._@+-]*(/…)*`, no `..` segment | `passstore.py:17,24-25,32-35` |
| Entry content | line 1: the secret; lines 2+: notes (`email:`, `username:`, `url:` …) | `passstore.py:43-59` |

Areas in use: `db/<profile or family>` (dbrun), `github/pat`, `gitlab/pat`, `azure-devops/pat`, `bitbucket/token`, `jira/token` (reporun) [OBSERVED: plugins/db/scripts/dbrun.py:299-300; plugins/repo/scripts/reporun.py:52-58].
