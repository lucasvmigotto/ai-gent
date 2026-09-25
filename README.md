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
| `AI_GENT_BRANCH` | the repository's default branch; a release tag (`1.1.0`) pins that release |

A pinned clone stays on its tag until you pass another one (`AI_GENT_BRANCH=1.2.0`, or the default branch's name to follow it again). Releases are listed in `CHANGELOG.md`.

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

Each `skills/<name>/` and `plugins/<name>/` is symlinked into `~/.claude/skills/`. A plugin directory there loads as `<name>@skills-dir` — no marketplace or install step, and no plugin cache to refresh. Check one, including its token cost, with `claude plugin details <name>@skills-dir`.

Every enabled plugin's skill descriptions are loaded into every session. Turn off the ones a project doesn't need, for that project only:

```bash
claude plugin disable devsecops@skills-dir --scope project   # writes .claude/settings.json
claude plugin disable qa@skills-dir --scope local            # .claude/settings.local.json, not committed
```

The `git` plugin also installs a `PreToolUse` hook (`plugins/git/hooks/guard.sh`) that enforces `git:workflow`: it blocks `--no-verify`, `Co-Authored-By` trailers and `push --force` without a lease, and asks you to confirm pushes, commits or merges on `main`/`master`, `branch -D`, `reset --hard`, `clean -f`, `commit --amend`, history rewrites and `gh pr create`.

### opencode

[opencode](https://opencode.ai/v2/docs) has no plugin namespaces and requires a skill's name to match its directory, so `setup.sh`:

- links `skills/<name>/` and `~/.claude/skills/synced` into `~/.config/opencode/skills/` (which must be a real directory — a symlink to `~/.claude/skills` is replaced);
- generates `~/.config/opencode/skills/<plugin>-<skill>/SKILL.md` for each plugin skill — the real description plus an instruction to read and follow the source file in this repo — and a `/<plugin>-<skill>` command in `~/.config/opencode/commands/`.

[opencode](https://opencode.ai/v2/docs) also scans `~/.claude/skills` recursively, where plugin skills appear under short, colliding names (`spec`, `build`, …). Turn that off:

```bash
export OPENCODE_DISABLE_CLAUDE_CODE_SKILLS=1   # in your shell profile
```

Check with `opencode debug skill`.

opencode doesn't run Claude Code hooks, so the `git` guard isn't active there. Its own permission rules get close — for example, in `~/.config/opencode/opencode.json`:

```json
{
  "permission": {
    "bash": {
      "git push*": "ask",
      "git commit*--no-verify*": "deny",
      "git branch -D*": "ask",
      "git reset --hard*": "ask",
      "gh pr create*": "ask"
    }
  }
}
```

## Layout

```txt
skills/<name>/SKILL.md                          personal skills (none yet)  → /<name>
plugins/<name>/.claude-plugin/plugin.json
plugins/<name>/skills/<skill>/SKILL.md          namespaced plugins          → /<name>:<skill>
plugins/<name>/skills/<skill>/references/       files one skill reads on demand
plugins/<name>/references/                      files a plugin's skills share
plugins/<name>/hooks/hooks.json                 plugin hooks (git)
shared/                                         files several plugins share (symlinked into their references/)
scripts/check.sh                                repository checks, also run by CI
```

## Contents

- `plugins/git/`
  - `workflow` — Conventional Commits, branch-per-context, merge and cleanup rules, issue linking; enforced by a guard hook
- `plugins/devcontainer/` — container-first development (rules in `shared/containers.md`)
  - `setup` — per-module multi-stage Containerfiles with a thin `tools` layer that agents and CI run, task recipes, resource limits, Podman or Docker; plus human devcontainers, API and client apart by default
  - `infra` — simulate databases, queues, storage, SMTP/mail, OAuth2/OIDC/LDAP and more
  - `proxy` — simulate the production reverse proxy
  - `workflow` — run tasks through the tools recipes, or operate the devcontainer day to day
- `plugins/project/`
  - `init` — idea or references → product brief with the shared glossary
  - `architecture` — drivers and usage volumes → capacity model, topology, hosting, API style, data stores, ADRs
  - `spec` — brief + architecture → Spec Kit constitution, features, plans, tasks and contract skeleton
  - `docs` — static documentation site in `docs/site/`
  - `status` — where the pipeline stands (artifacts, feature statuses, gaps, contradictions) and the next stage to run
  - `introspec` — reverse-engineer an existing codebase into the full pipeline spec, every claim Observed, Inferred or Assumed
  - `retrofit` — upgrade in place with behavior frozen: `patch` (CVEs), `minor` (adapt code), `major` (breaking upgrades)
  - `refactor` — redesign keeping the core invariants; business changes recorded for approval; incremental migration
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
- `plugins/db/` — safe database access for any engine (PostgreSQL, MySQL/MariaDB, SQL Server, Oracle, SQLite), all through the `dbrun` runner
  - `connect` — list, test and classify connection profiles, never showing credentials
  - `inspect` — full discovery of a database: schema, constraints, indexes, routines, links, statistics, ERD
  - `review` — how the project uses its database: mappings vs. schema, migrations, indexes, queries, pooling, rights, backups
  - `investigate` — a scenario and a symptom → hypotheses, evidence, root cause (hand edit vs. application bug), repair
- `plugins/devsecops/` — GitHub Actions by default; GitLab CI, Azure Pipelines, Bitbucket Pipelines, Jenkins, Forgejo/Gitea
  - `pipeline` — design and create CI/CD pipelines, gates, environments and promotion
  - `supply-chain` — pinning, updates, SCA/SAST/secret scanning, SBOM, signing, provenance
  - `iac` — infrastructure as code (OpenTofu/Terraform by default) for the environments the architecture chose
  - `audit` — security and reliability review of existing pipelines
  - `migrate` — move pipelines between platforms

## Databases

Every `db:*` skill reaches a database only through `plugins/db/scripts/dbrun.py`, which enforces `plugins/db/references/safety.md`:

- **Remote** (production, staging/homologação, shared development — anything not proven local): `SELECT` only, with timeouts, row caps and cost guards. Changes are written as scripts for a person to run; `dbrun` never executes a remote write.
- **Local**: only a database container of this repository's compose project on this machine, reached through its own network namespace. Writes need a recorded plan, your confirmation and a backup, and can be restored.
- Results are masked by default (names, emails, CPF/CNPJ, phones, cards, secrets), revealed only when you ask; every statement is logged before it runs, outside the repository.

Connection profiles live in `~/.config/ai-gent/db/connections.env` (mode 600; format in `plugins/db/references/connections.md`); a running database container of the current project is discovered automatically as `local:<service>`. The `db` plugin's guard hook denies direct client calls (`psql`, `mysql`, `sqlcmd`, `sqlplus`, `sqlite3`, `mongosh`, …) that carry SQL that writes, and asks before any other direct use. In opencode, add the equivalent `permission.bash` rules (for example `"psql*": "ask"`, `"sqlcmd*": "ask"`).

## Product pipeline

The `project`, `frontend`, `backend`, `qa` and `devsecops` plugins form one chain. Each stage writes a file the next one reads, so stages can run in separate sessions or on their own. The contract for paths, ownership and shared rules is `shared/pipeline.md`.

```txt
project:init ─► project:architecture ─► project:spec ─┬─► frontend:uiux ─► frontend:spec ─► frontend:build ─┐
                                                      ├─► backend:domain ─► backend:spec ─► backend:build ──┼─► qa:e2e / qa:load ─► project:docs
                                                      └─► qa:strategy ──────────────────────────────────────┘
                       devsecops:pipeline / supply-chain from project:spec on; iac once hosting is decided; audit and migrate whenever needed
                       project:status at any point: where things stand, what to run next

existing codebase: project:introspec ─► project:retrofit (same behavior, upgraded)
                                     └► project:refactor (redesign) ─► the chain above, in review mode
```

Every feature moves Planned → In progress → Implemented (build checkpoints passed) → Verified (QA passed), tracked in `specs/README.md`.

Frontend and backend meet only at `contracts/openapi.yaml` and the brief's glossary. Specs use GitHub Spec Kit (`specify` CLI 1.x), bootstrapped by `project:spec`.

## Development

Run `scripts/check.sh` before committing (CI runs it on every pull request and push to `main`). It checks skill names and description budgets (400 characters — every session loads them), plugin manifests, `plugin:skill` references and relative paths, symlinks and shell scripts, then tests the git and db guards, `dbrun` (its statement classifier, masking and SQLite end-to-end paths), the release script and the installers in a throwaway `HOME`.

Releases are automatic: every push to `main` runs the checks, then `scripts/release.py` decides from the Conventional Commits since the last tag — `feat` → minor, `fix`/`perf`/`refactor` → patch, `!` or `BREAKING CHANGE` → major, anything else → no release. A release bumps the changed plugins' versions, turns `CHANGELOG.md`'s `## Unreleased` into the release entry (or generates one), commits `chore(release): X.Y.Z`, tags `X.Y.Z` and publishes a GitHub Release. Preview it with `python3 scripts/release.py --dry-run`, and `git pull --ff-only` after a release. `--quick` skips the installer tests; set `SHELLCHECK='uvx --from shellcheck-py shellcheck'` if shellcheck isn't installed. Conventions for editing skills are in `AGENTS.md`.

## License

Copyright (C) 2026 lucasvmigotto. Licensed under the GNU General Public License v3.0 or later (`GPL-3.0-or-later`); see `LICENSE`.

Exception: `plugins/frontend/references/visual-direction.md` is adapted from Anthropic's `frontend-design` skill under the Apache License 2.0; see `plugins/frontend/references/LICENSE-frontend-design.txt`.
