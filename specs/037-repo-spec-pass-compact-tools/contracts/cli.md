# CLI contract: `ai-gent repo`, `ai-gent spec`, `ai-gent compact`

Baselines: `reporun.py` docstring (lines 1-26) and `build_parser` (lines 832-884); `specsync.py` docstring and `main` (lines 413-438); `compact.py` docstring and `main` (lines 1197-1230). The pass-store module has no CLI.

## Exit codes

| Tool | 0 | 1 | 2 | 3 | 4 |
|---|---|---|---|---|---|
| repo | ok | error (`Failed`) | usage | refused (`Refused`): write without `--write`, blocked CLI verb, host not allowed | n/a |
| spec | ok | `Failed` (message on stderr) | usage | n/a | n/a |
| compact | ok | usage or I/O error | n/a in the binary (was: missing Pygments) | n/a | n/a |

## `ai-gent repo`

| Command | Flags | Output |
|---|---|---|
| `repo detect` | `--json` | platform, CLI state, token state per platform (entry present or not); never decrypts |
| `repo cli PLATFORM -- ARGS...` | `--write` | the platform CLI's output, redacted; exit code of the child mapped to 1 if non-zero [TO CAPTURE: confirm in `cmd_cli`] |
| `repo api PLATFORM METHOD PATH` | `--data FILE\|-`, `--query k=v` (repeat), `--write`, `--paginate`, `--max-pages N` | response body (JSON pretty or raw), redacted |
| `repo project-status NUMBER` | `--url URL` (required), `--state todo\|doing\|review\|done` or `--value NAME`, `--owner LOGIN`, `--field NAME`, `--write` | the field read, or the item set |
| `repo batch FILE` | `--write`, `--dry-run`, `--out FILE` | per-step results; `--out` writes `records` for `spec record` |
| `repo chain` | `--json` | branches: work, integration, staging?, production |

Platforms: `github`, `azure-devops`, `gitlab`, `bitbucket`, `forgejo`, `jira`. `cli` is unavailable for `bitbucket` and `forgejo`.

Batch plan file (JSON): a list of steps `{name, platform, method, path, data?, kind?}`; later steps reference earlier results as `{results.<name>.<field>}`; format fixed by `specs/009-repo-runner/contracts/` and `plugins/repo/references/sync.md` (the Rust port reads those, it does not redefine them).

Environment: `PASSWORD_STORE_DIR`, `AI_GENT_PASS_PREFIX` (default `ai-gent`), `CONTAINER_ENGINE` is not used. Secrets: entries `ai-gent/<platform>/<name>` as `shared/secrets.md`; notes `email:`, `url:`, `org:` read with the secret.

## `ai-gent spec`

| Command | Flags | Output |
|---|---|---|
| `spec plan TARGET` | `--json` | the feature's items and actions; TARGET is `NNN` or `specs/NNN-feature` |
| `spec record TARGET` | `--platform P` (required), `--from FILE` (required, a batch output) | writes `specs/NNN-*/tracker.md` |
| `spec features` | `--json` | every feature's state and pending changes |

States: `todo, doing, review, done`. Marker: `<!-- ai-gent:sync {path} {key} -->`.

## `ai-gent compact`

Flags as the Python parser: `PATH...` (files, directories, `-`), `-l/--lang`, `-L/--level 0..4` (default 3), `-p/--paragraphs`, `-c/--strip-comments`, `--outline DEPTH`, `-n/--numbers`, `-w/--wrap N` (default 1500), `-o/--output FILE`, `-q/--quiet`, `--no-verify`, `--no-ast`, `--list-languages`. `--no-ast` keeps its meaning (skip the grammar check); with the tree-sitter tokenizer there is no lexer-only mode for languages that have a grammar, so `--no-verify` is the only way to skip checking [decision at T061]. stderr carries per-file summaries and `fallback[...]` notes; stdout carries the code only.

`--list-languages` columns (new): language, extensions, ceiling level, join rule, tokenizer (`tree-sitter:<name>` or `none`).
