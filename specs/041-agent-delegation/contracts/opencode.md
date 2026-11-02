# OpenCode adapter

Observed on this machine (2026-11-02): OpenCode v2.2.5 at `~/.opencode/bin/opencode`.

## Facts and what they mean for the adapter

| Observed | Consequence |
|---|---|
| The binary is in `~/.opencode/bin`, **not on `PATH`** in the supervisor's shell. | `detect` checks `PATH` and the known install dirs (`~/.opencode/bin`), never assuming `PATH`. |
| `opencode run [flags] [message...]` with `--model provider/model#variant`, `--format json`, `--agent`, `--session`, `--auto`. | One non-interactive call per brief; read the JSON events for progress and usage (event shape to be verified in the first trial). |
| `--auto` approves permissions **not explicitly denied**. | Never pass it alone: the run needs an explicit permission set that denies what a delegate must not do (below). |
| There is **no OS sandbox flag** (Codex has `-s workspace-write`). | Containment is the permission set and the worktree, not the OS. This is weaker than Codex's sandbox. |
| Native V2 config has an ordered `permissions` array of `{action, resource, effect}` rules and a `plugins` array; `setup.sh` already writes ai-gent's rules and loads `opencode/guard` (the V2 guard plugin) for the opencode target. | OpenCode can **enforce** ai-gent's guards, which Codex cannot. For a delegate this is the main advantage, but only if that config is in place for the run. On this machine `~/.config/opencode` holds only `service.json`: ai-gent is **not** set up for opencode here. |
| The model list shows seven free models: `opencode/flint-40b-free` (default), `nemotron-3.5-super-vl-free`, `beam-free`, `mimo-v2.6-flash-free`, `deepseek-v4.1-flash-free`, `glm-5.3-flash-free`, `nemotron-3.5-lightning-free`. | These are the owner's chosen models. Quality varies and none has been tried. |

## Run shape (to verify in trial)

`opencode run --model opencode/<model> --format json "<brief>" < /dev/null` from the worktree, with stdout and stderr to files under `.ai-gent/delegate/<track>/`, a PID file, and the supervisor's clock. Permissions come from a per-run config that is **not** committed (an env or config-dir override; the exact switch is verified in trial) so the user's own OpenCode config is never edited silently (open question 3).

## Permission set for a delegate (deny-by-default intent)

Allow: read; edit inside the worktree; shell commands the task needs (build, test, `git status|diff|add`). Deny: `git push`, `git commit --no-verify`, `git reset --hard`, `git branch -D`, network clients except the model call, database clients and container engines, reading `~/.ssh`, any `pass`/gpg commands and files named like credentials. The supervisor commits and pushes (as for Codex), so the delegate needs no commit rights.

## Roles for free models

Treat the free models as **researchers, first-pass reviewers and builders of small, well-specified tasks** until trials show otherwise. Hard tasks stay with Claude or Codex. A delegate's output from a free model always gets the mandatory cross-agent review (FR-010); the review of a Claude or Codex change by OpenCode is a cheap extra opinion, never the only gate.

## Privacy caveat (unverified)

Free hosted models often use prompts and completions to improve their provider's models. Whether that applies to these seven has **not been checked**. Until it is, the skill must send OpenCode free models only content that is already public (this repository is public) and must refuse tasks that include private repositories, credentials, customer data or anything under `.ai-gent/`. The opt-in screen says so.

## Trial plan

1. A read-only task first (the T002 driver survey draft, researched by OpenCode with its own web access, reviewed by Claude), to read the JSON events, confirm the permission switch and record time and any usage.
2. Then one small build task inside `allowed_paths`, supervisor-committed, reviewed by Codex.
3. Compare with the Codex trials on the checklist; try more than one free model and record which is usable.
