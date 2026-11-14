# Measurements: 035 token tools

Status: synthetic corpus measured (T051, T052); real-corpus baseline measured on the owner's machine 2026-11-14 (T002, below).

## Binary size (release profile, `x86_64-unknown-linux-gnu`, stripped, opt-level z)

| Build | Bytes |
|---|---|
| `dev` @ ffc1be3 (version, doctor, guard) | 1,111,040 |
| this branch (adds `tokens`) | 1,749,792 |
| delta | +638,752 (+57%), well inside the 8 MB budget of spec 032 |

No new dependency: `serde`, `serde_json` and `regex` were already linked; the cost is the port itself (`cargo deny check` passes).

## SC-004: `budget --projected 400k --json`, synthetic corpus

`scripts/bench-tokens.sh --synth 300MB --runs 5` (219 files, 300,004,691 bytes, warm page cache, 8 cores, Python 3.13.5, release binary). Generated messages only; no transcript content.

| Implementation | Median | Slowest |
|---|---|---|
| `token_report.py` | 826 ms | 832 ms |
| `ai-gent tokens` first cut (tree JSON parser, sequential) | 1112 ms | 1138 ms |
| + typed borrowed scan (no tree, cheap reject of lines without `"usage"`) | 383 ms | 387 ms |
| + files read on up to 8 threads, merged in file order | 200 ms | 234 ms |

Rust is 4.1x faster than Python on this corpus (goal 3x). A real corpus has more non-assistant lines and larger tool results per line, which the cheap reject favours; the owner's run (T002) confirms or corrects this. A run on a small week (a few MB) is dominated by process start and is not user-visible either way.

| Measurement | Value |
|---|---|
| machine / OS / Python | dev container, Linux 6.12, 8 cores, Python 3.13.5 |
| corpus (files, bytes) | synthetic: 219 files, 300,004,691 bytes |
| Python `budget --json` median / p95 | 826 ms / 832 ms |
| Rust median / p95 | 200 ms / 234 ms |
| Real corpus (T002) | see the next section |

## T002: real-corpus baseline (owner's machine, 2026-11-14)

Only bytes, counts and timings were recorded; output went to /dev/null, no transcript text was read.

| Item | Value |
|---|---|
| machine / OS | 11th Gen Intel Core i7-1165G7 @ 2.80GHz, 8 threads, 7.6 GB RAM; Linux 6.12.111 (Debian 13) |
| Python | 3.13.5 |
| Rust binary | `ai-gent 0.1.0 (x86_64-unknown-linux-musl)`, the `bin-x86_64-unknown-linux-musl` artifact of the successful `rust.yml` run 43623830008 (branch `feat/dbrun-core-plan`, whose tip is merged into dev) |
| corpus | `find ~/.claude*/projects -name '*.jsonl' -mtime -7`: 38 files, 81,332,053 bytes (234 files in all, any age) |
| command | `budget --projected 400k --json` (Python: `plugins/agents/scripts/token_report.py`; Rust: `ai-gent tokens budget`), both give verdict `proceed`, total 400000, same rate figures |
| method | `hyperfine` is not installed; a shell loop with `date +%s%N` (20 runs, output to /dev/null), median and p95 by nearest rank |

| Implementation | First run | Warm median | Warm p95 | Warm min / max |
|---|---|---|---|---|
| `token_report.py` | 429 ms | 387 ms | 440 ms | 373 / 444 ms |
| `ai-gent tokens budget` | 136 ms | 141 ms | 152 ms | 134 / 154 ms |

Rust is 2.7x faster at the median (warm) and 3.2x on the first run, above the 2x line, below the 4.1x of the synthetic corpus (the real week is smaller and each run carries process start). Both finish well under a second.

Caveat: a true cold page cache needs root (`drop_caches`), which was not used. The "first run" is the first timed run of the session, after the corpus had been read once for the size count, so it is a lightly cold run and not a measured cold-cache figure.
