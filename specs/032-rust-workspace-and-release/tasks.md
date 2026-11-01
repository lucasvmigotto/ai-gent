# Tasks: Rust workspace and release

Order matters inside a phase unless marked **[P]** (parallel-safe: touches different files). Each task fits one builder session. "Verify" is the command that must pass. Stories: US1 de-risk, US2 gates, US3 release, US4 installer, US5 crates, US6 2.27. Rules for every task: propose before writing (CLAUDE.md), `scripts/check.sh --quick --quiet` before each commit, no version bumps or tags by hand, commit per `git:workflow`.

## Phase 0 - de-risk (US1; first, D14)

- [ ] T001 [US1] Create `rust/Cargo.toml`, `rust/rust-toolchain.toml` (copy rstory's channel, add six targets), `rust/crates/ai-gent-core/{Cargo.toml,src/lib.rs}` and `rust/crates/ai-gent/{Cargo.toml,src/main.rs}` with only `ai-gent version`. Files above. Verify: `cd rust && cargo build --locked && ./target/debug/ai-gent version --json`. Satisfies contracts/cli.md `version`.
- [ ] T002 [US1] `ai-gent doctor` throwaway (four checks) in `rust/crates/ai-gent/src/cmd/doctor.rs`. Verify: `cargo test -p ai-gent` with `tests/cli.rs` (assert_cmd) for exit codes 0 and 1.
- [ ] T003 [US1] `rust/.cargo/config.toml` static CRT for Windows; `build.rs` emitting `AI_GENT_TARGET`. Verify: `cargo build --target x86_64-unknown-linux-musl` locally if the target is installed, else CI in T004.
- [ ] T004 [US1] `.github/workflows/rust-build.yml` (reusable, six legs, smoke, upload) and `.github/workflows/rust.yml` (dispatch only at this stage). Actions pinned by SHA copied from `release.yml`. Verify: `gh workflow run rust.yml --ref <branch>` then `gh run watch`; six green legs (P1 in `de-risk.md`).
- [ ] T005 [US1] Run probes P2-P10 with the owner; fill `specs/032-rust-workspace-and-release/de-risk.md`. Owner-run on real machines. Verify: no empty row.
- [ ] T006 [US1] Write the signing decision into `de-risk.md` and, if not "unsigned", add the signing step as a follow-up task in this file. Verify: SC-001; owner approves in the PR.

## Phase 1 - workspace and gates (US2)

- [ ] T010 [US2] Workspace metadata: `[workspace.package]` (license, `homepage` per D22, edition 2024, rust-version, repository), `include` lists, `[workspace.lints]` (`unsafe_code = forbid`, clippy groups), `ai-gent-core` modules from `plan.md` (`output.rs`, `exit.rs`, `version.rs`, `paths.rs`). Verify: `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] T011 [P] [US2] `rust/deny.toml` from rstory's: license allow-list, bans (openssl, openssl-sys, native-tls, reqwest, hyper, ureq, plus the dbus/xcb entries removed), sources. Verify: `cargo deny check` (and a throwaway branch adding `openssl` fails it).
- [ ] T012 [P] [US2] `--json` envelope and exit-code enum with snapshot tests: `rust/crates/ai-gent-core/src/output.rs`, `exit.rs`, `rust/crates/ai-gent/tests/cli.rs` (`insta`). Verify: `cargo test --workspace --locked`; satisfies contracts/cli.md.
- [ ] T013 [US2] `scripts/check.sh` Rust section: fmt, clippy, test (`--quick` skips test), deny; warn and skip when `cargo` is missing; one `pass`/`fail` line per gate. Verify: `scripts/check.sh --quick --quiet` with and without `cargo` on `PATH` (`PATH=/usr/bin:/bin scripts/check.sh --quick`).
- [ ] T014 [US2] `.github/workflows/check.yml`: `changes` step and Rust steps in the existing ubuntu job, cache keyed on `rust/Cargo.lock` + toolchain. Verify: open a PR touching only a SKILL.md and one touching `rust/`; the first shows the Rust steps skipped.
- [ ] T015 [US2] Release profile (`opt-level`, `lto`, `strip`, `codegen-units`) and size measurement for the host target; record numbers in `data-model.md` size budget. Verify: `ls -l rust/target/release/ai-gent`.
- [ ] T016 [US2] Measure the check time before/after on three PR kinds; record in the PR (SC-002). Verify: `gh run list` durations.
- [ ] T017 [US2] `scripts/consistency.py` (and `scripts/test_consistency.py`): `rust/` is not scanned as a plugin; crate `license` equals the repo's. Verify: `python3 -m unittest scripts/test_consistency.py`.

## Phase 2 - packaging and installer (US3, US4)

- [ ] T020 [US3] `scripts/package.py`: `--target <triple>`, `--binary <path>`, `--content-only`; zip for `*-windows-msvc`; refuses any path under `rust/` and any `*.rs`/`Cargo.*`/`target/`; MANIFEST includes the binary; deterministic. Verify: `python3 scripts/package.py --check`; two builds byte-identical.
- [ ] T021 [P] [US3] `scripts/release.py` for D23 (never bump by hand): (1) write the release version into `rust/crates/ai-gent/Cargo.toml` and its `Cargo.lock` entry in the release commit; (2) bump `ai-gent-core`, and `ai-gent-dbrun` when that crate exists (D28), from conventional commits touching `rust/crates/<crate>` (below 1.0.0: breaking = minor, otherwise patch; no bump when none touch it); (3) keep the binary's version requirement on each library crate equal to that crate's version, and fail the release (exit 1, nothing written) when they differ; (4) `--dry-run` text and `--json` list both as targets (`kind` `rust`); (5) `release.json`/`consistency.py` keep checking the JSON keys. Tests in `scripts/test-release.sh` against a temp repo with a `rust/` fixture: lockstep write, core breaking-minor and patch, no-touch no-bump, mismatch failure, dry-run preview. Verify: `scripts/test-release.sh` and `python3 scripts/release.py --dry-run --json`.
- [ ] T022 [US3] `scripts/test-package.sh`: fixture archives for all six triples built from stub binaries (a shell script renamed), served over `file://`; assert contents, no Rust sources, `bin/` mode. Verify: `scripts/test-package.sh`.
- [ ] T023 [US4] `install.sh`: platform map (`uname -s/-m`, `AI_GENT_TARGET`, `hw.optional.arm64`), fetch per-triple archive, verify, `bin/` check, unsupported-platform failure, keep content-only fallback if the per-triple asset is absent (2.x only). Verify: `scripts/test-install.sh` and `scripts/test-package.sh`, cases listed in US4.
- [ ] T024 [US4] `setup.sh`: link `~/.local/bin/ai-gent` or print the PATH line; `--uninstall` removes it; `--dry-run` shows it. Verify: `scripts/test-install.sh` new checks; shellcheck through `scripts/check.sh`.
- [ ] T025 [US4] macOS job in `check.yml` runs the per-triple install test under the system bash 3.2. Verify: the `macos` job green.

## Phase 2b - Windows installer (US7, D24; lands before 3.0.0, may follow 2.27)

- [ ] T050 [US7] `scripts/package.py`: dereference symlinks for `*-windows-msvc` (FR-021) and keep the dangling/escaping refusal; test in `scripts/test-package.sh` (the zip has no symlink entries, the 77 reference files are regular). Verify: `scripts/test-package.sh`.
- [ ] T051 [US7] `install.ps1` per `contracts/install-ps1.md` (function wrapper, steps 1-7, junction flip, `-List/-Rollback/-Prune/-Version/-VerifyAttestation`). Verify: `scripts/test-install-ps1.ps1` on a Windows machine, then in CI (T054).
- [ ] T052 [US7] Inventory what `setup.sh` links for Claude Code and Codex (names, directories, the stub files for Codex) in `specs/032-rust-workspace-and-release/de-risk.md` appendix or a new `windows-setup.md`; mark unverified items. Verify: reviewed by the owner.
- [ ] T053 [US7] `setup.ps1` for tiers 1-2 and the user `PATH` entry; `-Uninstall`; `-DryRun`. Verify: cases in `scripts/test-install-ps1.ps1`.
- [ ] T054 [US7] `.github/workflows/check.yml`: required `windows` job (windows-2025, plus windows-11-arm) running `scripts/test-install-ps1.ps1`. Verify: both legs green on a PR.
- [ ] T055 [US7] `release.yml`: upload and attest `install.ps1`; `SHA256SUMS` includes it; shape-test checks. Verify: `scripts/test-rust-workflows.sh`.
- [ ] T056 [US7] `ai-gent update` on Windows fetches and runs the target release's `install.ps1` (coordinate with 034 T020). Verify: 034 test on a Windows leg.
- [ ] T057 [US7] Document WSL/Git Bash as the alternative route; README and install page tasks live in 039. Verify: 039 T041/T043.
- [ ] T058 [US7] Owner check on a real Windows 11 machine and a Windows 11 arm64 machine without Git Bash: the one-liner works end to end (US7 scenario 6). Owner-run. Verify: transcript in the PR.

## Phase 3 - release integration (US3, US5)

- [ ] T030 [US3] `.github/workflows/release.yml`: `rust-build` job, `package` per target + content-only, `SHA256SUMS`, attest all subjects, publish after upload; `git add` gains `rust/Cargo.toml rust/Cargo.lock`. Verify: `scripts/test-rust-workflows.sh` (T032) and a `rust.yml` rehearsal.
- [ ] T031 [US3] Release job: `git add` of both Cargo.toml files and `rust/Cargo.lock` (written by T021, no cargo needed in the job; `cargo metadata --locked` in `rust-build` proves the lock is consistent). Verify: temp-repo test in `scripts/test-release.sh`; the shape test checks the `git add` list.
- [ ] T032 [P] [US3] `scripts/test-rust-workflows.sh` per `contracts/workflows.md`; wire in `scripts/check.sh` after the other shape tests. Verify: `scripts/test-rust-workflows.sh` prints `N/N` and exits 0; mutating a SHA pin to a tag makes it fail.
- [ ] T033 [US5] `publish-crates` job (gated by `vars.AI_GENT_CRATES_PUBLISH`), idempotent against crates.io, core first, then `ai-gent-dbrun` when it exists in the workspace (D28), then the binary, each after an index wait (D23). Verify: shape test; `cargo publish --dry-run -p ai-gent-core` in a `rust.yml` dry-run job (the bin crate's dry run waits until core exists at that version: use `cargo package --workspace` if the pinned cargo supports it [ASSUMPTION]).
- [ ] T034 [US3] `docs/` and `shared/releasing.md` "If the release ships files" paragraph: per-target assets and attestation; `shared/release-platforms.md` unchanged. Verify: `scripts/check.sh` (links, consistency).
- [ ] T035a [US3] Release notes: append the `## Checksums` block (D25, FR-023) in the `package` job; shape test checks. Verify: a rehearsal run shows the block in the draft release (rehearsal may create no release: assert on the generated file in `dist/`).
- [ ] T035 [US3] Rehearsal run on `dev` per `quickstart.md`; paste the run URL and asset list in the PR. Verify: SC-005.

## Phase 4 - owner steps and 2.27 (US5, US6)

- [ ] T040 [US5] Verify Trusted Publishing (the owner configured it on crates.io for both crates, and for `ai-gent-dbrun` (D28), with repository `lucasvmigotto/ai-gent`, workflow `release.yml`, GitHub environment `crate-release`): run a `rust.yml` dry-run publish job, then the first real publish (a 0.1.1 or the first real release, whichever comes first, with `AI_GENT_CRATES_PUBLISH=true` set by the owner for that run). Owner action: the GitHub environment `crate-release` exists in the repository settings, optionally with a required reviewer. Verify: the publish job obtains a token through OIDC and crates.io lists the new version with no stored token.
- [ ] T033a [US5] Metadata check before every publish: a step in the `rust.yml` dry-run job and in `publish-crates` prints the packaged `Cargo.toml` and fails unless `homepage` is the docs site, `repository` is the GitHub URL, and `license`, `readme`, `keywords`, `categories` are present, for every published crate (shape test `scripts/test-rust-workflows.sh` asserts the step exists). Verify: `cargo publish --dry-run -p ai-gent-core` output shows the fields; the shape test passes.
- [ ] T041 [US6] Add `## Unreleased` notes (user-facing) to `CHANGELOG.md`; PR `dev` -> `main`. Verify: `python3 scripts/release.py --dry-run` plans 2.27.0 (a `feat:`).
- [ ] T042 [US6] After the release: install on a clean Linux container with `install.sh`, run `ai-gent version --json` and `gh attestation verify` on every asset (SC-003, SC-007). Verify: transcript in the release PR.
- [ ] T043 [US6] Record the 2.27 outcome (time, failures) at the end of `quickstart.md` and update `specs/README.md` status to Implemented only with evidence. Verify: `scripts/check.sh`.
