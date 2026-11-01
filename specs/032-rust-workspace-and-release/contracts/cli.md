# CLI contract: `ai-gent` (shared by 033-038)

Binary `ai-gent`. This file fixes what every subcommand shares; each spec adds its subcommand's section.

## Global behavior

- `ai-gent [--json] <subcommand> [args]`. `--json` is accepted by every subcommand (D9), before or after the subcommand name, and makes stdout exactly one JSON object followed by a newline; human text then goes to nothing (errors go to stderr as text, and also as `{"schema":1,"ok":false,"error":{...}}` on stdout when `--json` is set).
- `NO_COLOR` and a non-terminal stdout disable color. Output is UTF-8.
- `ai-gent --version` prints one line: `ai-gent <crate-version> (release <release-version>, <triple>)`.
- Environment: `AI_GENT_HOME` (install root), `AI_GENT_NO_UPDATE_CHECK`, `XDG_*` on Unix, `%LOCALAPPDATA%` on Windows.
- A secret value is never printed (shared/secrets.md).

## Exit codes (documented, stable)

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | runtime error (the operation failed) |
| 2 | usage error (bad arguments; clap's default) |
| 3 | nothing to do / not applicable (as `release.py`) |
| 4 | refused by policy or verification (a checksum, an attestation, a guard hard stop) |
| 101 | internal error (a panic); the binary prints the report location and nothing else |

`guard` is the exception: a hook must exit 0 and print its decision; its own contract is in 033.

## JSON envelope

```json
{ "schema": 1, "ok": true, "data": { } }
{ "schema": 1, "ok": false, "error": { "code": "usage|runtime|refused|internal", "message": "text", "hint": "optional next step" } }
```

`schema` is an integer per subcommand output shape; a breaking change to a shape bumps it and is a major-release item (039).

## `ai-gent version [--json]`

Text: `ai-gent 0.1.0 (release 2.27.0, x86_64-unknown-linux-musl)`.

```json
{ "schema": 1, "ok": true, "data": { "crate": "0.1.0", "release": "2.27.0", "target": "x86_64-unknown-linux-musl", "commit": "abc1234", "profile": "release" } }
```

`release` is `"dev"` for a build outside the release workflow; `commit` is `null` when unavailable.

## `ai-gent doctor [--json]` (2.27 throwaway; 034 replaces)

Exit 0 when every check passes, 1 otherwise. Checks in 2.27: `binary` (own path resolves, executable), `os` (supported minimum), `install_root` (exists, has `current`), `version_match` (release equals the install's `VERSION` file, `skip` when not under an install root). JSON: `data.checks: [{ "id": "...", "status": "ok|warn|fail|skip", "detail": "..." }]`.

## Compatibility rules

- Adding a field is not breaking; removing or retyping one is.
- Text output is for people and may change in a minor release; only `--json` and exit codes are contract.
