# Data model: guard evaluator

| Entity | Where | Fields |
|---|---|---|
| Event | stdin (claude format) | `hook_event_name` (`PreToolUse`), `tool_name`, `cwd`, `tool_input` (object: `command` for shells; `file_path`, `notebook_path`, `path`, `glob`, `pattern` for the db credentials check) |
| Decision | stdout | `decision` in `deny`, `ask`, `none`; `reason` (marker + text); `rule` id (internal, shadow log only) |
| Segment | in memory | `text` (quotes blanked), `words` (for rules that look at command words), `raw_index` |
| Reason entry | `rust/crates/ai-gent-core/src/guard/reasons.json` | `id`, `guard`, `marker`, `text`, `decision`; `text` may contain `{branch}` or `{tool}` placeholders |
| Fixture case | `shared/guard-fixtures/<guard>.json` | see `contracts/fixture-format.md` |
| Shadow record | `guard-shadow.jsonl` | see `contracts/shadow-log.md` |
| Bench result | stdout of `scripts/bench-guard.py` (and `--json`) | `os`, `impl` (`sh` or `bin`), `case`, `n`, `p50_ms`, `p95_ms`, `p99_ms` |

## Guard ids and markers

| Guard id | Plugin | Marker (prefix of every reason) | Tools |
|---|---|---|---|
| `git` | `git` | `git:workflow guard:` | Bash |
| `db` | `db` | `db guard:` | Bash, Read, Grep, Glob, Edit, Write, MultiEdit, NotebookEdit |
| `secrets` | `secrets` | `secrets guard:` | Bash |
| `repo` | `repo` | `repo guard:` | Bash |
| `containers` | `devcontainer` | `containers guard:` | Bash |

## Decision precedence for `--guard all`

deny beats ask beats none; within a level the first in the order git, db (shell), db credentials, secrets, repo, containers (the order of `evaluate()` in `opencode/guard/rules.mjs`).
