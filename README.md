# ai-gent

Personal skills and plugins for Claude Code and [opencode](https://opencode.ai/v2/docs), kept in one repo and linked into place so there's a single place to edit.

## Install / update

```bash
curl -fsSL https://raw.githubusercontent.com/lucasvmigotto/ai-gent/HEAD/install.sh | sh
```

That clones the repo into `~/.local/share/ai-gent` and runs `setup.sh` for Claude Code and [opencode](https://opencode.ai/v2/docs). Run the same command again to update: it fast-forwards the clone (leaving it alone if it has local changes) and re-links.

Pass `setup.sh` flags after `-s --`, and choose where the clone lives with `AI_GENT_DIR`:

```bash
curl -fsSL https://raw.githubusercontent.com/lucasvmigotto/ai-gent/HEAD/install.sh | sh -s -- --target claude
curl -fsSL https://raw.githubusercontent.com/lucasvmigotto/ai-gent/HEAD/install.sh | AI_GENT_DIR=~/codes/ai-gent sh
```

| Variable | Default |
| --: | :-- |
| `AI_GENT_DIR` | `${XDG_DATA_HOME:-~/.local/share}/ai-gent` |
| `AI_GENT_REPO` | `https://github.com/lucasvmigotto/ai-gent.git` |
| `AI_GENT_BRANCH` | the repository's default branch |

Prefer to read it first? Download, inspect, then run:

```bash
curl -fsSLo install.sh https://raw.githubusercontent.com/lucasvmigotto/ai-gent/HEAD/install.sh
less install.sh && sh install.sh
```

Or clone it yourself:

```bash
git clone https://github.com/lucasvmigotto/ai-gent.git ~/codes/ai-gent
~/codes/ai-gent/setup.sh
```

Re-run `setup.sh` after adding, renaming or removing a skill or plugin, or after changing a plugin skill's description — it's idempotent and prunes whatever it installed whose source is gone. Edits to skill bodies need no re-run: everything points straight at this repo. Start a new session to pick up a changed set of skills.

| Flag | Effect |
| --: | :-- |
| `--target claude\|opencode\|all` | which tool to install for (default `all`; [opencode](https://opencode.ai/v2/docs) is skipped when not installed) |
| `--dry-run` | show what would change |
| `--force` | back up (`<name>.bak-<timestamp>`) and replace a real directory or foreign symlink in the way |
| `--uninstall` | remove everything the script installed for the target(s) |

`CLAUDE_SKILLS_DIR`, `OPENCODE_SKILLS_DIR` and `OPENCODE_COMMANDS_DIR` override the target directories. `~/.claude/skills/synced/` is managed by [claude.ai](https://support.claude.com/en/collections/14445694-claude-code) skill sync and is never modified.

### Claude Code

Each `skills/<name>/` and `plugins/<name>/` is symlinked into `~/.claude/skills/`. A plugin directory there loads as `<name>@skills-dir` — no marketplace or install step, and no plugin cache to refresh. Check one with `claude plugin details <name>@skills-dir`.

### opencode

[opencode](https://opencode.ai/v2/docs) has no plugin namespaces and requires a skill's name to match its directory, so `setup.sh`:

- links `skills/<name>/` and `~/.claude/skills/synced` into `~/.config/opencode/skills/` (which must be a real directory — a symlink to `~/.claude/skills` is replaced);
- generates `~/.config/opencode/skills/<plugin>-<skill>/SKILL.md` for each plugin skill — the real description plus an instruction to read and follow the source file in this repo — and a `/<plugin>-<skill>` command in `~/.config/opencode/commands/`.

[opencode](https://opencode.ai/v2/docs) also scans `~/.claude/skills` recursively, where plugin skills appear under short, colliding names (`spec`, `build`, …). Turn that off:

```bash
export OPENCODE_DISABLE_CLAUDE_CODE_SKILLS=1   # in your shell profile
```

Check with `opencode debug skill`.

## Layout

```txt
skills/<name>/SKILL.md                  personal skills        → /<name>
plugins/<name>/.claude-plugin/plugin.json
plugins/<name>/skills/<skill>/SKILL.md  namespaced plugins     → /<name>:<skill>
plugins/<name>/references/            files a plugin's skills read on demand
shared/                                  files several plugins share (symlinked into their references/)
```

## Contents

- `skills/`
  - `git-workflow` — Conventional Commits, branch-per-context, merge rules
- `plugins/devcontainer/`
  - `setup` — design devcontainer(s); separate API and client containers by default
  - `infra` — simulate databases, queues, storage, SMTP/mail, OAuth2/OIDC/LDAP and more
  - `proxy` — simulate the production reverse proxy
  - `workflow` — operate an existing devcontainer day to day
- `plugins/project/`
  - `init` — idea or references → product brief with the shared glossary
  - `architecture` — drivers and usage volumes → capacity model, topology, hosting, API style, data stores, ADRs
  - `spec` — brief + architecture → Spec Kit constitution, features, plans, tasks and contract skeleton
  - `docs` — static documentation site in `docs/site/`
- `plugins/frontend/`
  - `uiux` — layout and language vision
  - `spec` — design system and per-feature UI specs in Spec Kit format
  - `build` — implement the frontend phase by phase
- `plugins/backend/`
  - `domain` — domain model: contexts, aggregates, invariants, lifecycles
  - `spec` — canonical OpenAPI contract and per-feature backend specs
  - `build` — implement the backend, tests and contract first
- `plugins/qa/`
  - `strategy` — risk-based test strategy, per-feature traceability and gates
  - `e2e` — cross-stack Playwright journeys; sets features Verified
  - `load` — k6 load, stress, spike, soak and breakpoint tests from the capacity model
  - `review` — audit an existing test suite (mutation testing, flakiness, gaps)
- `plugins/devsecops/` — GitHub Actions by default; GitLab CI, Azure Pipelines, Bitbucket Pipelines, Jenkins, Forgejo/Gitea
  - `pipeline` — design and create CI/CD pipelines, gates, environments and promotion
  - `supply-chain` — pinning, updates, SCA/SAST/secret scanning, SBOM, signing, provenance
  - `audit` — security and reliability review of existing pipelines
  - `migrate` — move pipelines between platforms

## Product pipeline

The `project`, `frontend`, `backend`, `qa` and `devsecops` plugins form one chain. Each stage writes a file the next one reads, so stages can run in separate sessions or on their own. The contract for paths, ownership and shared rules is `shared/pipeline.md`.

```txt
project:init ─► project:architecture ─► project:spec ─┬─► frontend:uiux ─► frontend:spec ─► frontend:build ─┐
                                                      ├─► backend:domain ─► backend:spec ─► backend:build ──┼─► qa:e2e / qa:load ─► project:docs
                                                      └─► qa:strategy ──────────────────────────────────────┘
                       devsecops:pipeline / supply-chain from project:spec on; audit and migrate whenever needed
```

Every feature moves Planned → In progress → Implemented (build checkpoints passed) → Verified (QA passed), tracked in `specs/README.md`.

Frontend and backend meet only at `contracts/openapi.yaml` and the brief's glossary. Specs use GitHub Spec Kit (`specify` CLI 1.x), bootstrapped by `project:spec`.

## License

Copyright (C) 2026 lucasvmigotto. Licensed under the GNU General Public License v3.0 or later (`GPL-3.0-or-later`); see `LICENSE`.

Exception: `plugins/frontend/references/visual-direction.md` is adapted from Anthropic's `frontend-design` skill under the Apache License 2.0; see `plugins/frontend/references/LICENSE-frontend-design.txt`.
