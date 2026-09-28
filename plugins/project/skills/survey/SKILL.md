---
name: survey
description: Survey a project on first contact — its stack and versions, layout and entry points, how it builds, tests and runs, CI and containers, history and conventions, main branches and how far they drift apart, stale or unmerged branches, and red flags — ending in which stage to run next. Read-only. Use for "get to know this project", "survey this repo", "cold start", or before project:introspec.
---

# Survey — first contact with a project

A quick, broad, read-only pass over a repository nobody has explained yet:
enough to talk about it accurately and to choose what to do next. It
answers what a new senior engineer asks on day one. Depth belongs
elsewhere: `project:introspec` for the full reverse-engineered spec,
`project:retrofit` for upgrades and CVEs, `project:recap` when returning
to a known project.

Cheap first: read manifests and git metadata before source files, sample
rather than read everything, and stop at what the report needs. Nothing
is built, installed, run or fetched without asking — except `git fetch`,
which only updates remote-tracking refs (skip it offline and say so).
Read code per `../../references/reading-code.md` — `compact:code` for
large or many files in a brace language.

## 1. Stack

- **Languages** by size: `git ls-files | sed 's/.*\.//' | sort | uniq -c
  | sort -rn | head`, ignoring lockfiles, vendored and generated code.
- **Manifests and lockfiles** (`package.json`, `pom.xml`, `build.gradle*`,
  `*.csproj`, `pyproject.toml`, `go.mod`, `Cargo.toml`, `composer.json`,
  `Gemfile`, …): frameworks and their versions, runtime versions
  (`.nvmrc`, `.tool-versions`, `.python-version`, `global.json`, Maven
  toolchains, `engines`), the package manager.
- **Data and integrations:** database drivers and ORMs, migration tools,
  brokers, caches, external APIs — from dependencies and config
  (`application*.yml`, `appsettings*.json`, `.env.example`, compose files).
  Name what the evidence shows; mark guesses as such. Never open `.env`
  files with real values — note that they exist.

## 2. Layout and how to work with it

- Top-level structure: modules or services, entry points (`main`,
  `Program.cs`, `app.py`, `server.ts`, …), frontend vs. backend, tests.
- **Build, test, run:** the commands, from `Makefile`, `justfile`,
  `package.json` scripts, the README, the CI file — as the project
  documents them. Don't run them unless the user asks.
- **Environment:** `.devcontainer/`, `Containerfile`/`Dockerfile`, compose
  files, `compose.tools.yml`.
- **CI/CD:** `.github/workflows/`, `.gitlab-ci.yml`, `azure-pipelines.yml`,
  `Jenkinsfile`, … — what runs on pull requests, on main branches and on
  release.
- **Docs:** README, `docs/`, ADRs, `AGENTS.md`/`CLAUDE.md`, and whether
  they match what you found.

## 3. History and branches

```bash
git fetch --all --tags
python3 ../../scripts/repo_state.py --since "90 days ago"   # path relative to this file
git log --format='%cs %an %s' -n 30
git shortlog -sn --since="1 year ago" | head
git log --since="1 year ago" --name-only --format= | sort | uniq -c | sort -rn | head -15
```

- **Age and activity:** first commit, commit count, the pace lately.
- **Conventions:** commit style (Conventional Commits or not), merge vs.
  squash vs. rebase, how releases are tagged.
- **Branching model** from the main branches `repo_state.py` found:
  trunk-based, GitHub flow, gitflow (`develop`), environment branches
  (`homolog`, `staging`).
- **Drift:** each main branch against the base and against its remote.
  For example, `main` 14 commits behind `homolog` means promoted work
  hasn't reached production yet; local behind remote means this clone is
  stale. Say what each gap means for someone about to work.
- **Branches:** recent branches no main branch contains (work in flight),
  and stale ones.
- **Ownership:** how many people commit, the most-changed files
  (hotspots), and code only one person touches (a knowledge risk). Report
  counts, not a ranking of people.

## 4. Red flags

Only what the survey came across, each with its evidence:

- committed secrets or `.env` files with real values (report the path, never the value);
- runtimes or frameworks at or past end of life, lockfiles missing,
  dependencies years behind — flagged here, triaged by `project:retrofit`;
- no tests, or tests CI doesn't run; CI missing or failing;
- docs that contradict the code; drift between main branches with no
  release in sight.

## Output

A report in the chat:

```
Survey — <repo>
Stack        languages · frameworks and versions · data stores and integrations
Layout       modules, entry points · build/test/run commands · environment · CI
History      age, activity, conventions, branching model, contributors (counts)
Branches     main branches with drift · work in flight · stale
Red flags    each with its evidence
Next         the stage to run and why — alternatives
```

Write it to `docs/product/survey.md` only when the user asks. **Next:**
`project:introspec` for an existing product that needs its specification;
`project:retrofit` when upgrades or CVEs dominate; `project:status` when
pipeline artifacts already exist; `project:init` for an almost empty
repository; `devcontainer:setup` when there's no way to run it locally.
