# CLI contract: `ai-gent wt`

Baseline: `plugins/agents/scripts/wt.py` (docstring lines 2-30, `main` 536-566).

| Command | Flags | Output | Exit |
|---|---|---|---|
| `wt plan` | `--json` | text list of ready/waiting, or `{"ready","blocked","done"}` | 0; 1 if no `specs/README.md` |
| `wt add BRANCH` | `--base BRANCH`, `--min-free-gb N` | JSON object on stdout; guidance line on stderr | 0; 1 on refusal |
| `wt list` | `--json` | table or array of rows | 0 |
| `wt remove BRANCH` | `--keep-containers` | `removed <path> and branch <b>`, then `removed container X` lines | 0; 1 on refusal or leftover |
| `wt baseline` | | `baseline: N external container(s) ...` | 0; 1 no engine |
| `wt resources` | `--json` | report; removes nothing | 0; 1 no engine |
| `wt finish` | | `removed ...` / `left ...` lines | 0; 1 if a removal did not take |

Exit codes: 0 ok, 1 failure with message on stderr, 2 usage (clap). Environment: `CONTAINER_ENGINE`, `AI_GENT_PROJECT`/`AI_GENT_WORKTREE`/`COMPOSE_PROJECT_NAME` are printed for the user to set, never read. Text wording is kept as in Python until a consumer proves it does not parse it (the mods use `--json`).
