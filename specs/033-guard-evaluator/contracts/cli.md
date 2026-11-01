# CLI contract: `ai-gent guard`

Extends `specs/032-rust-workspace-and-release/contracts/cli.md` (global flags, envelope, exit codes).

```
ai-gent guard --guard <git|db|secrets|repo|containers|all> [--shell sh|powershell|cmd]
              [--format claude|opencode] [--shadow --shell-decision <deny|ask|none>
              [--shell-reason <text>]] [--json]
ai-gent guard shadow-report [--since <duration>] [--guard <id>] [--json]
ai-gent guard replay --transcripts <dir> [--guard <id>] [--oracle <hooks dir>] [--json]
ai-gent guard list-reasons [--json]        # used by scripts/consistency.py
```

## `guard` (evaluate)

- Reads one event from stdin. `--format claude` (default): prints the hook JSON line, or nothing for `none`; **always exits 0**, even on internal failure (the failure policy turns it into an `ask` line). `--format opencode`: stdin is `{"action":"shell|read|edit","resources":["..."],"cwd":"..."}`, stdout is `{"decision":"deny|ask|none","reason":"..."}`, exit 0.
- `--shadow`: evaluates, compares to `--shell-decision` (and `--shell-reason` when given), appends a shadow record, prints nothing, exits 0. It never writes to stdout in this mode so a hook's output is untouched.
- Internal budget 2 s; commands over 256 KiB answer `ask`.

## `guard shadow-report`

Reads the shadow log; text summary: evaluations, agreements, disagreements per guard. `--json`: `{"schema":1,"ok":true,"data":{"since":"...","guards":{"git":{"evaluations":0,"agree":0,"disagree":0,"expected_divergence":0}},"disagreements":[<records>]}}`. Exit 0 when there is no unexplained disagreement, 1 when there is one (so a script can gate the promotion on it).

## `guard replay`

Reads Claude Code transcript JSONL under `--transcripts` (default `~/.claude/projects`), extracts Bash tool inputs, evaluates each, and prints counts per guard and decision. With `--oracle <dir holding the plugins' hooks>` it also runs the shell guards and lists the sha256 prefix of each command the two decide differently. Never prints a command. Exit 1 when any differ.

## `guard list-reasons`

Prints every reason entry (id, guard, marker, decision, text). `consistency.py` checks each has a `guard-ux.ts` entry.

## Failure output (claude format)

A binary-internal failure that can still print: the `ask` line with reason `<marker> guard failed internally (<code>); allow only if you accept this command unguarded.` The launcher (`guard.sh`, FR-011) prints the missing, crashed and timeout variants because the binary may not exist.
