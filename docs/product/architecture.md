Reconstructed by project:introspec on 2026-10-18 from 384c771

# Architecture (as-is) — ai-gent docs site and runners

Status: Draft

Scope: the documentation site (`docs/site/`) and the scripts that run things — `install.sh`, `setup.sh`, `plugins/db/scripts/dbrun.py`, `plugins/repo/scripts/reporun.py`, `plugins/repo/scripts/specsync.py`, `plugins/agents/scripts/wt.py`, `shared/passstore.py`. The skills' Markdown is out of scope. As-is only: no target design (that is `project:architecture` or `project:refactor`).

## Topology

```
                         GitHub (lucasvmigotto/ai-gent)
                          │  push main ─► release.yml ─► tag X.Y.Z
                          │                   │ workflow_run
   developer machine      │                   ▼
 ┌──────────────────────┐ │        Docs CI deploy ─► Cloudflare R2 bucket
 │ curl install.sh | sh │◄┘ clone            (prefix /ai-gent/) ─► docs.lucasvmigotto.me/ai-gent/
 │   └ setup.sh ─► ~/.claude/skills, ~/.agents/skills, …                ▲
 │                                                                      │ HTTPS, static
 │ agent session ─► dbrun ──► uv driver child ─► PostgreSQL/MySQL/…     readers, agents
 │               │        └► client container in a local DB's netns       (llms.txt)
 │               ├► reporun ─► gh/glab/az/jira CLI or REST ─► GitHub, GitLab,
 │               │                                     Azure DevOps, Bitbucket, Jira
 │               ├► specsync (files only) ─► reporun batch
 │               └► wt.py ─► git worktree
 │ pass + gpg ◄── passstore (dbrun, reporun)
 └──────────────────────┘
```

[OBSERVED: .github/workflows/release.yml, .github/workflows/docs-ci.yml:18-22,172-263; install.sh:41-79; plugins/db/scripts/dbrun.py:1081-1127; plugins/repo/scripts/reporun.py:52-58,312-347].

## Hosting

| Part | Where it runs | Evidence |
|---|---|---|
| Docs site | static files in a shared Cloudflare R2 bucket under `/ai-gent/`, served at `https://docs.lucasvmigotto.me/ai-gent/`; GitHub environment `ai-gent-docs` | `.github/workflows/docs-ci.yml:172-193` |
| Installer, runners | the developer's machine (Linux assumed), run by the user or by a coding agent | `install.sh:4-5`; [ASSUMPTION: Linux only] |
| CI | GitHub Actions `ubuntu-24.04` | `.github/workflows/*.yml` |

## Stack

| Part | Language and runtime | Key libraries | Evidence |
|---|---|---|---|
| Docs site | TypeScript 7.0.2, Bun 1.x, Node scripts | React 19.3.0, react-router 8.4.0 (HashRouter), Vite 8.3.2, Tailwind 4.3.3, react-markdown 10.1.0, lucide-react 1.50.0; Biome 2.5.15, Vitest 5.0.3, jest-axe 11, selenium-webdriver 4.50.0, sharp 0.35.5 | `docs/site/package.json` |
| Runners | Python 3.9+ standard library only | — (drivers fetched at run time by `uv`: psycopg[binary], PyMySQL, pymssql, oracledb — unpinned) | `dbrun.py:9-13,36-44`; `reporun.py:18`; `specsync.py:19`; `wt.py:14` |
| Installer | POSIX sh + bash ≥ 4 | git, optional fzf, python3 for opencode config | `install.sh:1,38-39`; `setup.sh:1,71` |

Full dependency inventory: `docs/product/sbom.cdx.json` (337 npm components from `docs/site/bun.lock`).

## Data stores

No server-side database. State is files on the user's machine:

| Store | Owner | Evidence |
|---|---|---|
| `${XDG_CONFIG_HOME}/ai-gent/db/connections.{toml,env}` | dbrun (read) | `dbrun.py:64-67,125,151-161` |
| `${XDG_STATE_HOME}/ai-gent/db/{plans,backups,log}/` | dbrun | `dbrun.py:69-73,856-860,1297-1300,1385-1388` |
| `${XDG_STATE_HOME}/ai-gent/{targets,opencode-config.json,opencode-config-created}` | setup.sh | `setup.sh:56-57,493-494` |
| pass store (`PASSWORD_STORE_DIR`) | the user; runners read, `dbrun secrets migrate` adds | `shared/passstore.py:28-29` |
| `specs/NNN-*/tracker.md` | specsync | `specsync.py:212-227` |
| `.worktrees/`, `.git/info/exclude` | wt.py | `wt.py:26,61-68` |
| browser `localStorage` (theme, locale) | docs site | `docs/site/src/main.tsx:6`; `docs/site/src/i18n.tsx:403` |

## Integrations

| System | Used by | Selected by (names only) | Label |
|---|---|---|---|
| Cloudflare R2 (S3 API) | Docs CI deploy | vars `CLOUDFLARE_R2_ACCOUNT_ID`, `CLOUDFLARE_R2_ENDPOINT_S3_CLIENT`, `CLOUDFLARE_R2_BUCKET_ID`; secret `CLOUDFLARE_R2_ACCOUNT_SECRET` | [OBSERVED: docs-ci.yml:186-193] |
| Docker Hub `selenium/standalone-chrome` (digest-pinned) | Docs CI e2e | — | [OBSERVED: docs-ci.yml:135] |
| GitHub / GitLab / Azure DevOps / Bitbucket / Jira APIs and CLIs | reporun | remote URL, `git config ai-gent.platform`, `ai-gent.jira.url`, pass entries `<prefix>/<platform>/…` | [OBSERVED: reporun.py:52-58,163-220] |
| PostgreSQL, MySQL, MariaDB, SQL Server, Oracle, SQLite | dbrun | profiles; `AI_GENT_DB_CONNECTIONS` | [OBSERVED: dbrun.py:34-44] |
| podman / docker | dbrun (local proof, client containers, backups) | `DOCKER_HOST`, `CONTAINER_HOST`, `AI_GENT_DB_CLIENT_IMAGE` | [OBSERVED: dbrun.py:44,94-104,454-468] |
| `ghcr.io/astral-sh/uv:python3.12-bookworm-slim` | dbrun client container (tag, no digest) | `AI_GENT_DB_CLIENT_IMAGE` | [OBSERVED: dbrun.py:44] |
| pass + gpg | passstore | `PASSWORD_STORE_DIR`, `AI_GENT_PASS_PREFIX` | [OBSERVED: passstore.py:24-29] |
| GitHub (clone) | install.sh | `AI_GENT_REPO`, `AI_GENT_BRANCH`, `AI_GENT_DIR` | [OBSERVED: install.sh:41-43] |

## Qualities found

- **Safety by refusal**: unknown = write (dbrun, reporun), unknown = production (dbrun), writes need an explicit flag the guard hooks turn into a user question [OBSERVED: dbrun.py:660-661; reporun.py:278-281].
- **Secrets**: values only on a child's stdin or environment; output redacted [OBSERVED: dbrun.py:9-13,838-854; reporun.py:140-145,330-347].
- **Timeouts**: dbrun statement/lock timeouts; pass reads 90 s; reporun HTTP 30 s [OBSERVED: dbrun.py:45; passstore.py:43; reporun.py:466-468].
- **Caching**: site assets immutable for a year, entry and LLM files `no-cache` [OBSERVED: docs-ci.yml:231-263].
- **Accessibility**: axe tests on three pages, skip link, live regions, `aria-pressed`, `aria-expanded` [OBSERVED: docs/site/tests/pages.test.tsx; docs/site/src/components.tsx].
- **i18n**: `LOCALES` + typed `Messages`, en-US only [OBSERVED: docs/site/src/i18n.tsx:9].
- **Missing**: no bundle-size budget, no tests of the LLM output, docs-ci actions pinned by tag only, no lifetime on dbrun session containers.
- **Capacity**: no metrics exist; static site with modest read traffic per the brief — no capacity model.

## Decisions the history explains

The brief's decision log (`docs/product/brief.md`) records: site under `docs/site/`, all skill pages generated, English only with the i18n architecture kept (reversing pt-BR parity), R2 deploy. No ADRs exist (`docs/product/adr/` absent); none are reconstructed.
