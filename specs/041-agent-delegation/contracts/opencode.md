# OpenCode adapter

> **Status:** the third adapter of the feature (FR-003), built in Rust (`delegate/opencode.rs`, `--agent opencode`) after trial 3 and tested only against a fake `opencode`; one real run so far ("Real run 1"), which found an isolation defect, since fixed in code and proven only with a stub ("Isolation fix"). ai-gent's own development does not use it: this repository's allowlist is Codex and a second Claude profile (FR-002, owner, 2026-11-02).

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

## Verified in trial 3 (2026-11-02, `opencode/flint-40b-free`)

- `OPENCODE_CONFIG_CONTENT` takes the inline per-run config (permissions array); `OPENCODE_CONFIG`, `OPENCODE_CONFIG_DIR` and `OPENCODE_CONFIG_PROJECT_DISABLE` also exist. No file is written and the user's config is untouched.
- `--standalone` gives the run a private server; without it the run goes through the shared background service (`service.json`).
- Events (`--format json`) are `step_start`, `tool_use`, `step_finish` and `text`; usage is in each `step_finish.part.tokens` (`input`, `output`, `reasoning`, `cache.read|write`) and `cost` (0 for free models), so cost is the **sum** over steps, unlike Codex's single final event.
- The run can exit **1 even when it wrote its file and finished**: judge by the final `step_finish` (`reason: stop`) and the diff, not the exit code alone.
- OpenCode has its own `websearch` and `webfetch` tools; `websearch` was cancelled three times and recovered. It also wrote the wrong file name three times before finding the right one.

## Run shape

`opencode run --model opencode/<model> --format json "<brief>" < /dev/null` from the worktree, with stdout and stderr to files under `.ai-gent/delegate/<track>/`, a PID file, and the supervisor's clock. Permissions come from a per-run config that is **not** committed (an env or config-dir override; the exact switch is verified in trial) so the user's own OpenCode config is never edited silently (open question 3).

## Permission set for a delegate (deny-by-default intent)

Allow: read; edit inside the worktree; shell commands the task needs (build, test, `git status|diff|add`). Deny: `git push`, `git commit --no-verify`, `git reset --hard`, `git branch -D`, network clients except the model call, database clients and container engines, reading `~/.ssh`, any `pass`/gpg commands and files named like credentials. The supervisor commits and pushes (as for Codex), so the delegate needs no commit rights.

## Roles for free models

Treat the free models as **researchers, first-pass reviewers and builders of small, well-specified tasks** until trials show otherwise. Hard tasks stay with Claude or Codex. A delegate's output from a free model always gets the mandatory cross-agent review (FR-010); the review of a Claude or Codex change by OpenCode is a cheap extra opinion, never the only gate.

## Privacy caveat (unverified)

Free hosted models often use prompts and completions to improve their provider's models. Whether that applies to these seven has **not been checked**. Until it is, the skill must send OpenCode free models only content that is already public (this repository is public) and must refuse tasks that include private repositories, credentials, customer data or anything under `.ai-gent/`. The opt-in screen says so.

## The adapter

`OpenCodeAdapter` (`rust/crates/ai-gent-core/src/delegate/opencode.rs`), on the same `Adapter` trait as Codex and the Claude profile.

- `detect`: `AI_GENT_OPENCODE_BIN`, else `opencode` on `PATH`, else `~/.opencode/bin/opencode`; then `--version`. `PATH` is never assumed and no auth file is read.
- `ready`: `opencode run --standalone --model <m> --format json --auto "Reply with exactly: READY"`, stdin closed, with the read-only permission set. Ready only when the events end in a `step_finish` with `reason: stop` and the text holds `READY` (the exit code alone is not trusted). Otherwise "opencode is not logged in"; a binary that cannot start is "not available". Nothing is worked around.
- `run`: `opencode run --standalone --model opencode/<model> --format json --auto "<brief>"`, stdin closed, cwd the worktree, started by the watcher like the other adapters. Default model `opencode/flint-40b-free`; `--model` takes `flint-40b-free` or a full `provider/model`.
- Permission set: passed inline in `OPENCODE_CONFIG_CONTENT` (`permissions` array, `{action, resource, effect}`); the user's own OpenCode configuration is never edited. **Every rule is a `deny`**, because `--auto` approves whatever is not denied (a rule that would ask is a rule that gets approved); a deny-only set also has no ordering to get wrong. Denied for every role: `git push|commit|add|reset|branch -D|...` (the supervisor stages and commits), `git commit --no-verify`, `git -C|-c|--git-dir|--work-tree`, network clients (`curl`, `wget`, `ssh`, `gh`, ...), database clients, container engines, `pass`/`gpg`, `webfetch` and `websearch`, reading or editing `~/.ssh`, `~/.gnupg`, `~/.password-store`, ai-gent's and OpenCode's own configuration, every home entry named like another agent's profile (names listed, contents never read), and files named like credentials. Reviewer and researcher roles also get `edit *` and `shell *` denied: read-only, no shell.
- Git and the network clients are denied in three forms each: the command first (`git push*`), by path (`*/git push*`, `/usr/bin/git push`) and after other words (`* git push*`, `env VAR=x git push`, `sudo git push`). Shell readers of credential-named files (`cat`, `less`, `more`, `head`, `tail`, `tac`, `cp`, `base64`, `xxd`, `strings`, `od`, `hexdump`, `grep`, `sed`, `awk`, `tar`, `zip`, in the same three forms) are denied when the line names `.env`, `credentials`, `id_rsa`, `id_ed25519`, `.pem`, `.p12`, `.key`, `.netrc`, `connections.` or `.npmrc`. **This is a request OpenCode matches, not a sandbox, and a pattern list cannot cover an interpreter (`python -c`, `node -e`, `perl`), a script or program the worker wrote, a shell redirect (`< .env`) or any command not on the list**; OpenCode's own read and edit rules on the same names are the other layer. Home entries named like another agent's profile are denied as the entry itself (`~/.claude.json` can be a file) as well as `<entry>/*` and `<entry>/**`, for read and edit.
- `result`: usage is **summed** over every `step_finish.part.tokens` (`input` plus cache read and write as the input, cache read as the cached part, `output`, `reasoning`); cost is summed too (`OpenCodeAdapter::cost`, 0 for free models). A run is judged by its final `step_finish`: exit code 1 with `reason: stop` is reported `done` (`reason: exit-code-1-but-finished`), exit 0 without a final stop is `failed` (`no-final-step-finish`). The watcher cannot read OpenCode's events (it wrote `done` for exit 0 and `failed` otherwise, with no usage), so when the verdict differs `result` **writes it back**: `status.json` (state and usage) and the track's delegation log line are corrected in place, and `status` runs the same judgement first, so all three agree once either has been called (before that, `status.json` and the log hold the watcher's version). The log line is rewritten, not appended; a line another run appended in the few milliseconds between the read and the rename could be lost. Whether the diff is right stays the supervisor's check (FR-011).
- Binary: a relative `AI_GENT_OPENCODE_BIN` with a directory part (`./bin/opencode`) is made absolute against the supervisor's working directory once, because the watcher starts the agent from the worktree; a bare name is looked up on `PATH`.
- Privacy: a free hosted model (name ends in `free`, case ignored, after dropping a `#variant` suffix; a `:tag` part ending in `free` counts too, the cautious reading, since the contract does not define `:tag`) is **refused** unless the caller passes `--privacy-ack`; the message says why. The adapter only enforces the gate; the supervising skill gives the notice (public content only, nothing from private repositories, credentials, customer data or `.ai-gent/`). `ready` sends fixed public text and needs no acknowledgement. `preflight` takes `--model` and `--privacy-ack` like `run` and its `privacy_acknowledged` check fails for a free model without the flag.
- `stop` is by PID file only.

### What it does not give, and the compensations

- **No OS sandbox.** The permission set and the worktree are the containment. They are patterns OpenCode matches, not a sandbox: a subprocess of an allowed build (`cargo build`, a build script, a proc macro an edit introduced) can call git, the network or read files, and a shell pattern can be sidestepped (`/usr/bin/curl` is covered by extra patterns, an interpreter running a script is not). This is weaker than Codex.
- Compensations, as in `adapter.md`, all outside the worker: the supervisor's FR-011 diff checks, verification run outside the worker, the worker never holding push credentials, and the cross-agent review of everything a free model produces (FR-010). The worker also **inherits the supervisor's environment** (the watcher does not clear it), so the supervisor must not run it with credentials in the environment.
- Progress: `status` counts events, but `status.json` tokens stay empty while the run goes, because the watcher only knows Codex's and Claude's usage events; once finished, `result` (or `status`) reads the usage from `events.jsonl` and writes it to `status.json` and the log line. `items_started` and `items_completed` stay 0.

### Real run 1 (2026-11-07, `opencode/flint-40b-free`, OpenCode v2.2.5, `--privacy-ack`)

A harmless docs task (write `specs/041-agent-delegation/quickstart.md` from the CLI's own help), launched with `ai-gent delegate run --agent opencode` in a git worktree, reviewed by Codex.

- **Worked:** the run started, 48 events (`step_start`, `text`, `tool_use`, `step_finish`), tool names `read`, `glob`, `shell`, `write`; exit code 0; `result --agent opencode` read the final message and summed the usage (465,401 input, 378,624 cached, 18,549 output tokens, cost 0), 309 s wall. `status` and `result` default to `--agent codex`: asked without `--agent opencode` they show no message and no usage.
- **Defect, isolation (found by this run):** the adapter launched OpenCode with the worktree as its working directory (`launch.json` records it), but OpenCode resolved its project root to the MAIN checkout: its `glob` returned paths under the main checkout and the file was written there, not in the worktree. Only that one new untracked file escaped (the supervisor moved it into the worktree); nothing tracked changed. Any run of this adapter from a worktree can therefore edit the main checkout, which defeats the worktree as the unit of containment. Until it is fixed, treat OpenCode in a worktree as unsafe for tasks that edit tracked files; this repository's allowlist (`codex`, `claude-profile`) does not include OpenCode. `opencode run` has no directory option (v2.2.5); the fix is below.
- **Quality:** the model's file was well formed and every flag it used exists, but a Codex review found six real errors in how the steps fit together (the brief output never saved, `--agent` not carried through, an event id used as a track id, no spec given to `review`, no verification or finding handling before `commit`, a blocking `watch`); the supervisor fixed all six. Cross-agent review of a free model's output (FR-010) was needed.

### Isolation fix (fixed in code, proven with a stub; a second REAL run has NOT been done)

- **Cause, as far as the code shows:** the adapter set the child's working directory to the worktree (`current_dir`), but `current_dir` does not change `PWD`, and the watcher passed the child the supervisor's whole environment, so `PWD` (and `INIT_CWD`, `OLDPWD`, any `GIT_*` location variable) still named the supervisor's directory, the main checkout. A Node or Bun tool that prefers `PWD` to the real working directory would resolve its project root there. This is the leading hypothesis, not a proven one: nothing was captured from the failed run, and OpenCode's own root discovery (for example from the git common directory) could not be read or ruled out without a model run. `opencode --help` and `opencode run --help` (v2.2.5) show no directory option for `run` (the top-level `[<directory>]` argument belongs to the interactive start, not to `run`), so there is nothing documented to pass.
- **Change:** the watcher now sets `PWD` to the worktree for every agent it starts, and the OpenCode adapter also removes `INIT_CWD`, `OLDPWD`, `GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR`, `GIT_INDEX_FILE` and `GIT_PREFIX` (`launch.json` `env_remove`). Whatever else OpenCode might do, a guard catches it: `launch.json` also records `guard_root` (the main checkout), the watcher fingerprints its `git status` (path, status code, size, mtime; untracked files one by one; `.ai-gent/` and the worktree left out) before and after the run, and any path that is new or changed makes the track `failed` with `reason` `wrote-outside-worktree: ...` (listing the paths, also in `escaped.txt` in the track directory) and `is_error`, whatever the exit code and final `step_finish` say. A main checkout that was already dirty is not blamed.
- **Proof, without a model:** `cargo test -p ai-gent --test delegate_opencode` against the stub (`examples/fake_opencode.rs`): in a linked worktree the stub's cwd and `PWD` are the worktree, none of the removed variables reach it, its file lands in the worktree and not in the main checkout; a stub told to write into the main checkout makes the track `failed` with the reason above.
- **Not proven:** that real OpenCode now stays in the worktree. A second real run needs the owner's approval; until it has been done, keep OpenCode off tasks that edit tracked files, and treat the guard as the safety net (it reports a write after the fact, it does not prevent it, and a write that leaves the main checkout's `git status` unchanged, such as an ignored file or one outside the repository, is not seen).

### Unverified

- Partly verified by one real run (see "Real run 1"): the `text` event carries `part.text` and `step_finish.part` carries `reason`, `tokens` and `cost`. Still assumed: the permission **action names** `shell`, `read`, `edit` come from ai-gent's own OpenCode rules (`scripts/opencode-config.py`), `webfetch` and `websearch` are assumed from the tool names and have not been confirmed to deny anything.
- Whether `*` in a resource crosses `/`, and whether a project-level OpenCode configuration can weaken the inline one (`OPENCODE_CONFIG_PROJECT_DISABLE` exists and is not set).
- Whether `input` already includes the cache counts (they are added to it here, as for Claude).
- Whether the ready probe's model call is sent with the same privacy terms as a task (unchecked, as above).

## Trial plan

1. A read-only task first (the T002 driver survey draft, researched by OpenCode with its own web access, reviewed by Claude), to read the JSON events, confirm the permission switch and record time and any usage.
2. Then one small build task inside `allowed_paths`, supervisor-committed, reviewed by Codex.
3. Compare with the Codex trials on the checklist; try more than one free model and record which is usable.
