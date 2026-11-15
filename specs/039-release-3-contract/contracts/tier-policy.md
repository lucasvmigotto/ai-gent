# Contract: support tiers (D2)

| Tier | Agents | Promise |
|---|---|---|
| 1 | Claude Code | Everything: skills, enforced guards, mods, update notice, evals, required CI |
| 2 | Codex | Skills and the binary's tools installed and tested in required CI; guards enforced if Codex can run hook commands (it can, T032; integration untested), else advisory |
| 3 | opencode, Cursor, Antigravity, GitHub Copilot, Amp, Kiro, Windsurf | Best-effort and frozen: skills install through generated stubs; no new tier-3-only features; regressions fixed by PR when reproducible; no SLA; never a release blocker; tests run in a non-required job |

## Feature by tier

"Yes" means shipped and covered by the stated CI. "Best-effort" means shipped, run in the non-required job, not a blocker. "N/A" means the agent has no such surface.

| Feature | Claude Code (1) | Codex (2) | Others (3) |
|---|---|---|---|
| Plugin skills | Yes: `/plugin:skill`, symlinks, required CI (`check.sh`, `claude plugin validate`) | Yes: generated `plugin-skill` stubs in `~/.agents/skills`, required install test | Best-effort: same stubs (`~/.agents/skills` for Cursor, Copilot, Amp; Antigravity, Kiro, Windsurf own dirs) |
| Personal skills | Yes | Yes | Best-effort |
| `ai-gent` binary tools called from skills | Yes (shell tool) | Yes (shell tool) | Best-effort: needs the agent to run commands |
| Guards (git, db, secrets, repo, containers) | Yes: hooks call `ai-gent guard`, deny/ask enforced | Enforced only if hook commands are supported (T032); otherwise instructions | opencode: plugin calls the binary (best-effort); Cursor, Antigravity, Copilot, Amp, Kiro, Windsurf: instructions only (as README today) |
| Mods (bands, panes, status line, toasts) | Yes (Claude Code mods; engine 2.1.312 pin) | N/A | N/A |
| Secret masking at prompt and result | Yes (hooks module) | No | No |
| Update notice at session start | Yes (SessionStart hook runs `ai-gent update --check`) | No: run `ai-gent update --check` by hand | No: same |
| `ai-gent update`, `rollback`, `doctor` | Yes | Yes | Yes (agent-independent CLI) |
| Model-role routing (`shared/agent-roles.md`) | Yes (subagents) | Yes (custom agents) | Best-effort (opencode commands as subtasks; others custom agents where they exist) |
| Evals (`claude plugin eval`) | Yes (not in CI) | N/A | N/A |
| Install paths and `--target` | Yes (Windows: `install.ps1` + `setup.ps1`) | Yes (Windows: same) | Yes; `--target all` still includes them; on Windows through Git Bash or WSL |
| Required CI | Linux `check`, macOS installer job, Windows binary and guard job, six-target build | Codex install case in `scripts/test-install.sh` (required) | install cases in a separate `tier3` job, `continue-on-error` |
| Docs-site compatibility matrix | green | green | "best-effort" |

## Tier-3 support policy

1. Frozen: no new capabilities are built for tier 3 alone; shared work (a skill, a guard rule, a binary subcommand) reaches them for free.
2. Fixes: a reproducible regression reported with the agent version is fixed on `dev` in the next release; no promised time.
3. CI: tier-3 install cases run in `tier3` (non-required). A red `tier3` job opens an issue, not a release block.
4. Agent API drift: if an agent changes its skills directory or format, the entry in `AGENT_IDS` is updated by PR; if the agent cannot be supported for two consecutive minor releases, the README says "unsupported" and the next major may drop it. Nothing is dropped in 3.0.0.
5. Honesty: the README and docs site label tier 3 as best-effort; claims of enforced guards are made only for tiers with a tested enforcement path.

## Codex hooks (T032)

Verified 2026-11-07 with codex-cli 0.166.0 (`codex features list`: `hooks stable true`, so not experimental): Codex runs synchronous `PreToolUse` command hooks, configured in `hooks.json` or `[[hooks.PreToolUse.hooks]]` (`type = "command"`), feature key `features.hooks`; `PermissionRequest` and `PostToolUse` also exist. A `PreToolUse` hook can deny a local tool call (`permissionDecision: "deny"` or exit code 2) or rewrite its arguments (`allow` plus `updatedInput`); it covers Bash/`exec_command`, `apply_patch`, MCP tools and other local functions, not hosted tools. Limits: non-managed hooks need a trust review, `ask` is unsupported, and an error or timeout lets the call proceed (fail-open). Sources: the installed CLI and the official Codex Hooks guide and configuration reference. Not yet tested: wiring `ai-gent guard` as a Codex hook. Whether tier 2 then means "enforced" is the owner's call.

## Codex hook wiring: evidence

Run 2026-11-07, codex-cli 0.166.0, `ai-gent` built from `origin/dev` (c9ee2df) with `cargo build --offline`. No model call, no network, no login; a throwaway `CODEX_HOME` in the session scratchpad (the real Codex home was never read or written). Evidence only; the tier-2 wording stays the owner's decision.

**Config used** (`$CODEX_HOME/config.toml`; key names from the installed binary's own config schema):

```toml
[features]
hooks = true

[[hooks.PreToolUse]]
matcher = "^(Bash|shell|local_shell|exec_command|shell_command)$"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "<abs path>/ai-gent guard --guard <name>"
timeout = 10
statusMessage = "ai-gent guard"
```

The subcommand takes `--guard git|db|secrets|repo|containers|all` (default `all`); a bare positional name is not the syntax, so a Codex entry uses `--guard`: one entry per guard, or the default `all` in one entry.

**What ran against real Codex 0.166.0**

1. `codex app-server --print-startup-config` accepted the config.
2. The app-server (`--listen stdio://`, started inside `unshare -rn`, so no network) answered JSON-RPC `hooks/list` with the hook parsed as `eventName: preToolUse`, `handlerType: command`, the exact command, matcher and `timeoutSec: 10`, `enabled: true`, `source: user`, no errors or warnings. Its `trustStatus` was `untrusted`: Codex keeps a trust hash per hook (`currentHash`) and a non-managed hook must be reviewed and trusted before it runs. How a hook becomes trusted was not exercised.
3. The hook contract was read from the JSON schemas embedded in the binary (`pre-tool-use.command.input` and `.output`), because the package ships no documentation.

**What did not run**: a real Codex turn firing the hook before a tool call. No Codex path executes a tool call without a model (`codex debug`, `codex sandbox` and `app-server command/exec` do not go through hooks), and a session needs a model and a login, which this run was barred from. The `tool_name` Codex gives its shell tool is therefore unverified (the schema leaves `tool_name` a free string and `tool_input` unconstrained); the matcher lists likely names and the test uses `Bash`.

**Codex PreToolUse input** (schema, `additionalProperties: false`): required `session_id`, `turn_id`, `transcript_path` (string or null), `cwd`, `hook_event_name: "PreToolUse"`, `model`, `permission_mode` (default, acceptEdits, plan, dontAsk, bypassPermissions), `tool_name`, `tool_input` (any), `tool_use_id`; optional `agent_id`, `agent_type`. This is Claude Code's shape plus `turn_id`; `ai-gent guard` reads `hook_event_name`, `tool_name`, `cwd` and `tool_input`, so the extra fields are ignored.

**Codex PreToolUse output** (schema): top-level `continue`, `decision` (`approve|block`), `reason`, `stopReason`, `suppressOutput`, `systemMessage`, and `hookSpecificOutput` with `hookEventName` (required), `permissionDecision` (`allow|deny|ask`), `permissionDecisionReason`, `updatedInput`, `additionalContext`. `ai-gent guard` prints `hookSpecificOutput.{hookEventName, permissionDecision, permissionDecisionReason}`, which validates. Also in the binary: exit code 2 with a stderr reason blocks; a hook that exits another non-zero code or prints invalid JSON is reported as a hook error; MCP, prompt and agent handlers are not supported.

**Fixtures through that wiring**: a scratchpad driver (`codex_wire.py`, not committed) builds Codex-shaped events, feeds them to the exact command line of the config, validates stdout against the Codex output schema and compares with `shared/guard-fixtures` (it reuses the sandbox repositories and command expansion of `scripts/guard-fixtures.py`, `sh` flavor).

| Check | Result |
|---|---|
| Baseline `scripts/guard-fixtures.py run --impl bin --shell sh` (Claude-shaped events) | 234 run, 0 failed |
| Codex-shaped events, five guards, shell-command cases | 216 run, 216 match (git 66, db 38, secrets 40, repo 34, containers 38) |
| Expected decisions in those 216 | deny 74, none 82, ask 60 |
| Output-schema errors, non-zero exits | 0, 0 |
| Not run | 18 cases whose tool is not a shell tool (file edits and similar), 32 cases with no `sh` form |

**Differences and gaps**

- No format difference in the event or the decision: the default (`claude`) format is accepted as is, so `--format codex` is not needed for shape.
- `ask` (60 of the 134 deciding fixtures): Codex's output schema allows `permissionDecision: "ask"`, but the T032 note above (from the Codex hooks guide) says `ask` is unsupported, and this run could not observe a live Codex handling it. If it is ignored or counted as a failed hook, every `ask` rule is unenforced on Codex. Decide this before any "enforced" claim.
- Trust: the hook is `untrusted` until reviewed in Codex; the install docs must say so, or the guard silently does not run.
- Fail-open: a crash or timeout of `ai-gent guard` lets the call proceed.
- Coverage: only shell calls are matched. `apply_patch` and MCP calls are other tool names; the guards that look at file paths would need their own matchers, and the corpus has no Codex `apply_patch` event to test them.

**Adapter shim**: not needed for event or decision shape. If the owner wants `ask` to be a hard stop on Codex, the smallest change is a `--format codex` flag on `ai-gent guard` that prints `deny` (reason prefixed "needs confirmation:") in place of `ask`, or `allow` plus a `systemMessage`. Described only, not built. A `setup.sh` helper that writes the `[[hooks.PreToolUse]]` block and explains the trust step is the other missing piece.

**Proven**: Codex 0.166.0 parses the wiring; the input and output contract embedded in its binary is satisfied by `ai-gent guard`'s default format; 216 of 216 shell-command fixtures give the expected decision and reason through Codex-shaped events. **Unproven**: a live Codex firing the hook, the shell tool's `tool_name`, how Codex treats `ask`, and the trust flow.
