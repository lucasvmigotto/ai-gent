# CLI contract: `ai-gent update` and `ai-gent doctor`

Extends `specs/032-rust-workspace-and-release/contracts/cli.md`.

```
ai-gent update --check [--json]
ai-gent update [--to <X.Y.Z|latest>] [--yes] [--verify-attestation] [--json]
ai-gent update --rollback [--yes] [--json]
ai-gent update --list [--json]
ai-gent update --refresh                      # hidden; the detached worker
ai-gent doctor [--fast] [--strict] [--json]
```

## `update --check`

stdout: the notice line (FR-008) or nothing. Exit 0 always (it never fails a hook), except usage errors (2).
`--json`: `{"schema":1,"ok":true,"data":{"installed":"2.27.0","latest":"2.28.0","cached_at":"2026-11-01T12:00:00Z","stale":false,"pinned":false,"checking":false,"notice":"[ai-gent] update available: ..."}}`; `latest` and `cached_at` are `null` with no cache; `notice` is `null` when silent; `checking` is true when a refresh was started by this call.

## `update`

Without `--yes` on a terminal it prints the plan (from, to, source URL, asset names) and asks `Proceed? [y/N]`; without a terminal and without `--yes` it exits 2 ("confirmation needed"). Steps: resolve target, fetch target's `install.sh` and `SHA256SUMS`, verify, run `sh install.sh --skip-prompt` with `VERSION` and `AI_GENT_HOME`. Exit: 0 installed; 3 already on the target, or a development build, or no `sh` (Windows); 4 verification failed; 1 other failure.
`--json` data: `{"from":"2.27.0","to":"2.28.0","home":"...","pinned":false,"verified":{"checksum":true,"attestation":false}}`.

## `update --rollback`, `--list`

Delegate to `install.sh --rollback` and `--list` of the current install. `--rollback` exit 3 when no `previous`. `--list` data: `{"versions":[{"version":"2.28.0","current":true,"previous":false}],"pinned":null}`.

## `doctor`

Text: one line per check, `ok`/`warn`/`FAIL`/`skip`, id, detail; hints indented below. Exit 0 when no `fail` (with `--strict`, no `warn` either), else 1.

| id | Checks | Fail / warn |
|---|---|---|
| `binary` | own path, `--version` data, executable bit | fail if the path is not under an install root and not a dev build |
| `install_root` | `current` resolves to an existing version directory; `previous` optional | fail dangling `current`; warn dangling `previous` |
| `version_match` | binary release equals the version directory's `VERSION` file | fail on mismatch |
| `install_integrity` | every line of `MANIFEST` hashes equal (skipped with `--fast`) | fail naming up to 5 files |
| `path_link` | `~/.local/bin/ai-gent` points at `current/bin/ai-gent`; the directory is on `PATH` | warn |
| `agent_links` | for each agent skills directory `setup.sh` knows (Claude, Codex, the shared agents dir, opencode, ...), ai-gent links resolve; links into another ai-gent tree | warn with `setup.sh --migrate`; fail on dangling |
| `hooks` | each installed plugin's `hooks.json` command targets exist (`hooks/guard.sh`, `bin/ai-gent`) | fail on missing |
| `guard_selftest` | evaluates a built-in set (one deny, one ask, one none per guard) through the evaluator of 033 | fail on any mismatch |
| `tools` | `git`, `curl` (needed), `sh` (needed for update), `gh`, `pass`, `podman`/`docker` (optional, info) | warn for a missing needed tool; `skip` for optional |
| `state` | state and cache directories exist or are creatable; shadow log size | warn over 20 MiB |
| `update_cache` | cache readable; age; pinned; shows cached latest, never contacts the network | skip when disabled |
| `environment` | stale variables: `AI_GENT_MODE=git`, `AI_GENT_BRANCH`, `AI_GENT_DIR` (removed in 3.0.0) | warn |
| `permissions` | Unix modes of ai-gent's credential and state files (0600 files, 0700 dirs); never reads content | warn |

`--json` data: `{"checks":[{"id":"binary","status":"ok","detail":"...","hint":null}],"summary":{"ok":11,"warn":1,"fail":0,"skip":1}}`.
