# Tasks: Guard evaluator

**[P]** = parallel-safe (different files). Every task: propose first (CLAUDE.md), run `scripts/check.sh --quick --quiet` before committing. Fixture ids are in `shared/guard-fixtures/`. Stories: US1 parity, US2 Windows shells, US3 shadow, US4 opencode, US5 failure policy, US6 latency.

## Phase 0 - baseline and extraction (no Rust)

- [ ] T001 [US6] Write `scripts/bench-guard.py` (stdlib; `--impl sh|bin`, `--runs 100`, `--json`, budget check, discards 5 warmups, `perf_counter_ns`) and run it for the five shell guards on Linux; paste p50/p95/p99 into `specs/033-guard-evaluator/quickstart.md` (this host's first numbers: git 3.3, db 1.1, secrets 0.8, repo 0.6, devcontainer 1.6 ms mean). Verify: `python3 scripts/bench-guard.py --impl sh --json`.
- [ ] T002 [P] [US6] Run the same on macOS and on Windows (Git Bash) and record. Owner-run on real machines. Verify: three rows in the table.
- [ ] T003 [US6] Decide the Windows budget from T002 and edit FR-012 in `spec.md` (replace the assumption). Verify: spec has no `[ASSUMPTION]` on the Windows number.
- [ ] T010 [US1] One-off extraction (not committed; its counts go in `manifest.json`): read the `expect` calls of the five `scripts/test-*guard.sh` and the vector arrays of `opencode/guard/test/rules.test.mjs`, merge duplicates, run each case through the real `guard.sh` to record `want` and `reason`, write `shared/guard-fixtures/{git,db,secrets,repo,containers,adapter}.json` and `manifest.json`. Files: those. Verify: `python3 scripts/guard-fixtures.py count` prints 192 shell-sourced cases plus the opencode-only ones; none lost.
- [ ] T011 [US1] `scripts/guard-fixtures.py` (stdlib): `run --guard <id> --impl sh|mjs|bin`, `count`, `lint` (unique ids, `reason` present for deny/ask, variants valid). Creates the sandbox repos for `@main` etc. Verify: `python3 scripts/guard-fixtures.py lint`.
- [ ] T012 [US1] Rewrite `scripts/test-guard.sh`, `test-db-guard.sh`, `test-secrets-guard.sh`, `test-repo-guard.sh`, `test-containers-guard.sh` (the guard parts; keep the `leftovers` section of the last) as thin loaders over T011, keeping the `ok`/`FAIL` line format. Verify: `scripts/test-guard.sh` and the four others print the same number of ` ok ` lines as before T010 (192 in total) and `scripts/check.sh --quick --quiet` is green.
- [ ] T013 [P] [US1] Rewrite `opencode/guard/test/rules.test.mjs` to read the fixtures (keep `adapterCases` in `adapter.json`). Verify: `node opencode/guard/test/rules.test.mjs` (or the runtime `check.sh` picks) reports the same check count or more.
- [ ] T014 [US1] Add fixtures to `scripts/check.sh` output (case count line) and `package.py --check` assertion that `shared/guard-fixtures/` is absent from the package. Verify: `python3 scripts/package.py --check`.

## Phase 1 - evaluator for sh (US1)

- [ ] T020 [US1] `rust/crates/ai-gent-core/src/guard/{event,mod,reasons}.rs` and `reasons.json` (extract every reason from the five `guard.sh` and `rules.mjs`; ids). Verify: `cargo test -p ai-gent-core guard::reasons` (every marker prefix matches its guard; no duplicate ids).
- [ ] T021 [US1] `segment/sh.rs`: port the awk pipeline (blank `\"..\"` then `'..'`, `\n`, separators; containers variant with backtick and `$CONTAINER_ENGINE` kept). Unit tests from the pipeline's own comments. Verify: `cargo test -p ai-gent-core segment::sh`.
- [ ] T022 [P] [US1] `rules/git.rs` + `branch.rs`. Verify: `cargo test -p ai-gent-core --test guard_parity -- git` (fixtures `git-*`).
- [ ] T023 [P] [US1] `rules/repo.rs`. Verify: same for `repo-*`.
- [ ] T024 [P] [US1] `rules/containers.rs`. Verify: `containers-*`.
- [ ] T025 [P] [US1] `rules/secrets.rs` (prefix regex, gpg/password-store, pass-run, subcommand switch). Verify: `secrets-*`.
- [ ] T026 [P] [US1] `rules/db.rs` including the credentials check for non-Bash tools (keys `file_path|notebook_path|path|glob`, plus `pattern` for Glob) and the quote-blanking scanner (`words`). Verify: `db-*`.
- [ ] T027 [US1] `evaluate_all` and the `ai-gent guard` subcommand (`--guard`, `--format claude|opencode`, stdout JSON identical after parsing). Verify: `python3 scripts/guard-fixtures.py run --guard all --impl bin` is green; `cargo test -p ai-gent --test cli guard`.
- [ ] T028 [US1] `tests/guard_parity.rs` loads all fixtures, shell `sh`, asserts decision and reason. Verify: `cargo test -p ai-gent-core --test guard_parity` (SC-001).
- [ ] T029 [US1] `scripts/consistency.py`: read reasons from `reasons.json` (keep the `guard.sh` scrape while the scripts still hold logic); test in `scripts/test_consistency.py`. Verify: `python3 -m unittest scripts/test_consistency.py`.

## Phase 2 - PowerShell and cmd (US2)

- [ ] T030 [US2] `segment/powershell.rs` and `segment/cmd.rs` with unit tests for each quoting case in US2 (doubled quote, backtick, caret). Verify: `cargo test -p ai-gent-core segment::`.
- [ ] T031 [US2] Add `variants` to every sh case where the command has an equivalent (hand-written; cases that are sh-only, such as `$(...)` or heredocs, carry `skip_in` for the other shells with a reason). Verify: `python3 scripts/guard-fixtures.py lint` and the parity test runs the variants (count printed).
- [ ] T032 [P] [US2] Windows-only fixtures: drive-letter paths, `%APPDATA%`, `$env:XDG_CONFIG_HOME`, `Get-Content` and `type` of the db credentials file, `git -C C:\path commit`, `psql.exe`. Verify: fixtures tagged `windows` pass on the windows legs of `rust.yml`.
- [ ] T033 [US2] Run `cargo test` for the guard on the Windows legs in `rust-build.yml` (032). Verify: both Windows legs green.
- [ ] T034 [US2] `--shell` selection and `tool_name` mapping; verify what Claude Code on Windows sends for a PowerShell command (event shape, tool name, which shell runs hook commands). Owner-run. Verify: answer recorded in `spec.md` open question 4 and the ASSUMPTION in FR-003 replaced.

## Phase 3 - shadow mode, release 2.27 (US3)

- [ ] T040 [US3] `shadow.rs` (record, redaction with `shared/secret-patterns.json`, rotate) and `guard --shadow`. Verify: `cargo test -p ai-gent-core shadow` (no command text in the file; capture only on disagreement; mode 0600).
- [ ] T041 [US3] `guard shadow-report` (text and `--json`, exit 1 on unexplained disagreement). Verify: `cargo test -p ai-gent --test cli shadow_report`.
- [ ] T042 [US3] Edit the five `plugins/*/hooks/guard.sh`: after the decision is printed, run the shadow call detached when the binary exists; `AI_GENT_NO_SHADOW` disables. Tests: `scripts/test-*guard.sh` unchanged output plus a new check that the log gains one line when a fake binary is on `PATH`. Verify: `scripts/test-guard.sh`; time with `scripts/bench-guard.py --impl sh` that shadow adds no wait (hook exit within 5 ms of baseline).
- [ ] T043 [US3] `guard replay` and the first run over the owner's Claude Code transcripts (counts and hashes only). Owner-run. Verify: report shows 0 differences or each is explained by a fixture.
- [ ] T044 [P] [US3] Mutation corpus: `scripts/guard-fixtures.py mutate` generates variations (whitespace, `sudo`, env prefixes, `-C` placement) of every parity case, runs the oracle and the binary, and fails on any difference. Verify: `python3 scripts/guard-fixtures.py mutate --impl bin --oracle sh`.
- [ ] T045 [US3] Document shadow in `plugins/git/README`-level docs and the 2.27 `## Unreleased` notes (user-facing: "a new binary now double-checks guard decisions; nothing changes in what is blocked"). Verify: `scripts/check.sh`.
- [ ] T046 [US3] After 2.27 has been in use: run the promotion check (`FR-010`) with `guard shadow-report --json` and record the numbers in `quickstart.md`. Owner-run. Verify: criteria 1-5 each marked met with evidence.

## Phase 4 - cutover, release 2.28 (US1, US5, US6)

- [ ] T050 [US5] Write the launcher head once as `shared/guard-launcher.sh` (about 10 lines; detects a package install by `../../VERSION`, else falls through) and copy it at the top of the five `hooks/guard.sh` with the guard id substituted; `scripts/consistency.py` fails if a copy differs (the `--sync-shared-code` pattern). Verify: `python3 scripts/consistency.py`.
- [ ] T051 [US5] Failure tests in the five `test-*guard.sh` via the fixture runner: binary renamed, binary exits 1 with no output, binary sleeps over the budget; each must print the `ask` line with the install command. Verify: `scripts/test-guard.sh` (and siblings) with `--impl sh` now exercising the launcher; SC-006.
- [ ] T052 [US1] Add the launcher head to the five scripts in one commit per guard (shell logic stays below it for clones); fixtures run `--impl sh` (clone path), `--impl launcher` (package path with the real binary) and `--impl bin`. Verify: `scripts/check.sh` green; fixture counts equal for the three.
- [ ] T053 [US6] Bench gate: `scripts/bench-guard.py --impl bin --check` in the Linux CI job (budget FR-012); macOS and Windows jobs run it informationally and the numbers go into the PR. Verify: CI log lines.
- [ ] T054 [US1] Update `docs`: the `plugins/*/hooks` header comments, `CLAUDE.md` layout rule text ("The OpenCode guard rules and the shell guards must decide the same" becomes "all agents call `ai-gent guard`"), `shared/guard-ux.ts` header. Verify: `scripts/check.sh`.
- [ ] T055 [US5] Optional binary for clone contributors: `scripts/dev-bin.sh` (builds `rust/` or downloads the matching release binary into `bin/`). Verify: a fresh clone runs the guard tests after `scripts/dev-bin.sh`.

## Phase 4b - 3.0.0 deletion (US1)

- [ ] T056 [US1] With git mode removed (039 T010): delete the shell logic below the launcher in the five `guard.sh`, switch the five `test-*guard.sh` to `--impl launcher`, update `scripts/consistency.py` to read only `reasons.json`. Verify: SC-005 grep; `scripts/check.sh`.

## Phase 5 - opencode (US4)

- [ ] T060 [US4] `opencode/guard/index.ts` (2.28): when running from a package install call `ai-gent guard --guard all --format opencode` through `execFileSync` with a timeout; apply FR-014 on failure. Keep `branchOf` out (the binary resolves branches). Verify: `opencode/guard/test/adapter.test.mjs` (new) with a fake binary: decisions mapped, failure => ask.
- [ ] T061 [US4] At 3.0.0 delete `opencode/guard/rules.mjs` and the old parity test; point `scripts/check.sh` at the binary-backed adapter test; adjust `scripts/package.py` `TREES` if `opencode/guard` shrinks. Verify: `scripts/check.sh` and `python3 scripts/package.py --check`.
- [ ] T062 [US4] `scripts/opencode-config.py` and `setup.sh` unchanged unless the plugin path moves; run `scripts/test-install.sh`. Verify: the opencode install checks pass.
