# Contract: migration guide (draft text for the README and the docs-site Install page)

Audience: someone who already has ai-gent installed. Find your row.

| I have... | Do this | Check |
|---|---|---|
| 2.28 from the one-line installer | `ai-gent update` (confirm). Roll back with `ai-gent update --rollback`. | `ai-gent doctor` |
| 2.26 or 2.27 from the installer | Run `curl -fsSL https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.sh \| sh` (or update to 2.28 first with `scripts/ai-gent-update.sh`). | `ai-gent doctor` |
| a git clone from the dev mode | `mv ~/.local/share/ai-gent ~/.local/share/ai-gent.clone`, run the installer, then `~/.local/share/ai-gent/current/setup.sh --migrate`. The old clone stays; delete it when you are happy. | `ai-gent doctor` (`agent_links` ok) |
| `AI_GENT_MODE`, `AI_GENT_BRANCH`, `AI_GENT_REPO` or `AI_GENT_DIR` in my shell profile | Remove them; they are ignored (the installer stops if `AI_GENT_MODE=git`). | `ai-gent doctor` (`environment`) |
| I contribute to ai-gent | Work in a checkout; run `./setup.sh` from it (links point at the checkout); run `scripts/dev-bin.sh` for the binary. | guard tests pass |
| scripts of mine call `python3 .../wt.py` and friends | Use `ai-gent wt ...` etc. (table below, generated from 035-038 at the gate). `--json` output keeps its shape. | your script's own tests |
| Windows | Install with Git Bash or WSL (`install.sh`); `ai-gent.exe` runs natively; guards understand PowerShell and cmd. A PowerShell installer is a 3.x item. | `ai-gent doctor` |
| tier-3 agent (opencode, Cursor, Antigravity, Copilot, Amp, Kiro, Windsurf) | Nothing to do; re-run `setup.sh` through the installer. These agents are best-effort from 3.0.0. | skills listed |
| I pinned a version | Unchanged; `ai-gent update --to latest` unpins. | `ai-gent update --check --json` |
| CI or automation that fetches release assets | The URL scheme is the same; asset names are per target (`ai-gent-<ver>-<triple>.tar.gz`); read `SHA256SUMS`. | checksum passes |

## Old to new commands (filled at the gate from 035-038)

| Old | New |
|---|---|
| `python3 plugins/agents/scripts/wt.py ...` | `ai-gent wt ...` |
| `python3 plugins/agents/scripts/token_report.py ...` | `ai-gent tokens ...` |
| `python3 plugins/repo/scripts/reporun.py ...` | `ai-gent repo ...` |
| `python3 plugins/repo/scripts/specsync.py ...` | `ai-gent spec ...` |
| `python3 plugins/compact/scripts/compact.py ...` | `ai-gent compact ...` |
| `scripts/update-check.sh`, `scripts/ai-gent-update.sh` | `ai-gent update --check`, `ai-gent update` |
| `plugins/*/hooks/guard.sh` (read or patched) | `ai-gent guard --guard <id>` |
| `python3 plugins/db/scripts/dbrun.py ...` | unchanged (D19) |

(The subcommand names are from the decisions file's D9 list; 035-038 own the final flags.)

## What stays the same

Skills, invocations, plugin names, the pipeline, pass entries, the db plugin, the release flow.
