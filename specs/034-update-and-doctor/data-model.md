# Data model: update and doctor

| Entity | Where | Fields |
|---|---|---|
| Install root | `<home>` = `$AI_GENT_HOME` or default | `<ver>/` directories, `current`, `previous` (symlinks to a version name, relative), `pinned` (file containing the pinned version), `.stage.*` (temporary, installer only) |
| Version directory | `<home>/<ver>/` | `bin/ai-gent[.exe]`, `VERSION`, `MANIFEST`, `setup.sh`, `install.sh`, `plugins/`, ... (032 contracts) |
| Update cache | `${XDG_STATE_HOME:-~/.local/state}/ai-gent/update-check` | line: `<epoch-seconds> <X.Y.Z>`; optional third field `<YYYY-MM-DD>` of the last notice (open question 1); the shell script reads the first two and ignores a third |
| Pin | `<home>/pinned` | the pinned `X.Y.Z` plus newline; present means "stay silent and do not update unless asked" |
| Release source | env | `AI_GENT_RELEASE_BASE` (file:// or https base containing `<ver>/` and `LATEST`), `AI_GENT_UPDATE_REPO` (owner/repo, default `lucasvmigotto/ai-gent`), default `https://github.com/<slug>/releases/latest` redirect |
| Doctor result | stdout | list of `{id, status, detail, hint}`; `status` in `ok`, `warn`, `fail`, `skip` |

## State transitions of the install root

```
(nothing) --install--> <v1>, current=v1
current=v1 --update--> <v1>,<v2>, current=v2, previous=v1
current=v2 --rollback--> current=v1, previous=v2
update --to X.Y.Z  : pinned=X.Y.Z      update --to latest : pinned removed
```

All transitions are performed by `install.sh` (FR-006); the binary only reads the tree, except it writes the cache.

## Version comparison

Numeric on the three fields, as `_ai_gent_ver_gt` in `update-check.sh` (missing parts are 0, a non-numeric suffix is ignored). Pre-release suffixes do not exist in this project's tags.
