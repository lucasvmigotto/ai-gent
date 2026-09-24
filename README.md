# ai-gent

Personal Claude Code skills and plugins, kept in one repo and symlinked
into `~/.claude/skills/` so there's a single place to edit.

## Install / update

```sh
git clone git@github.com:lucasvmigotto/skills.git ~/codes/ai-gent
~/codes/ai-gent/setup.sh
```

Re-run `setup.sh` after adding, renaming or removing a skill or plugin —
it's idempotent and prunes links whose source is gone. Edits to existing
files need no re-run: the links point straight at this repo.
Start a new Claude Code session to pick up a changed set of skills.

| Flag | Effect |
|---|---|
| `--dry-run` | show what would change |
| `--force` | back up (`<name>.bak-<timestamp>`) and replace a real directory or foreign symlink in the way |
| `--uninstall` | remove every link that points into this repo |

`CLAUDE_SKILLS_DIR` overrides the target directory. `~/.claude/skills/synced/`
is managed by claude.ai skill sync and is never touched.

## Layout

```
skills/<name>/SKILL.md                  personal skills        → /<name>
plugins/<name>/.claude-plugin/plugin.json
plugins/<name>/skills/<skill>/SKILL.md  namespaced plugins     → /<name>:<skill>
```

A plugin directory placed in `~/.claude/skills/` loads as
`<name>@skills-dir` — no marketplace or install step, and no plugin cache
to refresh. Check one with `claude plugin details <name>@skills-dir`.

## Contents

- `skills/`
  - `edit-outside-workdir` — reading/editing files outside the working directory
  - `frontend-design` — visual design direction for UI work
  - `git-workflow` — Conventional Commits, branch-per-context, merge rules
  - `project-docs` — static documentation site workflow
- `plugins/devcontainer/`
  - `setup` — design devcontainer(s); separate API and client containers by default
  - `infra` — simulate databases, queues, storage, SMTP/mail, OAuth2/OIDC/LDAP and more
  - `proxy` — simulate the production reverse proxy
  - `workflow` — operate an existing devcontainer day to day
