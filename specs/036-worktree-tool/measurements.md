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

## T052: manual check against real podman (owner's machine, 2026-11-14)

Binary: `ai-gent 0.1.0 (x86_64-unknown-linux-musl)` from the `rust.yml` run 43623830008 artifact. Rootless podman; 57.4 GB free on the container store (floor 24.1 GB).

| Step | Command | Result |
|---|---|---|
| add | `ai-gent wt add chore/wt-manual-check --base origin/dev` | Refused, exit non-zero: `3 track(s) already hold a volume each; the cap is 3 (max_volumes in .ai-gent/parallel.yaml). Land or remove one first`. Three worktrees were live (the cap counts live worktrees), so the check was not run past the cap and the disk floor and engine path of `add` were not exercised here. The cap itself is the expected behavior. |
| create the worktree instead | `git worktree add .worktrees/chore-wt-manual-check -b chore/wt-manual-check origin/dev` | Created by hand (not through `wt add`). |
| real labelled volume | `podman volume create --label me.lucasvmigotto.ai-gent.project=ai-gent --label me.lucasvmigotto.ai-gent.worktree=ai-gent/chore-wt-manual-check wt-manual-check-vol` | Created. Before the worktree existed, `wt resources` listed it as `track chore-wt-manual-check (worktree gone)`; after, without that note. |
| list | `ai-gent wt list` | Four worktrees, the new one `+0/-0 vs dev`. |
| remove | `ai-gent wt remove chore/wt-manual-check` | exit 0: `removed .worktrees/chore-wt-manual-check and branch chore/wt-manual-check`, `removed volume wt-manual-check-vol`. |
| after | `podman volume ls`, `git branch --list`, `wt resources` | The volume, branch and directory are gone; 0 labelled containers. Nothing else was touched. |

Verdict: list and remove with a real labelled volume work against real podman. `add` was blocked by the worktree cap, so its real-engine path remains unchecked; T052 is ticked for add (cap refusal), list and remove, with that gap noted.
