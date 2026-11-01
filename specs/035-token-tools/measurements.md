# Measurements: 035 token tools

Status: synthetic corpus measured (T051, T052). The real-corpus baseline (T002) needs the owner's machine and is not done.

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
| Real corpus (T002) | not measured |
