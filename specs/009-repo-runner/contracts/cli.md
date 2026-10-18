Reconstructed by project:introspec on 2026-10-18 from 384c771

# Contract — reporun CLI

[OBSERVED: plugins/repo/scripts/reporun.py:11-16,707-748]. Exit codes: 0 ok, 1 error, 2 usage, 3 refused [OBSERVED: reporun.py:36].

| Command | Arguments | Remote change |
|---|---|---|
| `detect` | `[--json]` | no; nothing decrypted |
| `cli` | `<platform> [--entry E] [--write] -- <cli args…>` | only with `--write` |
| `api` | `<platform> METHOD PATH [--data FILE\|-] [--query k=v …] [--entry E] [--write] [--paginate] [--max-pages 10]` | only with `--write` (non-GET/HEAD, except read-only POSTs) |
| `batch` | `FILE [--write] [--dry-run] [--out FILE]` | only with `--write`, once for the batch |
| `chain` | `[--json]` | no; reads local and remote-tracking refs |

Environment: `AI_GENT_PASS_PREFIX`, `PASSWORD_STORE_DIR`, `AI_GENT_REPO_REMOTE` (default `origin`); git config `ai-gent.platform`, `ai-gent.jira.url` [OBSERVED: reporun.py:218-220,670; shared/passstore.py:24-29].
