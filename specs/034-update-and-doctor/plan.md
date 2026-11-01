# Implementation Plan: Update and doctor

**Branch**: `docs/specs-rust-core` (spec) | **Date**: 2026-11-01 | **Spec**: `spec.md`

## Summary

`ai-gent update --check` is the notify-only cache reader and detached refresher from `update-check.sh`, ported with the same cache and silence rules. `ai-gent update` does not re-implement installation: it fetches the target release's `install.sh`, verifies it and runs it, so there is one verified path. `ai-gent doctor` is an offline, read-only health report over the install tree, agent links, hooks and the guard self-test.

## Technical context

**Language**: Rust (workspace of 032), modules in `ai-gent-core::{install, update, doctor}`, CLI in `crates/ai-gent/src/cmd/{update,doctor}.rs`. **Dependencies**: `serde_json`, `sha2` (MANIFEST), `time` or `std` for timestamps; the network and the installer are the system `curl` and `sh`. **Testing**: `assert_cmd` + `tempfile` + a fake release base over `file://`; `scripts/test-update.sh` for the end-to-end with the real `install.sh`; fixture install trees for doctor. **Constraints**: D15, D16, AGENTS.md; no HTTP/TLS crate (032 deny list).

## Constitution and constraint check

| Constraint | Check | How |
|---|---|---|
| D15 notify-only, detached refresh, interval, env switch, silent when pinned, no silent self-replacement, `--rollback`, same verified path as `install.sh` | pass | FR-004 to FR-008; the update installs a new directory and flips a link |
| D16 install.sh and setup.sh stay a short POSIX bootstrap | pass | `update` delegates to `install.sh` (fetched fresh per target release) |
| D7 no dual paths | pass | after 2.28 the hook calls only the binary; the shell scripts are retired on the schedule in FR-011 (a temporary overlap is the upgrade path for old installs, not a second implementation of the feature) |
| D8 missing binary fails with the install command | n/a here | the update hook is silent when the binary is missing (FR-012); the guards (033) are where it matters |
| D9 `--json`, exit codes | pass | `contracts/cli.md` |
| `shared/secrets.md`: the binary never prints a secret; only a runner reads a value | pass | `doctor` reads modes only |
| Container rule (§9a) | n/a | no containers touched |
| Package allowlist | pass | `scripts/update-check.sh` leaves `ROOT_FILES` in 2.28 and `scripts/package.py`'s required-file list (line ~279) is updated; `ai-gent-update.sh` leaves in 3.0.0 |
| Naming rule (3.0.0 / 3.x) | pass | |

## Architecture

```
ai-gent-core/src/
  install.rs     InstallRoot::discover(exe_path, env) -> {home, version, dev_build}; read current/previous/pinned; list versions
  update/
    cache.rs     read/write `<epoch> <tag>` (atomic rename), staleness, optional notice date
    source.rs    ReleaseSource { Github{slug}, Base{url} }; latest() via curl; installer_url(ver), sums_url(ver)
    check.rs     decide notice, spawn detached refresh (std::process::Command, stdio null, setsid on unix, DETACHED_PROCESS on windows)
    apply.rs     fetch install.sh + SHA256SUMS to a temp dir, verify, run `sh install.sh ...`; rollback/list delegate
    version.rs   numeric compare identical to _ai_gent_ver_gt
  doctor/
    mod.rs       Check trait, runner, summary; checks/*.rs one per id in contracts/cli.md
```

### The dottod pattern, and what changes

`~/codes/dottod/scripts/.update.sh` (and its copy `scripts/update-check.sh`): pure definitions; foreground reads a timestamped cache and prints a notice; the network refresh runs in a backgrounded subshell with stdio closed; `*_NO_UPDATE_CHECK` disables; weekly interval. Kept as is: cache format, the three silences, never blocking. Changed: (1) latest comes from the `releases/latest` redirect rather than the API JSON (no rate limit, no parsing; `install.sh` already uses it); (2) the root is found from the binary's own path, not a `BASH_SOURCE` walk; (3) the pinned test reads `<home>/pinned` only, the git-checkout tests go away with git mode; (4) a notice date to print at most once a day (open question 1).

### Why `update` delegates to `install.sh`

A Rust re-implementation of download, checksum, unpack, symlink flip and `setup.sh` would be a second implementation of the verified path, against D16's "stays a short POSIX bootstrap" and D7's no dual paths. Fetching the **target** release's `install.sh` (not the installed one) matters for the 2.x to 3.0.0 jump, where asset names change (032 FR-009): the old installer would ask for an asset that no longer exists. The fetched script is verified against the same release's `SHA256SUMS`, which is the same trust as the tarball today (integrity, not provenance; provenance is `--verify-attestation` with `gh`, 032 FR-010). Cost: `update` needs `sh` and `curl`. On Windows the same idea runs `install.ps1` (D24).

### Install root discovery

`std::env::current_exe()` canonicalized; expect `<home>/<ver>/bin/ai-gent[.exe]` with `<home>/<ver>/VERSION`. If `VERSION` is missing next to `bin/`, it is a development build (`cargo run`, a checkout). `~/.local/bin/ai-gent` is a symlink, so the canonical path is what counts. The `current`-through-symlink case resolves to the version directory (`<home>/current/bin/ai-gent` canonicalizes to `<home>/2.28.0/bin/ai-gent`).

### Retirement plan for the shell scripts

| Release | `scripts/update-check.sh` | `scripts/ai-gent-update.sh` | hook | tests |
|---|---|---|---|---|
| 2.27 | unchanged | unchanged | unchanged | unchanged (the binary ships `version` and a doctor stub, 032) |
| 2.28 | stays, for git clones | stays (upgrade path of 2.26-2.27 installs and of clones; it fetches the target installer, 039 G28-7) | binary first for package installs, shell script for clones (FR-012) | `scripts/test-update-check.sh` stays for the clone path; new Rust tests and `scripts/test-update.sh` cover the binary; inventory in `quickstart.md` |
| 3.0.0 | deleted | deleted with git mode | binary only | `test-update-check.sh` deleted after the inventory is covered (SC-002) |

Overlap safety: one hook command runs either the binary or the script (`exec`), never both, and both share the cache, so at most one notice is printed per session start.

### Doctor design

Checks are independent functions returning `{id, status, detail, hint}`; the runner never panics (a panic in one check becomes `fail` with detail "check crashed"). Time budget 2 s: `install_integrity` hashes about 350 files, 650 KB, and is measured in T022; `--fast` skips it. `agent_links` uses the same agent registry as `setup.sh` (`AGENT_IDS`); to avoid a second registry the check reads `setup.sh --print-targets --json` if added, or a small `agents.json` generated from `setup.sh` by `scripts/consistency.py` and shipped in the package [open design item: T030 decides; ASSUMPTION that a generated file is simpler than parsing bash].

## Files

| Path | What |
|---|---|
| `rust/crates/ai-gent-core/src/{install.rs,update/*,doctor/*}` | library |
| `rust/crates/ai-gent/src/cmd/{update,doctor}.rs` | CLI |
| `rust/crates/ai-gent/tests/{update_check,update_apply,doctor}.rs` | tests |
| `scripts/test-update.sh` | end-to-end with the real `install.sh` over `file://` |
| `plugins/git/hooks/hooks.json` | SessionStart command (FR-012) |
| `scripts/package.py` | allowlist and required-file list |
| `scripts/check.sh` | swap the update-check test for the new ones |
| `README.md`, `docs/site` install page | the notice text and commands (task, not edited here) |

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| The detached child is killed when the hook's process group ends | refresh never lands | new session/process group (`setsid` equivalent); test with a hook-like parent that exits |
| `curl` absent (minimal containers, some Windows) | no notices | silent for `--check`; `doctor` warns |
| `install.sh` of an old release lacks `--skip-prompt` or a flag | `update --to <old>` fails | the target release's own installer is used, so flags match it; `--to` older than 2.26 refused |
| GitHub redirect format changes | wrong version string | validate `^[0-9]+\.[0-9]+\.[0-9]+$` on the last path segment, else treat as failure (as the shell script validates the tag) |
| Doctor false alarms on unusual agent setups | trust loss | `warn` not `fail` for anything outside ai-gent's own tree |
| Windows `current` flip is not atomic | a brief window without `current` | junction flip documented in 032 FR-020; `--rollback` repairs |

## Phased approach

- **Phase 0 - inventory**: map every `test-update-check.sh` check to a Rust or end-to-end test.
- **Phase 1 - discovery and cache**: install root, version compare, cache, `update --check`, detached refresh.
- **Phase 2 - update apply**: `update`, `--to`, `--rollback`, `--list` via `install.sh`; `scripts/test-update.sh`.
- **Phase 3 - doctor**: checks, JSON, fixtures.
- **Phase 4 - cutover (2.28)**: hook, package allowlist, delete `update-check.sh` and its test, docs tasks.
- **Phase 5 - 3.0.0 cleanup**: delete `ai-gent-update.sh` with git mode (039 owns the timing).
