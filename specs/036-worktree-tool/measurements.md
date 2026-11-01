# Measurements: 036 worktree tool

Status: Implemented through Phase 4 (2026-11-01). Phases 5 (T050-T052) and 6 (cutover) are open.

## Binary size (release profile, x86_64-unknown-linux-gnu, stripped)

| Build | Bytes |
|---|---|
| `dev` at 13bc8d0 | 1,870,472 |
| with `ai-gent wt` | 1,979,688 |
| Delta | +109,216 (+5.8%) |

No new dependency: `df`, `ps` and PowerShell are subprocesses, the JSON printer is the one from the token port.

## Parity

23 fixtures in `rust/fixtures/wt/` (16 tests plus 7 edge cases), recorded from wt.py by `scripts/test-parity-wt.py --bless`; wt.py and the binary agree on exit code, stdout, stderr, the fake engine's state and call log in all 23. No disagreements.
