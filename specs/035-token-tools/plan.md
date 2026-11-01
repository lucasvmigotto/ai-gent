# Implementation Plan: Token tools (`ai-gent tokens`)

**Branch**: `docs/specs-rust-tools` (spec) | **Date**: 2026-11-01 | **Spec**: `spec.md`

## Summary

Port `plugins/agents/scripts/token_report.py` to a `tokens` module in `ai-gent-core` and a thin `ai-gent tokens` subcommand, proving parity with the 68 Python tests turned into shared fixtures (D10), then switch the skills, the mods and `scripts/check.sh` to the binary and delete the Python file and its test in the same release (D7).

## Technical Context

**Language**: Rust (pinned in `rust-toolchain.toml` by 032). **Crates** (candidates, 032 decides the policy): `serde`, `serde_json` (transcripts and history), `clap` (derive; `allow_hyphen_values` for `--project`), `chrono` or `time` (UTC), `regex`, `rayon` (parallel file scan, optional behind a measurement), `dirs`/`home` (config dirs), `tempfile` (tests and atomic writes). No network crate. No async runtime needed: the work is file I/O and CPU.
**Storage**: reads `~/.claude*/projects/**/*.jsonl`; reads/writes `docs/product/token-history.jsonl`, `token-snapshots.json`, `specs/*/tasks.md`, `specs/*/tracker.md`, `.ai-gent/orchestrate-status.md`, `~/.ai-gent/token-history.jsonl`; reads `.ai-gent/budget.yaml`, `.ai-gent/orchestrate.yaml`, `specs/*/cards.md`, `shared/token-tags.json` (embedded).
**Testing**: `cargo test` over fixtures in `rust/fixtures/tokens/` (format in `contracts/parity-fixtures.md`), plus a Python oracle runner `scripts/test-parity-tokens.py` that runs the same fixtures through `token_report.py` until it is deleted.
**Platforms**: Linux, macOS, Windows, x86_64 and aarch64 (D3).

## Module layout (names follow 032's workspace; `rust/` prefix per D4, exact crate paths assumed)

```
rust/ai-gent-core/src/tokens/
  mod.rs          public API: Usage, Message, Session, Row, Card, Record
  transcripts.rs  discover_sessions, collect (dedupe by id), config_dirs, meta sidecars   [FR-003, FR-004]
  attribute.rs    expand_ids, attribute, build_groups, layer_of, effort_of                 [FR-005]
  coordinator.rs  windows, split_coordinator, git markers (shells out to git), snapshots   [FR-006]
  tags.rs         vocab (embedded token-tags.json), parse_tags, format_tags, norm_stack, infer_tags
  history.rs      Record, read/upsert/load_history, index, seed                              [FR-008]
  estimate.rs     estimate_card, estimate_by_tags, context factors, bias, starting tables    [FR-010]
  budget.rs       parse_budget, usage_messages, usage_rate, wave_cards, verdict, pack_batches [FR-011]
  writers.rs      write_task_lines, write_tracker, write_ledger (atomic), legacy output      [FR-007]
  report.rs       render, summary_lines, idle_rebuilds
rust/ai-gent/src/cmd/tokens.rs   clap definitions, text rendering, --json, exit codes
rust/fixtures/tokens/            parity fixtures
scripts/test-parity-tokens.py    Python oracle runner (deleted with token_report.py)
scripts/bench-tokens.sh          measurement (not in CI)
```

Git for stage markers: shell out to `git` (same decision and reasons as 036: byte-identical output with the Python, one code path, no libgit2 build cost, Windows-friendly).

## Constraint check

| Constraint | Result |
|---|---|
| D7 per-subcommand cutover, no dual paths | Cutover steps below; Python deleted in the same release as the skill edits |
| D9 `--json` everywhere, documented exit codes | `budget`, `tasks --dry-run`, `estimate` keep their JSON; exit codes in `contracts/cli.md`. `report`, `index`, `seed` gain `--json` summaries only if a mod needs them (not planned; open) |
| D10 parity gate | 68 tests become fixtures (table below); new Windows cases added |
| D3 Windows | FR-015; line endings; atomic rename; slug |
| D16 dev-only tools stay | `scripts/check.sh` keeps running `cargo test` for the port; `test_token_report.py` deleted at cutover |
| AGENTS.md propose-first | The cutover edits to skills/hooks are listed here for owner approval before they are written |
| Secrets | the tool reads no secret and prints no transcript text beyond `desc` (FR-019/spec context) |
| `shared/token-tags.json` single source | embedded copy checked by `scripts/consistency.py` (new check, T012) |
| Package allowlist (`scripts/package.py`) | binary shipped by 032's package change; `plugins/agents/scripts/token_report.py` leaves the allowlist at cutover [verify the allowlist entry in T030] |

## Parity: Python tests become shared fixtures

Source: `plugins/agents/scripts/test_token_report.py`, 68 tests, 16 classes. Python tests mutate module globals and build sessions with helper methods (`TasksFixture.agent(...)`). The conversion:

1. Extract each fixture builder to a declarative scenario file `rust/fixtures/tokens/<case>/scenario.toml`: config dirs, sessions, agents (id, description, model, billable, timestamp, calls, edits), `cards.md`, `tasks.md`, `budget.yaml`, history, clock (`now`), env. A scenario materialises into a temp HOME with the same layout the Python helpers write.
2. `expect/` holds the files and stdout the Python tool produced (golden), generated once by `scripts/test-parity-tokens.py --bless` from the unmodified Python, then reviewed in the PR.
3. Both runners (`scripts/test-parity-tokens.py`, `cargo test -p ai-gent --test parity_tokens`) materialise the scenario, run the command, and diff stdout/files/exit code. Time is injected (`AI_GENT_NOW` env for tests only, ignored in release builds? open: a `--now` hidden flag is simpler; decided in T003).
4. Pure-function tests (`test_expand_ids_and_tokens_format`, `PctParityTests`, `UsageRateTest`, `test_parse_budget_*`) become Rust unit tests with the same inputs and expected values copied from the Python asserts, plus a table fixture `rust/fixtures/tokens/pure.json` consumed by both.

| Python class (tests) | Fixture group | Notes |
|---|---|---|
| TokenReportTest (10) | report-*, snapshot-* | `test_project_name_comes_from_the_slug...` needs cwd control: fixture sets cwd |
| TasksTest (7) | tasks-* | |
| EstimateTest (6), TagEstimateTest (9), TagsTest (3) | estimate-* | history files in fixture |
| TasksJsonTest (6) | tasks-json-* | byte compare |
| IndexSeedTest (3), LegacyTest (2) | index-*, seed-*, legacy-* | seed reads manifests of fake repos |
| EventsTest (1), UsageSinceTest (1) | events-*, usage-* | |
| UsageRateTest (4), PctParityTests (3) | pure.json | |
| BudgetTest (13) | budget-* | caps, rate, ETA, Opus risk |

Total 68 = 10+7+6+9+3+6+3+2+1+1+4+3+13 (counted from `grep -n "def test"` per class; the unittest run reports 68).
New cases (Rust-only until Python is deleted): Windows path slug, CRLF `tasks.md`, invalid UTF-8 line, 50 MB line, read-only target file, `--project=-slug` parsing.

## Measurement (performance win)

Claim to test: transcript scanning (`usage_messages`, `collect`) is the hot path; Python does `json.loads` per line over the week's files each call, and `budget` runs from a status line.

Baseline task (owner or a builder on the owner's machine; never committed, never read for content):
1. `find ~/.claude*/projects -name '*.jsonl' -mtime -7 | xargs du -ch | tail -1` records the corpus size (bytes and file count are the only things written down).
2. `hyperfine --warmup 2 --runs 10 'python3 plugins/agents/scripts/token_report.py budget --projected 400k --json'` (cold and warm page cache noted) records median and spread. The command prints only totals; no transcript text reaches the terminal. If `hyperfine` is missing use `scripts/bench-tokens.sh` (a loop with `date +%s%N`).
3. Record the table in `specs/035-token-tools/measurements.md`: machine, OS, corpus size, files, Python version, median, p95.
4. The Rust binary is measured the same way; SC-004 is judged on that table. Also measure a synthetic corpus generated from fixtures (so CI-free reproducibility): `scripts/bench-tokens.sh --synth 1GB`.
Expected shape [ASSUMPTION]: parse cost dominates; Rust `serde_json` with typed partial deserialization and `rayon` across files should be several times faster, but a warm-cache Python run on a small week may already be sub-second, in which case the win is not user-visible. If the win is under 2x, say so in the release notes and do not market it.

## Cutover (D7), exact steps

Proposed release: **3.0.0** (open question 2; subcommand ships in 2.28). One PR, one release:

1. Skills and references (11 files, from `git grep 'token_report\.py'`, excluding `specs/`, `docs/product/`, `CHANGELOG.md`):
   `plugins/agents/skills/orchestrate/SKILL.md` (1), `.../references/cards.md` (1), `.../context-discipline.md` (1), `.../token-economy.md` (2), `plugins/agents/skills/parallel/SKILL.md` (1), `plugins/project/skills/spec/references/feature-loop.md` (1), `plugins/project/skills/status/SKILL.md` (1), `shared/token-tracking.md` (8), `README.md` (2), `AGENTS.md` (1). The `TOKREP="python3 <plugin>/scripts/token_report.py"` variable becomes `TOKREP="ai-gent tokens"`.
2. Mods: `plugins/agents/hooks/{token-model.ts (5), tokens.ts (3), tokens.test.tsx (4), cards.ts (3), cards-model.ts (2), cards.test.tsx (1), cards-fixtures.ts (1), landing.ts (2), landing-model.ts (1), landing.test.tsx (4), register.tsx (2), pct-fixtures.ts (2)}` and `plugins/project/hooks/{handoff.ts (3), handoff-model.ts (1), handoff.test.tsx (2)}`, `plugins/project/types/index.d.ts` (1). The mods locate the script at `${$.plugin.root}/../agents/scripts/token_report.py` (handoff.ts:82); they switch to `ai-gent` on PATH and degrade silently when absent.
3. Manifests: `plugins/agents/.claude-plugin/plugin.json` (1) and `plugins/project/.claude-plugin/plugin.json` (2) mention the script in descriptions; reword (versions untouched: releases bump them).
4. Scripts: `scripts/check.sh` (3 lines, the unittest block at lines ~275-279 becomes `cargo test` or is dropped if 032 runs cargo elsewhere), `scripts/consistency.py` (1 line, the hint text `token_report.py estimate`).
5. Guards: no guard pattern names `token_report.py` today [verified by the grep: zero hits in `plugins/*/hooks/guard.sh`, `opencode/guard/`]. Nothing to change; the `ai-gent` binary is itself covered by 032's guard work.
6. Delete `plugins/agents/scripts/token_report.py` and `plugins/agents/scripts/test_token_report.py`; delete `scripts/test-parity-tokens.py` in the same PR (its purpose ends; fixtures stay for Rust).
7. `CHANGELOG.md` `## Unreleased`: breaking note. No version bump by hand.
8. Verify: `git grep -n 'token_report\.py' -- . ':!CHANGELOG.md' ':!specs' ':!docs/product'` is empty; `scripts/check.sh` green; `scripts/package.py --check` green.

Counts above come from `git grep -c` on 2026-11-01: 75 matching lines in 32 files including the script and its test. They differ from the "about 52" in `rust-decisions.md` because that count excluded the mods' fixtures and tests; re-run before the PR.

## Risks

| Risk | Mitigation |
|---|---|
| Float rounding differs (`round`, medians, weighted quantiles) | Fixtures compare formatted output; implement Python's banker's rounding (`round()` rounds half to even) explicitly; unit cases at .5 |
| Dict ordering in Python (`dict` insertion order) leaks into output | Use `IndexMap`/ordered structures where output order depends on insertion; fixtures catch it |
| `json.dumps` formatting (`ensure_ascii`, separators) differs from `serde_json` | Custom serializer settings to match (`ensure_ascii=True` is the Python default: non-ASCII escaped); byte-compare fixtures |
| Python `sorted` on mixed keys / stable sort | Use stable sorts and the same keys |
| Transcript format drifts (Claude Code updates) | The tool is already exposed to this; keep parse tolerant (`serde(default)`), add a canary fixture from a real (redacted) transcript |
| Windows slug and paths unverified | FR-015; needs a real run |
| No speed win | Report honestly; the distribution reason (D "Why") stands without it |

## Phased approach

- **Phase 0 (measure + oracle)**: baseline table, scenario format, bless script. Nothing Rust yet.
- **Phase 1 (US2, US5-part)**: transcripts, dedupe, report, snapshot. Smallest slice, proves the scanning layer and timing.
- **Phase 2 (US1)**: attribution, coordinator, writers (the riskiest: byte-identical files).
- **Phase 3 (US3, US5)**: tags, history, estimate, index, seed.
- **Phase 4 (US4)**: budget and `--json` for the mods.
- **Phase 5 (US6)**: benchmark, optimisation if needed.
- **Phase 6 (cutover)**: edits, deletion, release notes. Gated on SC-001..SC-006.
