# Tasks: Update and doctor

**[P]** = parallel-safe. Propose first, `scripts/check.sh --quick --quiet` before each commit. Stories: US1 notice, US2 update, US3 rollback, US4 doctor, US5 retirement.

## Phase 0 - inventory

- [x] T001 [US5] Write the table in `specs/034-update-and-doctor/quickstart.md`: each check of `scripts/test-update-check.sh` (lines 81-262) mapped to the Rust test or `scripts/test-update.sh` step that replaces it. Verify: every check line of the script appears in the table (`grep -c '^check' scripts/test-update-check.sh` equals the table rows). **Evidence:** table in quickstart.md (15 `^check` lines, 15 rows).

## Phase 1 - discovery, cache, check (US1)

- [x] T010 [US1] `ai-gent-core/src/install.rs`: `InstallRoot::discover`, readers for `current`/`previous`/`pinned`, version listing. Verify: `cargo test -p ai-gent-core install` over tempdir trees (package, dev build, dangling `current`). **Evidence:** `cargo test -p ai-gent-core install` (5 tests: package via `current`, dev build, dangling, garbled pin, bad VERSION).
- [x] T011 [P] [US1] `update/version.rs` with the cases of `_ai_gent_ver_gt` (test at `test-update-check.sh:81`). Verify: `cargo test -p ai-gent-core update::version`. **Evidence:** `update::version` tests incl. the shell's cases.
- [x] T012 [P] [US1] `update/cache.rs` (atomic write, staleness, future timestamp, optional notice date). Verify: `cargo test -p ai-gent-core update::cache`; read a file written by the real `scripts/update-check.sh` refresh (fixture) and the reverse. **Evidence:** `update::cache` tests (atomic write, future timestamp, shell-format round trip); the third notice-date field is read, never written (open question 1 left to the owner).
- [x] T013 [US1] `update/source.rs` and `update/check.rs`: release source (redirect via `curl -sI -o /dev/null -w %{url_effective}`; `AI_GENT_RELEASE_BASE` `LATEST`), detached refresh. Verify: `cargo test -p ai-gent --test update_check` (US1 scenarios 1-6; a fake `curl` on `PATH` that sleeps 10 s proves the 50 ms foreground bound, SC-001). **Evidence:** `update_check` 9 tests; hung fake curl: check took ~3 ms (SC-001).
- [x] T014 [US1] `cmd/update.rs --check` wiring and `--json`; hidden `--refresh`. Verify: snapshot tests of `--json` (`insta`). **Evidence:** `json_shape` insta snapshots; hidden `--refresh`.

## Phase 2 - update, rollback (US2, US3)

- [x] T020 [US2] `update/apply.rs`: fetch target `install.sh` and `SHA256SUMS`, verify, run with env, map exit codes (4 on mismatch). Verify: `cargo test -p ai-gent --test update_apply` using a release base built by `scripts/package.py` in a temp dir. **Evidence:** `update_apply` with package.py releases and the real install.sh; tampered installer and archive give exit 4.
- [x] T021 [US2] `--to`, pin semantics (`pinned` written by the installer; `--to latest` clears), refusal of versions below 2.26, confirmation logic (`--yes`, terminal detection, exit 2 without one). Verify: same test file. **Evidence:** same file: pin, `--to latest`, below 2.26 refused, exit 2 without a terminal.
- [x] T022 [US3] `--rollback` and `--list` delegating to the current install's `install.sh`. Verify: same test file; exit 3 with no `previous`. **Evidence:** same file: rollback, `--list`, exit 3 without previous.
- [x] T023 [US2] `scripts/test-update.sh` end-to-end: install 9.9.0 then 9.9.1 from `file://` with the real `install.sh` and the real binary built from `rust/`; update, rollback, bad checksum, pinned silence. Wire in `scripts/check.sh` (skipped with `--quick`, like the installer tests). Verify: `scripts/test-update.sh`. **Evidence:** `scripts/test-update.sh` (13 checks), wired in check.sh (skipped with --quick).

## Phase 2b - Windows (D24)

- [ ] T024 [US2] `update` on Windows fetches and runs the target release's `install.ps1` (junction layout of 032 FR-020); `doctor` checks `powershell` instead of `sh`. Verify: the windows leg of `scripts/test-install-ps1.ps1` extended with an update and rollback case.

## Left for the owner / not done here

- T024 (Windows `install.ps1` update, `doctor` checking `powershell`): the code path exists but is untested; needs a Windows machine.
- T040-T042 (hook cutover, hook-wiring test, docs) and T049-T050 (3.0.0): the hook and the shell scripts stay (D7), by instruction.

## Phase 3 - doctor (US4)

- [x] T030 [US4] Decide and implement the agent registry source for `agent_links` (generated `agents.json` from `setup.sh` `AGENT_IDS`, checked by `scripts/consistency.py`), and record the choice in `plan.md`. Verify: `python3 scripts/consistency.py`. **Decision:** a table in `doctor/checks.rs` (`AGENT_DIRS`: six skills directories with their override variables), not a generated `agents.json`; a consistency check against `setup.sh` is left for the owner (not added: `consistency.py` untouched).
- [x] T031 [US4] `doctor/mod.rs` runner plus the checks `binary`, `install_root`, `version_match`, `path_link`. Verify: `cargo test -p ai-gent --test doctor` fixtures healthy/dangling/mismatch. **Evidence:** `cargo test -p ai-gent --test doctor` healthy/dangling/mismatch fixtures.
- [x] T032 [P] [US4] `install_integrity` (MANIFEST) and `--fast`; measure time on a real package (target under 2 s total). Verify: test with a tampered file; `time ai-gent doctor` on an installed 2.x. **Evidence:** tampered file fails; healthy doctor ~1.8 s in the debug profile on 353 files, under 2 s.
- [x] T033 [P] [US4] `agent_links` and `hooks`. Verify: fixtures with an old-clone link (`warn`, hint `setup.sh --migrate`) and a missing `hooks/guard.sh`. **Evidence:** old-clone link warns with `setup.sh --migrate`; missing `hooks/guard.sh` fails.
- [x] T034 [P] [US4] `guard_selftest` calling the 033 evaluator (skipped until 033 Phase 1 lands; then required). Verify: test flips one built-in expectation and sees `fail`. **Evidence:** `doctor::selftest` tests, flipped expectation seen (033 evaluator has landed, so required).
- [x] T035 [P] [US4] `tools`, `state`, `update_cache`, `environment`, `permissions` (modes only). Verify: fixtures; a test asserting no file content is read (a fixture file with unreadable content but readable metadata still produces the result). **Evidence:** fixtures; `permissions_look_at_modes_and_never_content` (mode-000 file, no content in output).
- [x] T036 [US4] `--json`, `--strict`, exit codes; snapshot. Verify: `cargo test -p ai-gent --test doctor`. **Evidence:** `--json` envelope, `--strict`, exit codes in doctor.rs tests.

## Phase 4 - cutover, 2.28 (US5)

- [ ] T040 [US5] `plugins/git/hooks/hooks.json` SessionStart command per FR-012 (binary first, script for clones); the scripts and `scripts/test-update-check.sh` stay in 2.28; add the Rust tests and `scripts/test-update.sh` to `scripts/check.sh`. Verify: `python3 scripts/package.py --check`, `scripts/check.sh`.
- [ ] T041 [US5] A hook-wiring test replacing `test-update-check.sh:216-251`: the hook command in a package layout and in a plugin outside any tree stays silent and exits 0. Verify: `scripts/test-update.sh` section.
- [ ] T042 [US5] Doc tasks (not edited by this spec): README update section (lines ~55-60), docs site install page, `shared/releasing.md` mention, notice text in the `git` plugin docs. Verify: `scripts/check.sh` links.

## Phase 5 - 3.0.0

- [ ] T049 [US5] Delete `scripts/update-check.sh`, `scripts/test-update-check.sh`, remove the script branch from the hook, update `package.py`'s allowlist and required-file list. Verify: SC-006, `python3 scripts/package.py --check`.
- [ ] T050 [US5] Delete `scripts/ai-gent-update.sh` with git mode; `doctor` `environment` check keeps warning on the removed variables for one more release. Verify: SC-006.
