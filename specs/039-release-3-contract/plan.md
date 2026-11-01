# Implementation Plan: The 3.0.0 release contract

**Branch**: `docs/specs-rust-core` (spec) | **Date**: 2026-11-01 | **Spec**: `spec.md`

## Summary

3.0.0 is the major release earned by the breaking contract (D20): package-only install, tiers in force, the binary replacing the shell guards and the first Python tools. This plan sequences the work as three gated releases (2.27, 2.28, 3.0.0), says which spec owns each piece, and turns every documentation consequence into a task.

## Technical context

**Release machinery**: `scripts/release.py` (commit types decide; `feat!` or a `BREAKING CHANGE:` footer is major), `.github/workflows/release.yml`, `scripts/package.py`. **Tests used as gates**: `scripts/check.sh`, `scripts/test-install.sh`, `scripts/test-package.sh`, the new `scripts/test-migrate.sh`, `scripts/test-update.sh` (034), the guard fixtures (033), `rust.yml`. **Constraints**: AGENTS.md/CLAUDE.md, `shared/releasing.md`, D1-D21.

## Constitution and constraint check

| Constraint | Check | How |
|---|---|---|
| D1 package-only in 3.0.0 | pass | B1, B2, FR-003 |
| D2 tiers, nothing removed | pass | B10, `contracts/tier-policy.md` |
| D7 per-subcommand cutover, no dual paths | pass | B7 ships per tool with its parity gate and deletion |
| D8 missing binary fails with the install command | pass | B4 |
| D13 opencode calls the binary | pass | B6 |
| D16 bootstrap stays POSIX | pass | the migration uses `install.sh` and `setup.sh` |
| D18 docs-site v2 | pass | G30-8 requires a minimal version |
| D20 major earned by the contract | pass | breaking list, `BREAKING CHANGE:` footer |
| D21 naming | pass | FR-011 grep gate |
| `shared/releasing.md`: no hand tags or bumps; notes under `## Unreleased` | pass | G30-10, G30-12 |
| CLAUDE.md "propose first" | pass | every task in `tasks.md` is a proposal-first change |

## Sequencing

```
2.27  032 foundation + 033 shadow         (G27)         binary exists, nothing user-visible breaks
2.28  033 cutover + 034 + first tools     (G28)         announce 3.0.0; B3-B6, B8 (update-check)
3.0.0 remaining tools, git mode removed,  (G30)         B1, B2, B7 (rest), B8 (rest), B9, B10, B11
      tiers, docs v2, content asset gone
```

Why this order: the binary must ship and be observed (2.27) before it decides (2.28); the user-facing migration is only safe when the update path already installs a fresh installer (G28-7) and every user has had a release with the binary present; the destructive removals (git mode, content-only asset, Python files) are the last step.

## Where each requirement is built

| Need | Spec / task |
|---|---|
| Binary, release matrix, packaging, installer | 032 |
| Guard evaluator, shadow, cutover, opencode | 033 |
| Update, doctor, retire shell updaters | 034 |
| Ported tools and their parity gates | 035-038 (other builder) |
| Docs-site v2 | 040 (other builder) |
| `install.sh` git mode removal, new messages | 039 T010-T013 |
| `setup.sh --migrate` extension, `scripts/dev-bin.sh`, `scripts/test-migrate.sh` | 039 T014-T017 |
| Tier CI split, compatibility matrix | 039 T030-T033 |
| Docs and `shared/*.md` | 039 Phase 3 |
| Naming grep gate | 039 T040 |
| Rollback rehearsal | 039 T050-T052 |

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| 2.26-2.27 users jump to 3.0.0 with the old updater | download error | safe failure (nothing written); 2.28 announces the major; README row; open question 2 |
| A tool port misses its parity gate and blocks 3.0.0 | delay | G30-2 lists whatever 035-038 delivered; a tool not ready keeps its Python file and is not part of 3.0.0 (D7: no dual paths per tool, but tools are independent) |
| Windows ARM or Intel macOS runner label changes | red required leg | gate waiver with owner signature, or cross-build fallback |
| `release.py` bumps every plugin to a new major because of one breaking commit | noisy changelog | expected; notes explain; check with `--dry-run --json` (G30-10) |
| Docs drift (the README and docs-site tier tables disagree) | credibility | one source, `contracts/tier-policy.md`, checked by a script (T031) |
| Tier-3 job red for weeks | ignored signal | the job opens an issue; policy item 4 sets the drop rule |
| The merge to `main` is the release; a late discovery cannot be undone cleanly | public bad release | rollback plan rehearsed (FR-008); `dev` rehearsal of the whole release with `rust.yml` and `test-migrate.sh` |
| Codex hook support unknown | tier-2 promise unclear | T032 verifies; the policy has both outcomes written |

## Phased approach

- **Phase 0 - gates and fixtures**: gate register, fixture homes, `test-migrate.sh` skeleton against today's 2.26 behavior (green before any change).
- **Phase 1 - installer and migration (lands in 2.28 and 3.0.0)**: git-mode removal messages, `--migrate` extension, `dev-bin.sh`.
- **Phase 2 - tiers and CI split**.
- **Phase 3 - documentation and shared references** (as tasks).
- **Phase 4 - gates, rehearsals and the release**: G27, G28, G30 walkthrough; rollback rehearsal; the PR to `main`.
