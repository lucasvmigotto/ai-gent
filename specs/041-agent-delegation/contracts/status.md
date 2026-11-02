# Status file (per track)

`.ai-gent/delegate/<track>/status.json`, git-excluded, written atomically (write to a temp name, then rename) by the adapter for delegates and by the supervisor for Claude's own tracks.

| Field | Meaning |
|---|---|
| `track` | Track id (the branch name with `/` as `-`). |
| `agent` | `claude`, `claude-subagent` or `codex` (first version). |
| `role` | `builder`, `reviewer` or `researcher`. |
| `tasks` | Task ids from the spec (`["T001","T004"]`). |
| `state` | `queued`, `running`, `stalled`, `done`, `failed` or `taken-back`. |
| `started`, `updated` | Epoch seconds. |
| `events` | Event count so far (delegates that stream events). |
| `worktree` | Path relative to the repository root. |
| `verdict` | Supervisor result once checked: `pending`, `accepted`, `rejected`, with a short reason (a rule id from FR-011, never a diff or a brief). |
| `tokens` | Usage the agent reports, or null. |

Never in this file: the prompt or brief text, credentials, command output. The pane and the `status` operation read only this file and `.ai-gent/delegation.log`.
