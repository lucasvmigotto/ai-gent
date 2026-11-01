# Feature Specification: Update and doctor

**Feature Branch**: `docs/specs-rust-core` (specification only; built on its own branches)

**Created**: 2026-11-01

**Status**: Draft

**Input**: D15 of `docs/product/rust-decisions.md`, as written: update is notify-only; `ai-gent update --check` prints a notice only when a newer release is cached, never blocks, refreshes detached on an interval (default 7 days), honours `AI_GENT_NO_UPDATE_CHECK`, and stays silent for a pinned version; `ai-gent update` installs through the same verified path as `install.sh`; `--rollback` goes back; no silent self-replacement. Pattern: `~/codes/dottod/scripts/.update.sh`. Plus `ai-gent doctor`, an offline health check.

**Depends on**: 032 (binary, install tree, release assets). Read by 039.

## Context and ground rules

Verified on 2026-11-01:

- `scripts/update-check.sh` (178 lines, bash) is the SessionStart hook (`plugins/git/hooks/hooks.json`, which runs it by the relative path `${CLAUDE_PLUGIN_ROOT}/../../scripts/update-check.sh` and no-ops when the file is missing). It reads the cache `${XDG_STATE_HOME:-~/.local/state}/ai-gent/update-check` (one line, `<epoch> <tag>`), refreshes it detached when older than `AI_GENT_UPDATE_DAYS` (default 7) by calling the GitHub API `releases/latest` with `curl --max-time 8`, stays silent when `AI_GENT_NO_UPDATE_CHECK` is set, when a git checkout is detached at a tag, or when a package install has a `pinned` file next to its version directory, and prints one line: `[ai-gent] update available: A -> B - run scripts/ai-gent-update.sh ... (propose first, never self-apply in-session)`. Its ancestor is `~/codes/dottod/scripts/.update.sh` (same cache, notify-only, `_dottod_update_check`, `DOT_UPDATE_DAYS`, `DOT_NO_UPDATE_CHECK`).
- `scripts/ai-gent-update.sh` (88 lines): for a package install it re-runs the installer from the installed tree (`VERSION=latest sh $root/install.sh --skip-prompt`, `AI_GENT_HOME` = the parent of the version directory); for a clone it fast-forwards or checks out a tag.
- `install.sh` owns the install tree: `$AI_GENT_HOME/<ver>/`, `current`, `previous` (flipped atomically), `pinned` (written by an explicit `VERSION=x.y.z`, removed by `latest`), `--list`, `--rollback`, `--prune`, checksum verification before anything is written, optional `gh attestation verify`.
- `scripts/test-update-check.sh` (267 lines, about 30 checks) covers version ordering, slug resolution, cache and detached refresh, pinned silence, the hook's relative path in a clone and in a package, and the update script. Its cases become the Rust test inventory (`quickstart.md`).
- The guard hooks will run the binary (033), so the binary is security-relevant: it must never replace itself behind the user's back (D15).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A quiet notice when a newer release exists (Priority: P1)

**Independent Test**: `cargo test -p ai-gent --test update_check` with a fake release base over `file://` and a temp state directory.

**Acceptance Scenarios**:

1. **Given** the cache says `2.28.0` and the install is `2.27.0`, **When** `ai-gent update --check` runs, **Then** it prints exactly one line naming both versions and the command `ai-gent update`, and exits 0.
2. **Given** no cache, **When** `--check` runs, **Then** it prints nothing, starts a detached refresh and returns within 50 ms; the next run sees the cache.
3. **Given** a cache older than 7 days (or `AI_GENT_UPDATE_DAYS`), **When** `--check` runs, **Then** it notifies from the stale cache and starts a detached refresh.
4. **Given** `AI_GENT_NO_UPDATE_CHECK=1`, a `pinned` file, or a binary that is not under an install root (a development build), **When** `--check` runs, **Then** it prints nothing and starts no refresh.
5. **Given** the network is down, **When** the refresh runs, **Then** it fails silently and leaves the previous cache intact.
6. **Given** `--check --json`, **Then** the output is one JSON object (`contracts/cli.md`) with `notice: null` when there is nothing to say.

### User Story 2 - Update through the verified installer path (Priority: P1)

**Independent Test**: `scripts/test-update.sh` (new, bash) drives `ai-gent update` against a `file://` release base in a throwaway `HOME`, like `scripts/test-package.sh`.

**Acceptance Scenarios**:

1. **Given** an install of 2.27.0 and a release base holding 2.28.0, **When** `ai-gent update` runs and the person confirms (or passes `--yes`), **Then** it fetches the target release's `install.sh`, verifies it against `SHA256SUMS`, and runs it with `VERSION=2.28.0 AI_GENT_HOME=<home>`; `current` points at 2.28.0, `previous` at 2.27.0.
2. **Given** a checksum mismatch, **Then** nothing changes and the exit code is 4.
3. **Given** `ai-gent update --to 2.26.0`, **Then** the install is pinned (a `pinned` file is written, exactly as `VERSION=` does) and `--check` goes silent.
4. **Given** `ai-gent update --to latest`, **Then** the pin is cleared.
5. **Given** the command runs inside an agent session, **Then** it never runs without `--yes` unless a terminal confirms; the notice text says "propose first, never self-apply in-session" (the existing rule) so an agent asks the person.
6. **Given** the running binary lives in the version directory being replaced, **Then** no file in the running version is modified: a new version directory is unpacked and `current` is flipped (nothing is replaced in place), so the running process and Windows file locks are unaffected.

### User Story 3 - Go back (Priority: P1)

**Acceptance Scenarios**:

1. **Given** `previous` exists, **When** `ai-gent update --rollback` runs, **Then** `current` and `previous` swap and the agent links are refreshed (`setup.sh --skip-prompt`), exactly like `install.sh --rollback`.
2. **Given** no `previous`, **Then** exit 3 (nothing to do) with a message.
3. **Given** the rolled-back version is older than the guard fixtures the binary expects, **Then** nothing breaks: `current/bin/ai-gent` is that older version's own binary.

### User Story 4 - Know the install is healthy, offline (Priority: P1)

**Independent Test**: `cargo test -p ai-gent --test doctor` over fixture install trees (healthy, dangling link, wrong version, old clone links, missing binary, wrong permissions).

**Acceptance Scenarios**:

1. **Given** a healthy install, **When** `ai-gent doctor` runs, **Then** every check is `ok` and the exit code is 0.
2. **Given** a skill link in `~/.claude/skills` pointing into an old git clone, **Then** doctor reports `warn` for `agent_links` with the hint `setup.sh --migrate`.
3. **Given** `MANIFEST` lists a file whose hash differs, **Then** `install_integrity` is `fail` naming the file.
4. **Given** no network, **Then** doctor still completes within 2 seconds and says which checks need the network (none do; the update check reports the cached state only).
5. **Given** `--json`, **Then** `contracts/cli.md` holds.

### User Story 5 - The shell scripts retire cleanly (Priority: P2)

**Acceptance Scenarios**:

1. **Given** 2.28 is installed as a package, **Then** the SessionStart hook runs `ai-gent update --check`; a git clone's hook still runs `scripts/update-check.sh` (D7 amended: no dual path survives 3.0.0).
2. **Given** a user on 2.26 or 2.27 runs their installed `scripts/ai-gent-update.sh`, **Then** it still upgrades them to 2.28 (the old script and the old installer see the content-only asset of 032 FR-009).
3. **Given** 3.0.0, **Then** `scripts/ai-gent-update.sh` and `scripts/update-check.sh` do not exist and `scripts/test-update-check.sh` is replaced by the Rust tests and `scripts/test-update.sh`.

## Requirements *(mandatory)*

- **FR-001**: Subcommands (full contract in `contracts/cli.md`): `update --check`, `update [--to X.Y.Z|latest] [--yes] [--verify-attestation]`, `update --rollback`, `update --list`, hidden `update --refresh` (the detached worker), `doctor [--fast] [--strict]`.
- **FR-002**: The install root is found from the running binary's real path (`<home>/<ver>/bin/ai-gent`), else `AI_GENT_HOME`, else the default (`${XDG_DATA_HOME:-~/.local/share}/ai-gent`, Windows `%LOCALAPPDATA%\ai-gent`). A path with no `VERSION` file beside `bin/` is a development build: `update` refuses (exit 3), `--check` is silent.
- **FR-003**: Cache: the same file and format as `update-check.sh` (`<epoch> <tag>` at `${XDG_STATE_HOME:-~/.local/state}/ai-gent/update-check`, Windows `%LOCALAPPDATA%\ai-gent\state\update-check`), written by write-temp-then-rename, so the shell script and the binary can coexist during the transition and the cache survives either.
- **FR-004**: Refresh: detached child process (`update --refresh`) with a hard 8 s limit, no output, no effect on the caller's exit code. The latest version is read from the redirect target of `https://github.com/lucasvmigotto/ai-gent/releases/latest` (what `install.sh` does; no JSON parsing and no API rate limit) through the system `curl`; `AI_GENT_RELEASE_BASE` (with `<base>/LATEST`) overrides it for tests; `AI_GENT_UPDATE_REPO` overrides the slug. The binary links no TLS or HTTP crate (032 FR-003).
- **FR-005**: Interval: `AI_GENT_UPDATE_DAYS` (default 7). `AI_GENT_NO_UPDATE_CHECK` (any value) disables check and refresh. A `pinned` file next to the version directories silences both (D15).
- **FR-006**: `update` obtains the **target release's** `install.sh` (asset of that release), verifies it against that release's `SHA256SUMS` (and with `gh attestation verify` when `--verify-attestation` and `gh` exist), then runs it with `VERSION`, `AI_GENT_HOME` and `--skip-prompt`. There is no second implementation of download, checksum, unpack or link flipping in Rust (D16: the bootstrap stays one POSIX script; "same verified path as `install.sh`"). On native Windows it fetches and runs the target release's `install.ps1` instead (D24; 032 contract `install-ps1.md`), verified the same way (the `SHA256SUMS` line for `install.ps1`).
- **FR-007**: `--rollback` and `--list` run `install.sh --rollback` and `--list` from `current/` (the installer shipped inside the installed version).
- **FR-008**: The notice is one line, `[ai-gent] update available: <local> -> <remote> - run: ai-gent update (propose first, never self-apply in-session)`, printed by `--check` to stdout so the SessionStart hook adds it to the session context; nothing else is printed by `--check`.
- **FR-009**: `doctor` is offline (no network access, no subprocess that contacts one), read-only, and finishes in under 2 s on a healthy install. Checks are listed in `contracts/cli.md` (`binary`, `install_root`, `version_match`, `install_integrity`, `path_link`, `agent_links`, `hooks`, `guard_selftest`, `tools`, `state`, `update_cache`, `environment`, `permissions`). Exit 0 unless a check is `fail` (or `warn` with `--strict`).
- **FR-010**: `doctor` never reads a secret value; the `permissions` check inspects file modes only (credential files of `shared/secrets.md` should be 0600 on Unix).
- **FR-011**: Retirement (D7 amended): 2.28 wires the package hook to the binary and keeps `scripts/update-check.sh` and `scripts/ai-gent-update.sh` for git clones and for the upgrade of 2.26-2.27 installs; both are deleted at 3.0.0 with git mode. The cache format is unchanged so both can read it.
- **FR-012**: The hook command in `plugins/git/hooks/hooks.json` becomes `b="${CLAUDE_PLUGIN_ROOT}/../../bin/ai-gent"; if [ -x "$b" ]; then exec "$b" update --check; fi; f="${CLAUDE_PLUGIN_ROOT}/../../scripts/update-check.sh"; if [ -f "$f" ]; then exec bash "$f"; fi; exit 0` (binary first, the shell script for clones, silent otherwise; at 3.0.0 the second branch is deleted). The missing binary is reported by `doctor` and the guards, not by the update notice. On native Windows without `sh` the hook form is the open point of spec 033 question 6.
- **FR-013**: `update` and `doctor` follow the global CLI contract of 032 (`--json`, exit codes).

### Key entities

See `data-model.md`: Install root, Update cache, Pin, Doctor check result, Release source.

## Edge cases

- Two sessions start together: both may start a refresh; the cache is replaced by rename, last writer wins, both values are valid.
- Clock skew (cache timestamp in the future): treated as stale.
- The installed version is newer than the latest release (a pre-release build): no notice.
- `current` is a dangling link: `doctor` fails `install_root` and prints `sh install.sh` as the fix.
- A user ran `install.sh` with a custom `AI_GENT_HOME` and starts an agent without it set: the binary finds its root from its own path, so `AI_GENT_HOME` is only a fallback.
- Windows: `current` and `previous` are directory junctions (032 FR-020); `InstallRoot::discover` resolves them like symlinks.

## Out of scope

- Updating without confirmation, or in the background (D15: no silent self-replacement).
- Updating the git-mode clone (stays `ai-gent-update.sh` until 3.0.0).
- A package manager distribution (brew, winget).
- Fixing problems `doctor` finds, beyond printing the exact command (`setup.sh --migrate`, `sh install.sh`).

## Success criteria *(mandatory)*

- **SC-001**: `--check` returns in under 50 ms when the cache is fresh or absent (no network on the foreground path), measured by a test with a 10 s hung fake `curl`.
- **SC-002**: Every case of `scripts/test-update-check.sh` has a Rust or `test-update.sh` counterpart (inventory in `quickstart.md`) before the script is deleted.
- **SC-003**: `update` to a `file://` release base installs, flips `current`/`previous`, and `--rollback` restores, with checksum mismatch leaving the tree unchanged.
- **SC-004**: `doctor` on a healthy fixture exits 0; on each broken fixture reports the expected check as `warn` or `fail`.
- **SC-005**: No Rust crate in the dependency graph provides HTTP or TLS (`cargo deny check bans`).
- **SC-006**: After 3.0.0, `git ls-files | grep -E 'update-check|ai-gent-update'` finds nothing.

## Open questions (for the owner)

1. **Interval and notice cadence**: keep 7 days (D15). Recommended: also print the notice at most once per day even if sessions restart, by storing the last notified date in the cache line (third field). The shell script notified at every session start; this is a small behavior change.
2. **Answered by D24**: Windows uses `install.ps1`; `doctor` checks for `powershell` instead of `sh` on Windows.
3. **`doctor` network check**: offline only (D15 spirit). Recommended: a separate `doctor --online` is not built; `update --check` already covers the network.
4. **Should `ai-gent update` offer the notice in non-Claude agents?** The notice is a Claude Code SessionStart feature (README says opencode has no session-start hook point). Recommended: unchanged; others run `ai-gent update --check` by hand.
