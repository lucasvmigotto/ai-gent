Reconstructed by project:introspec on 2026-10-18 from 384c771

# Contract — installer CLI

## install.sh

```
curl -fsSL https://raw.githubusercontent.com/lucasvmigotto/ai-gent/HEAD/install.sh | sh [-s -- <setup.sh args>]
```

| Variable | Default | Meaning |
|---|---|---|
| `AI_GENT_DIR` | `${XDG_DATA_HOME:-$HOME/.local/share}/ai-gent` | clone location |
| `AI_GENT_REPO` | `https://github.com/lucasvmigotto/ai-gent.git` | what to clone |
| `AI_GENT_BRANCH` | default branch | branch or release tag (a tag pins, detached) |

Exit: 0 on success; 1 via `die` (missing git/bash, non-clone directory, no `setup.sh`) [OBSERVED: install.sh:7-23,66-78].

## setup.sh

```
setup.sh [--target <ids|all>] [--skip-prompt] [--dry-run] [--force] [--uninstall]
         [--yes|-y] [--no-config-edits] [--verbose|-v] [-h|--help]
```

| Exit | When |
|---|---|
| 0 | done, dry run, help |
| 1 | no agent chosen in the selector |
| 2 | unknown option or invalid `--target` |

[OBSERVED: setup.sh:86-146,806-808]. Directory overrides: `CLAUDE_SKILLS_DIR`, `OPENCODE_SKILLS_DIR`, `OPENCODE_COMMANDS_DIR`, `AGENTS_SKILLS_DIR`, `KIRO_SKILLS_DIR`, `WINDSURF_SKILLS_DIR`, `OPENCODE_CONFIG`; `NO_COLOR=1`; `AI_GENT_OPENCODE_MODEL_SMALL` [OBSERVED: setup.sh:48-53,108-121,492].

Output lines are `[status] name` per item with `--verbose`, else a summary per status; statuses seen: ok, link, relink, generate, backup, skip, unlink, remove [OBSERVED: setup.sh:228-286; scripts/test-install.sh:81-86].
