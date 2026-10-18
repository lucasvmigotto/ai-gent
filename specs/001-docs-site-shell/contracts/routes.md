Reconstructed by project:introspec on 2026-10-18 from 384c771

# Contract — site routes and build inputs

No HTTP API: the site is static. Its contract is the route table, the build environment and the files it emits.

## Routes (hash-routed, under the base path)

| Route | Page | Feature | Evidence |
|---|---|---|---|
| `#/` | LandingPage | 002 | `docs/site/src/App.tsx:21` |
| `#/use/skills` | SkillsPage | 002 | `App.tsx:23` |
| `#/use/skills/:id` | SkillPage (`:id` = `<plugin>-<skill>`) | 002 | `App.tsx:24` |
| `#/use/install` | InstallPage | 003 | `App.tsx:22` |
| `#/use/flow` | FlowPage | 003 | `App.tsx:25` |
| `#/contribute` | ContributePage | 003 | `App.tsx:26` |
| `#/reference` | ReferencePage | 003 | `App.tsx:27` |
| `#/changelog` | ChangelogPage | 003 | `App.tsx:28` |
| `#/*` (anything else) | NotFoundPage | 002 | `App.tsx:29` |

## Build environment

| Variable | Default | Used for | Evidence |
|---|---|---|---|
| `VITE_BASE_PATH` | `/ai-gent/` | Vite `base`, favicon path | `docs/site/vite.config.ts:7`, `docs/site/.env.example` |
| `VITE_SITE_URL` | `https://docs.lucasvmigotto.me/ai-gent` | canonical, Open Graph, `llms.txt` links | `vite.config.ts:8-10`, `scripts/generate-llms.ts:11-12` |

## Commands (`docs/site/package.json`)

`gen`, `dev`, `build` (gen → vite build → generate-llms), `preview`, `lint` (Biome), `typecheck`, `test`, `e2e`, `og`.
