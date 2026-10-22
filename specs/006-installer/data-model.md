Reconstructed by project:introspec on 2026-10-18 from 384c771

# Data model — installer state

| Entity | Where | Shape | Evidence |
|---|---|---|---|
| Agent | `setup.sh` registry | id ∈ {claude, opencode, codex, cursor, antigravity, copilot, amp, kiro, windsurf}; label; detection probes (commands or `$HOME` dirs); skills directory | `setup.sh:48-84` |
| Link | agent skills dir | symlink → `skills/<n>/` or `plugins/<n>/` in the repo; owned when it points into the repo | `setup.sh:305-341` |
| Generated entry | agent skills dir | `<name>/SKILL.md` with `name`, quoted `description`, `metadata.source` → the source SKILL.md, plus a `.ai-gent-generated` marker file | `setup.sh:389-473` |
| opencode command | `OPENCODE_COMMANDS_DIR/<p>-<s>.md` | description, optional `subtask: true` (+ model) | `setup.sh:440-470` |
| Targets state | `$XDG_STATE_HOME/ai-gent/targets` | space-separated agent ids, one line | `setup.sh:817-832` |
| Config record | `$XDG_STATE_HOME/ai-gent/opencode-config.json`, `opencode-config-created` | what was added to `opencode.json`, so uninstall removes only that | `setup.sh:493-494` |

Ownership rule: setup removes only links into the repo and directories carrying the marker [OBSERVED: setup.sh:343-352,474-488].
