# Contract: guard fixtures

Directory `shared/guard-fixtures/`: `git.json`, `db.json`, `secrets.json`, `repo.json`, `containers.json`, `adapter.json`, plus `README.md` (this contract in short) and `manifest.json` (counts per file, the source of each case, the extraction commit).

Why JSON: every reader already exists in the repo or the toolchain (`python3` in the shell tests, `node` in the opencode test, `serde_json` in Rust); a multi-line heredoc is one JSON string. TOML was considered (nicer multi-line strings) and rejected because `tomllib` needs Python 3.11 or later and the macOS CI job runs the system tools.

```json
{
  "schema": 1,
  "guard": "git",
  "marker": "git:workflow guard:",
  "sandbox": { "repos": { "main": "main", "staging": "staging", "dev": "dev", "feat": "feat/x" } },
  "cases": [
    {
      "id": "git-001",
      "tool": "Bash",
      "cwd": "@feat",
      "shell": "sh",
      "command": "git commit --no-verify -m \"fix: x\"",
      "want": "deny",
      "reason": "git:workflow guard: hooks must not be bypassed (--no-verify / -n); fix what the hook reports and try again",
      "source": ["scripts/test-guard.sh", "opencode/guard/test/rules.test.mjs:gitVectors"],
      "tags": ["parity"],
      "variants": {
        "powershell": "git commit --no-verify -m 'fix: x'",
        "cmd": "git commit --no-verify -m \"fix: x\""
      }
    },
    {
      "id": "db-cred-read-001",
      "tool": "Read",
      "cwd": "/tmp",
      "tool_input": { "file_path": "/home/me/.config/ai-gent/db/<credentials file>" },
      "want": "deny",
      "reason": "db guard: <the credentials reason, exact text copied at extraction>",
      "source": ["scripts/test-db-guard.sh"]
    }
  ]
}
```

## Fields

| Field | Required | Meaning |
|---|---|---|
| `id` | yes | unique, stable (`<guard>-NNN`), never reused |
| `tool` | yes | `Bash`, `Read`, `Edit`, ... |
| `command` | for shells | the decoded command string (heredocs as `\n`) |
| `tool_input` | for non-shells | object passed as `tool_input` (exclusive with `command`) |
| `cwd` | yes | a path, or `@main`, `@staging`, `@dev`, `@feat` meaning the sandbox repo on that branch (harnesses create them with `git init -b`) |
| `shell` | no | `sh` (default), `powershell`, `cmd` |
| `want` | yes | `deny`, `ask`, `none` |
| `reason` | for deny and ask | the exact reason; may use `{branch}` |
| `source` | yes | file(s) the case came from, or `new:<spec>` |
| `tags` | no | `parity` (oracle-checked), `new` (no oracle; `expected_divergence`), `windows`, `slow` |
| `variants` | no | the same intent in other syntaxes; each inherits `want` and `reason` |
| `expected_divergence` | no | true when the oracle is known to differ (only with tag `new`) |
| `skip_in` | no | list of harnesses (`sh`, `mjs`, `rust`) that must skip it, with `skip_reason` |

## Harnesses

- `scripts/test-guard.sh` and its four siblings keep their names (so `scripts/check.sh` is unchanged) and become thin loaders calling `python3 scripts/guard-fixtures.py run --guard git --impl sh`, which prints `      ok    <want> <command>` or `FAIL` lines in the current format (`check.sh` counts `' ok '`).
- `opencode/guard/test/rules.test.mjs` reads the same files until it is replaced by a binary-backed test.
- Rust: `rust/crates/ai-gent-core/tests/guard_parity.rs` loads them by relative path from the repository and skips with a message when the directory is absent (the published crate's own tests).
- `--impl bin` runs the same cases through `ai-gent guard` as a subprocess (the path the hooks use).

Cases tagged `parity` must pass in every harness; `new` cases run only in Rust and `bin`.
