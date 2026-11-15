# Quickstart: how update and doctor are proven

```sh
cd rust && cargo test -p ai-gent --test update_check --test update_apply --test doctor
scripts/test-update.sh          # real install.sh + real binary, file:// releases, throwaway HOME
ai-gent doctor --json | jq .summary
ai-gent update --check --json
```

Manual rehearsal on the owner's machine: `AI_GENT_RELEASE_BASE=file://$PWD/dist AI_GENT_HOME=$(mktemp -d)`; install 9.9.0, build 9.9.1 with `scripts/package.py`, run `ai-gent update --yes`, then `--rollback`.

## Inventory: `scripts/test-update-check.sh` to its replacement (T001)

Filled by T001 (the table below). The groups to cover: version ordering (line 81), local version resolution (88), repo slug resolution (92), stale refresh and detached refresh (132-139), `ai-gent-update.sh` behaviors (164-210, stay until 3.0.0), hook wiring in a clone and in a package (216-251), pinned silence and unpin (248-251), package update re-running the installer with `latest` and an explicit version (253-262).

### Table (T001)

One row per `^check` line of the script (`grep -c '^check' scripts/test-update-check.sh` is 15). The script stays until the 3.0.0 cutover (D7); a row marked "stays" tests a shell path that has no binary counterpart.

| Line | Check | Replacement |
|---|---|---|
| 81 | version comparison orders triples | `update::version::tests::orders_triples_like_the_shell` |
| 88 | local tag resolves newest tag | stays (git-clone mode; the binary reads `VERSION`, `install::tests::package_install_found_through_symlinks`) |
| 92 | slug prefers override, then origin, then default | `update::source::tests::source_from_env` (no `origin` lookup: git mode is not ported) |
| 132 | stale refresh landed | `update_check::stale_cache_notifies_from_the_cache_and_refreshes` |
| 139 | detached refresh populates the cache | `update_check::no_cache_is_silent_then_the_refresh_lands` |
| 218 | clone: the hook's relative path exists | stays (clone hook, until 3.0.0) |
| 219 | clone: the hook exits 0 | stays |
| 220 | plugin outside any tree: silent no-op | stays; the binary branch is T041, not wired here |
| 227 | package: relative path exists in the package | stays |
| 242 | package: works through the skills symlink and `current` | `install::tests::package_install_found_through_symlinks` (resolves through `current`) |
| 248 | package: pinned install stays silent | `update_check::silent_when_opted_out_pinned_or_a_development_build` |
| 251 | package: unpinned notifies again | `update_check::newer_cached_release_prints_exactly_one_line`, `update_apply::explicit_version_pins_and_latest_unpins` |
| 259 | package update re-runs the installer with `latest` | `update_apply::update_installs_the_latest_and_rollback_restores`, `scripts/test-update.sh` |
| 262 | package update takes an explicit version | `update_apply::explicit_version_pins_and_latest_unpins` |
| (ok lines 40-53, 108, 185-214, 231) | notice, silence, `ai-gent-update.sh` clone cases | notice and silences: `update::check::tests`; `ai-gent-update.sh` cases stay until 3.0.0 |
