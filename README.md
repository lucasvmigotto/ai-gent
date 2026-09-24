# ai-gent

Personal Claude Code skills and plugins, kept in one repo and symlinked into `~/.claude/skills/` so there's a single place to edit.

## Install / update

```sh
git clone git@github.com:lucasvmigotto/skills.git ~/codes/ai-gent
~/codes/ai-gent/setup.sh
```

Re-run `setup.sh` after adding, renaming or removing a skill or plugin — it's idempotent and prunes links whose source is gone. Edits to existing files need no re-run: the links point straight at this repo. Start a new Claude Code session to pick up a changed set of skills.

| Flag | Effect |
| --: | :-- |
| `--dry-run` | show what would change |
| `--force` | back up (`<name>.bak-<timestamp>`) and replace a real directory or foreign symlink in the way |
| `--uninstall` | remove every link that points into this repo |

`CLAUDE_SKILLS_DIR` overrides the target directory. `~/.claude/skills/synced/` is managed by claude.ai skill sync and is never touched.

## Layout

```txt
skills/<name>/SKILL.md                  personal skills        → /<name>
plugins/<name>/.claude-plugin/plugin.json
plugins/<name>/skills/<skill>/SKILL.md  namespaced plugins     → /<name>:<skill>
plugins/<name>/references/            files a plugin's skills read on demand
shared/                                  files several plugins share (symlinked into their references/)
```

A plugin directory placed in `~/.claude/skills/` loads as `<name>@skills-dir` — no marketplace or install step, and no plugin cache to refresh. Check one with `claude plugin details <name>@skills-dir`.


## Contents

- `skills/`
  - `edit-outside-workdir` — reading/editing files outside the working directory
  - `git-workflow` — Conventional Commits, branch-per-context, merge rules
- `plugins/devcontainer/`
  - `setup` — design devcontainer(s); separate API and client containers by default
  - `infra` — simulate databases, queues, storage, SMTP/mail, OAuth2/OIDC/LDAP and more
  - `proxy` — simulate the production reverse proxy
  - `workflow` — operate an existing devcontainer day to day
- `plugins/project/`
  - `init` — idea or references → product brief with the shared glossary
  - `spec` — brief → Spec Kit constitution, features, plans, tasks and contract skeleton
  - `docs` — static documentation site in `docs/site/`
- `plugins/frontend/`
  - `uiux` — layout and language vision
  - `spec` — design system and per-feature UI specs in Spec Kit format
  - `build` — implement the frontend phase by phase
- `plugins/backend/`
  - `domain` — domain model: contexts, aggregates, invariants, lifecycles
  - `spec` — canonical OpenAPI contract and per-feature backend specs
  - `build` — implement the backend, tests and contract first

## Product pipeline

The `project`, `frontend` and `backend` plugins form one chain. Each stage writes a file the next one reads, so stages can run in separate sessions or on their own. The contract for paths, ownership and shared rules is `shared/pipeline.md`.

```txt
project:init ─► project:spec ─┬─► frontend:uiux ─► frontend:spec ─► frontend:build
                              └─► backend:domain ─► backend:spec  ─► backend:build
                                                                        project:docs
```

Frontend and backend meet only at `contracts/openapi.yaml` and the brief's glossary. Specs use GitHub Spec Kit (`specify` CLI 1.x), bootstrapped by `project:spec`.

## Third-party content

`plugins/frontend/references/visual-direction.md` is adapted from Anthropic's `frontend-design` skill under the Apache License 2.0; see `plugins/frontend/references/LICENSE-frontend-design.txt`.
