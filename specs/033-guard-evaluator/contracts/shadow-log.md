# Contract: shadow log

Path: `${XDG_STATE_HOME:-~/.local/state}/ai-gent/guard-shadow.jsonl`; Windows `%LOCALAPPDATA%\ai-gent\state\guard-shadow.jsonl`. Directory 0700, file 0600 (Unix). One JSON object per line, one `write` per line. Rotation: when the file exceeds 5 MiB the next write renames it to `guard-shadow.1.jsonl` (one generation kept).

```json
{
  "schema": 1,
  "ts": "2026-11-01T12:00:00Z",
  "release": "2.27.0",
  "target": "x86_64-unknown-linux-musl",
  "guard": "git",
  "tool": "Bash",
  "shell": "sh",
  "shell_decision": "ask",
  "rust_decision": "ask",
  "agree": true,
  "reason_equal": true,
  "expected_divergence": false,
  "rule": "git.push",
  "cmd_sha": "9f2c4a1b7d3e8a10",
  "cmd_len": 24,
  "verb": "git push",
  "latency_us": 1430,
  "capture": null
}
```

- `cmd_sha`: first 16 hex of sha256 of the decoded command. `verb`: only when the first two words match `^[a-z][a-z0-9-]*$` (for example `git push`, `psql`), else `null`. The command text is never stored by default (the same rule as the guard log of spec 024, which records the verb only).
- `capture`: only with `AI_GENT_SHADOW_CAPTURE=1`: the command after redaction with `shared/secret-patterns.json` (matches replaced by `[REDACTED]`), truncated to 2000 characters, written only for records where `agree` is false.
- `agree` compares decisions; `reason_equal` compares reason strings (the shell decision's reason is passed as `--shell-reason` when available, otherwise `null`).
- A disagreement is `agree=false` or `reason_equal=false`; it is *unexplained* when `expected_divergence=false`.
- `guard shadow-report` and the promotion script read only this file.
