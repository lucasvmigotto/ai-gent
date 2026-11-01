# Contract: release gates, 2.27 -> 2.28 -> 3.0.0

Each gate is a row with a **measure** that is a command, a file or a CI run, and an **evidence** cell the builder fills with a link or pasted output. The release PR is not opened with an empty evidence cell, unless the owner signs a waiver with a reason. Release flow itself: `shared/releasing.md` (nobody tags or bumps by hand).

## G27 - before cutting 2.27 (the Rust foundation, shadow guards)

| Id | Criterion | Measure | Evidence |
|---|---|---|---|
| G27-1 | Workspace gates pass | `scripts/check.sh` full; `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace --locked`, `cargo deny check` | |
| G27-2 | Rust job skipped on non-Rust PRs; check time not worse | CI run durations on three PR kinds (032 SC-002) | |
| G27-3 | Six-target dry run green | `gh workflow run rust.yml --ref dev`; six `bin-<triple>` artifacts | |
| G27-4 | De-risk sheet complete with a decision | `specs/032-rust-workspace-and-release/de-risk.md` has no empty row | |
| G27-5 | Per-triple install tests pass | `scripts/test-package.sh`, `scripts/test-install.sh`, macOS job | |
| G27-6 | Trusted Publishing configured for both crates; dry-run publish passes; publish variable unset | crates.io settings screenshot or text; `rust.yml` dry-run job | |
| G27-7 | Shadow wired, decisions unchanged | `scripts/test-*guard.sh` unchanged counts (192 shell cases); `scripts/guard-fixtures.py run --impl sh` green | |
| G27-8 | Fixtures extracted with no loss | `shared/guard-fixtures/manifest.json` counts equal the old suites | |
| G27-9 | Guard parity for sh | `cargo test -p ai-gent-core --test guard_parity` 100 percent | |
| G27-10 | Release notes under `## Unreleased` | `python3 scripts/release.py --dry-run` plans 2.27.0 | |

## G28 - before cutting 2.28 (the binary decides)

| Id | Criterion | Measure | Evidence |
|---|---|---|---|
| G28-1 | Shadow promotion criteria met (033 FR-010) | `ai-gent guard shadow-report --json`: 0 unexplained disagreements, at least 500 evaluations per guard; corpus replay counts | |
| G28-2 | Fixture suite green on Linux, macOS, Windows (x64 and arm64) | CI run of `rust-build.yml` | |
| G28-3 | Latency budget met on three OSes | `scripts/bench-guard.py --impl bin --check` outputs (033 FR-012) | |
| G28-4 | Failure policy tested | 033 SC-006 (binary renamed: every guard asks) | |
| G28-5 | Update and doctor | `cargo test --test update_check --test update_apply --test doctor`, `scripts/test-update.sh` | |
| G28-6 | `test-update-check.sh` inventory fully covered | `specs/034-update-and-doctor/quickstart.md` table, then the script deleted | |
| G28-7 | `ai-gent-update.sh` fetches the target release's installer, and the 2.28 notice announces 3.0.0 as a major | `scripts/test-update.sh` case | |
| G28-8 | Each tool cutover merged so far met D10's parity gate before its Python file was deleted | per-tool spec evidence (035-038) | |
| G28-9 | Migration guide drafted; `scripts/test-migrate.sh` passes against a 3.0.0 candidate package | script output | |
| G28-10 | `scripts/dev-bin.sh` exists and a fresh clone passes the guard tests after it | run in a throwaway clone | |
| G28-11 | Six-target release succeeded for 2.27 (the real run) | the 2.27 release run URL; all assets attested | |

## G30 - before the merge that releases 3.0.0

| Id | Criterion | Measure | Evidence |
|---|---|---|---|
| G30-1 | 2.28.x in use with the binary deciding, no guard regression reported | issues query; shadow log clean until cutover; owner statement | |
| G30-2 | All ported tools meet D10 and their Python files are deleted; no skill references a deleted file | `grep -rEn 'wt\.py|token_report\.py|reporun\.py|specsync\.py|compact\.py' plugins shared README.md AGENTS.md docs/site/src` empty except the migration guide and changelog (the exact list is whatever 035-038 delivered) | |
| G30-3 | Git mode removed, installer messages updated, tests updated | `scripts/test-install.sh`, `scripts/test-package.sh`, `scripts/test-migrate.sh` | |
| G30-4 | Tier policy in force in CI | required checks list in the repository settings equals FR-012; `tier3` job exists and is non-required | |
| G30-5 | Windows legs required and green for the last three runs of `rust.yml` plus the 2.28 release | run URLs | |
| G30-6 | Migration tested from every fixture home | `scripts/test-migrate.sh` in CI | |
| G30-7 | Rollback rehearsal passes | `scripts/test-migrate.sh --rollback` case (FR-008) | |
| G30-8 | Docs: every task in `tasks.md` Phase 3 done; docs-site v2 minimal Install page and matrix live | `scripts/check.sh`; docs CI; page URL | |
| G30-9 | Naming rule | grep gate in `scripts/consistency.py` (FR-011) green | |
| G30-10 | Release planned as major | `python3 scripts/release.py --dry-run --json` prints `"version": "3.0.0"` | |
| G30-11 | All items in `docs/product/rust-decisions.md` "Open" answered or moved | file reviewed by the owner | |
| G30-12 | CHANGELOG `## Unreleased` holds the breaking-changes list verbatim and the merge commit carries `BREAKING CHANGE:` | `git log -1 --format=%B` of the merge | |
| G30-13 | Crates decision recorded: publish switch on or off, and why | note in the release PR | |
| G30-14 | `grep -rn 'AI_GENT_MODE' install.sh setup.sh scripts` shows only the removal message and its test | grep | |

## Rollback

User level (any release):

1. `ai-gent update --rollback` (or `sh install.sh --rollback`): `current` and `previous` swap, `setup.sh --skip-prompt` relinks the agents. The previous version directory is still on disk because `--prune` is never automatic.
2. A user who skipped 2.28 and has a broken 3.0.0: `sh install.sh --rollback` works because the old version directory exists; git-mode users never lost their clone (the migration does not touch it).

Project level (3.0.0 found defective after publishing):

1. Do not move or delete the tag. Mark the release a pre-release and make the last good one latest: `gh release edit 3.0.0 --prerelease` and `gh release edit <last 2.x> --latest` [KNOWN flags; run them once in a scratch repo first, T060].
2. `install.sh` resolves `latest` from the redirect, so new installs get the last 2.x again.
3. Fix forward on `dev`, merge, let the bot release 3.0.1; the new release becomes latest.
4. Crates: if published, yank the bad version (`cargo yank`, owner-run through the runner pattern) and publish the fix; nothing to do while the publish switch is off.

Support window for 2.28.x: none beyond one patch for an installer defect (open question 3).
