# Data model — repo hygiene

| Entity | Fields | Source |
|---|---|---|
| Branch state | name (`main` · `master` · `dev` · `develop`), remote (`origin/<name>`), ahead, behind, releases[] (versions from release subjects behind), diverged (ahead > 0 and behind > 0) | `git rev-list --left-right --count`, `git log --format=%s` |
| Hygiene | branches[], fetchedAgoMs (null when `FETCH_HEAD` is missing), checkedAt, isRepo | the band's one drawn value |
| Session note | source (`update-check` · `leftovers`), lines[] | `classic.SessionStart` `additionalContext` lines starting `[ai-gent] update available:` or under `ai-gent containers (report only, nothing removed):` |
| Release preview | exitCode, stdout, stderr | `python3 scripts/release.py --dry-run` (exit 0 plan, 3 nothing to release, 1 error, 2 bad arguments) |

## State atoms

| Key | Plugin | Type | Purpose |
|---|---|---|---|
| `hygiene.state` | git | `Shaped<Hygiene>` (tag `hygiene-1`) | what the band draws |
| `hygiene.toasted` | git | boolean | update toast once per session |
| `leftovers.toasted` | devcontainer | boolean | leftovers toast once per session |

## Band text

```
main ↓1 (release 2.20.0) · dev ↓1 — git pull --ff-only on both before the next commit
dev diverged from origin/dev (↑2 ↓1) — see git:workflow
                                       as of the last fetch, 3 days ago
```
