# Quickstart: how parity and speed are proven

## The parity chain

```
tests today                      fixtures                         implementations
scripts/test-*-guard.sh   --T010-->  shared/guard-fixtures/*.json  --> guard.sh (oracle)   until the cutover
opencode rules.test.mjs   --T010-->                                 --> rules.mjs           until the cutover
                                                                    --> ai-gent guard       (Rust), all shells
```

1. The extraction (T010) runs every vector through the real `guard.sh` and records its decision and reason, so the fixtures reflect behavior, not just what the old tests asserted.
2. The old harnesses are rewritten onto the fixtures (T012, T013) and must keep their case counts: a lost vector shows as a lower count in `scripts/check.sh` output.
3. The Rust suite and `--impl bin` run the same files (T028, T027).
4. Mutations (T044) compare oracle and binary on variations nobody wrote by hand.
5. Shadow mode (T040-T046) compares them on real usage.

## Commands

```sh
python3 scripts/guard-fixtures.py lint
python3 scripts/guard-fixtures.py run --guard all --impl sh      # oracle
python3 scripts/guard-fixtures.py run --guard all --impl bin     # binary through the hook path
cargo test -p ai-gent-core --test guard_parity                   # in-process
ai-gent guard shadow-report --json
python3 scripts/bench-guard.py --impl sh --json
python3 scripts/bench-guard.py --impl bin --check
```

## Latency baseline (2026-11-01)

Measured by the author on the development host (Linux 6.12, one machine, 100 sequential runs per guard, input `{"tool_name":"Bash","cwd":"/tmp","tool_input":{"command":"git status --short"}}`, mean per run including `sh` start). Not verified on macOS or Windows.

| Guard | Mean per run |
|---|---|
| git | 3.3 ms |
| db | 1.1 ms |
| secrets | 0.8 ms |
| repo | 0.6 ms |
| devcontainer | 1.6 ms |

T001 replaces this with p50/p95/p99 from `bench-guard.py` on each OS. Budget: FR-012.

## Shadow evidence to collect for promotion (FR-010)

Per guard: evaluations, agreements, disagreements, expected divergences; the corpus replay counts; the CI fixture counts per OS; the bench table. Kept in this file when T046 runs.

## What is not provable offline

Claude Code's own behavior on a hook failure (whether a non-zero exit blocks, or only reports) and the Windows hook shell. T051/T034 observe both on a real install and record the answer in `spec.md`.
