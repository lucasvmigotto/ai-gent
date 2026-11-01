# Test strategy — ai-gent

Status: Draft
Date: 2026-10-31
Owner stage: `qa:strategy` (format), written by hand for this repository. Nothing here was web-researched; every claim is read from the files under Sources, and anything not verifiable from the repository is labelled `[ASSUMPTION]`.

## Scope and honest shape

ai-gent is a toolkit: skills (Markdown), hooks modules (TypeScript, run by Claude Code's mod engine), POSIX `sh` guards, Python standard-library scripts, an installer, plus a static docs site (`docs/site/`, Bun, React, Vite). It has **no HTTP API and no database**, so contract tests (OpenAPI is N/A, see `specs/README.md`), database tests, load tests (`qa:load`) and cross-stack personas do not apply. There is no per-feature `qa.md` and no story-to-test traceability matrix: the specs under `specs/` cover only the docs site and the runners, and their QA column is empty. Risk is judged per kind of artifact below, not per feature.

Statuses follow `shared/pipeline-digest.md` (Planned, In progress, Implemented, Verified); only a QA or build stage moves a feature to Verified, and a red CI blocks it.

## Layers: what each guards

| Layer | What it guards | Where it runs |
|---|---|---|
| Static and metadata (`scripts/check.sh`) | Skill names equal directories; descriptions single-line, at most 400 characters, no `: ` or ` #`; plugin manifests (name, SemVer, description, license, author); `claude plugin validate` per plugin; backticked reference paths and `plugin:skill` references resolve; no broken symlinks; `bash -n`/`sh -n` and pinned shellcheck | local, PR, release, weekly |
| Prose consistency (`scripts/consistency.py`, tests in `scripts/test_consistency.py`) | Token tag vocabulary vs `shared/token-tags.json`, cards carry `Tags:` and `Tokens: projected`, copies of shared hook code equal their source, container-resource rules, the release script's JSON keys | local, PR, release, weekly |
| Guard tests (POSIX `sh`) | `test-guard.sh` (git), `test-db-guard.sh`, `test-secrets-guard.sh`, `test-repo-guard.sh`, `test-containers-guard.sh`: each case is an input the guard must allow, ask about or deny | same |
| OpenCode parity | `opencode/guard/test/rules.test.mjs` runs the OpenCode rules against the shell guards so both decide the same | same, if bun or node exists |
| Python unit tests (stdlib `unittest`) | `reporun`, `specsync`, `dbrun`, `wt`, `token_report`, project scripts, `compact:code` (with pinned Pygments) | same |
| Hooks modules (`claude plugin test`) | `*.test.ts(x)` of the 7 plugins whose `hooks.json` names a module (agents, db, devcontainer, git, project, repo, secrets): decisions, formatting, the mask output; run by the engine, not by a fake | same, needs `claude` on PATH |
| Release and installer | `test-release.sh`, `test-release-single.sh` (bump rules against throwaway repos), `test-install.sh` (all supported agents, in a throwaway `HOME`; also on macOS bash 3.2), `test-update-check.sh` | release scripts always; installer and update-check skipped by `--quick` |
| Docs site (`docs/site`) | Biome lint, `tsc` typecheck, Vitest (app, i18n completeness, pages, LLM output, metadata, version), build, base-path and Open Graph assertions, then an e2e smoke in containerized Chrome | `Docs CI`, path-filtered |
| Evals (`plugins/*/evals/`) | Trigger cases: a request reaches the right skill and not its neighbor. Behavior cases (tag `behavior`, with `scaffold.sh`): what the skill does once loaded, in a scaffolded directory | by hand only; **not in CI** |
| Real-engine tests | `scripts/test-dbrun-engines.py` (PostgreSQL, MySQL, MariaDB, SQL Server, Oracle containers, about 5.5 GB of images) and `scripts/test-reporun-github.py` (a throwaway private GitHub repository, needs `gh`) | engines: weekly in `scheduled.yml` and before pushing a `dbrun.py` change; reporun: by hand only |
| Interactive probes | Behaviors only a live Claude Code session shows | by hand |

There is no performance, visual-regression or mutation layer, and no coverage tool. `qa:review`'s mutation testing has not been run on this repository.

## `scripts/check.sh`

One entry point for developers and CI. `--quick` skips the installer and update-check tests; `--quiet` prints only failures and the final line (use it in agent loops); the full script runs once at the end. Environment pins: `DESC_BUDGET` (400), `SHELLCHECK_VERSION`, `PYGMENTS_VERSION`, `UV_VERSION`. When `claude`, shellcheck, uv or bun are missing the matching step warns and is skipped, which `--quiet` hides; so a local green run without `claude` on PATH proves less than CI (CI installs Claude Code at the version pinned in `.github/workflows/check.yml`, 2.1.312).

## Evals and cost policy

- Cases live in `plugins/<name>/evals/<case>/` (13 plugins have a suite; 20 cases carry a `scaffold.sh`). `scripts/run-evals.py [plugin...] [--behavior] [--runs N] [--max-cost-usd X]` runs `claude plugin eval` per plugin, prints a table and appends rows to `docs/product/evals.md`.
- Cost is real tokens: recorded rows run about 0.06 to 0.13 USD per case-run (for example 0.30 for the two git behavior cases, 1.11 for 13 project trigger cases). **Run only the touched plugin's suite**, `--runs 1` by default, and set `--max-cost-usd` when widening. Never run all suites to "be safe".
- Flaky by nature: single-sample routing misses happen (`docs/product/evals.md` notes `security` trigger cases and the `db` `behavior-raw-client` case, which passes about half the time because the agent sometimes reads `.env` first). Policy: a miss is re-run once; a case that fails twice is a finding about the skill or the case and is fixed or rewritten, never retried to green or silently deleted. A known-flaky case is recorded with a note in `evals.md`, as above.
- Trigger graders count only with the plugin loaded (`--ablation none`); behavior cases that need a shell also need `--scaffold --allow-tools Bash` and `bubblewrap` plus `socat`.

## Interactive probes

Headless `claude -p --plugin-dir` does not load hooks modules, so the rendering of a mod (bands, panes, toasts) is verified only interactively. Open probes, from `docs/product/recap-2026-10.md` and the specs, on Claude Code 2.1.312:

- a cross-plugin `$.state.get` for an absent plugin: what does it answer;
- a duplicate `/bands` command when several band plugins register it;
- the `session.append` origin shape (spec 026);
- the dashboard rendering of the pipeline pane (029); its compatibility table and manual runs are pending;
- a live `gh run list` for the CI watch band (023);
- whether `$.process.run` is visible to the guards;
- spec 024's R1 and R2 probes.

Until a probe is run its mod stays Implemented, not Verified.

## What Verified means here

| Kind | Exit criteria for Verified |
|---|---|
| Skill (SKILL.md and references) | `check.sh` green (metadata, references, description budget); trigger cases for the skill pass at `--runs 1` on the touched plugin; if the change altered behavior, its behavior case passes or a new one exists. A skill with no eval is Implemented |
| Hooks module (mod) | `claude plugin test` green; shell guard and parity tests green where it decorates a guard; the interactive probes for that mod run and recorded |
| Script (Python or `sh`) | Its unit or shell test passes in `check.sh`, including the changed branch; for `dbrun.py` also `test-dbrun-engines.py`, for `reporun.py` also `test-reporun-github.py` (CLAUDE.md "Before editing") |
| Installer | `test-install.sh` green, including macOS in CI |
| Docs site | Lint, typecheck, tests and build green; the base-path and OG assertions pass; e2e smoke passes in CI; the deploy succeeded (the live-site smoke in `scheduled.yml` is the later check) |
| Release tooling | `test-release.sh` and `test-release-single.sh` green and `scripts/release.py --dry-run` gives the expected plan |

Verified requires green CI on the merged commit (pipeline rule 8); it is never claimed from a local run alone.

## Gates per stage

| Stage | Gate |
|---|---|
| Before commit | `scripts/check.sh --quick --quiet`; a change in a script's area also runs that script's test directly |
| Pull request | `check.yml` (full `check.sh` plus the macOS installer job); `Docs CI` when `docs/site/**` or `docs/product/**` changed; Dependabot PRs go through the same gates |
| Before promotion `dev` to `main` | Full `scripts/check.sh` (installer and update-check included); evals of the touched plugins only (`run-evals.py <plugin>` and `--behavior` where relevant); the interactive probes for any changed or new mod; `test-dbrun-engines.py` or `test-reporun-github.py` only if `dbrun.py` or `reporun.py` changed; `scripts/release.py --dry-run` to read the predicted bumps |
| Push to `main` | `check` again, then `release.py`; `Docs CI` deploy after a successful release |
| Weekly | `scheduled.yml`: full `check`, `dbrun` against five real engines, live docs-site smoke; a failure opens one `CI failing: scheduled on main` issue |

Thresholds are binary: zero failures in `check.sh`; zero Biome errors; zero typecheck errors; i18n completeness test passing. No coverage or mutation threshold exists. There is no accepted flake rate for CI jobs: a flaky check is fixed, not retried. The docs-site e2e is labelled advisory in `docs-ci.yml` but fails its job when it fails.

## Test data and environments

- **No real homes, engines or tokens.** Tests build a fake `HOME` (`test-install.sh` exports a sandbox `HOME`; `test_token_report.py` sets `HOME` to a temp directory and never reads real transcripts or `~/.ai-gent`). `wt.py` container code runs against a fake `CONTAINER_ENGINE` script; never point it at a real engine.
- **Fixtures from fragments.** Credential shapes in `shared/secret-cases.json` are assembled from parts at test time (`"gh"` + `"p_"` and so on), so no full token shape is committed; the secret scanners and the mask mod share these cases, and their output must agree.
- **Throwaway repositories** for release and git tests (`test-release.sh` builds a base repository and tags it `1.0.0`).
- **Environments**: a developer machine and GitHub-hosted runners (ubuntu-24.04, macos-15). Real databases and the GitHub repository exist only inside the real-engine tests, as containers or a private repository deleted afterwards (that needs `gh auth refresh -s delete_repo`; otherwise the script names the repository to delete). Browser tests use `selenium/standalone-chrome:153.0` pinned by digest, never a host driver. There is no staging.
- No personal data is used anywhere; there are no personas because there are no roles.

## Known gaps

Facts from the repository, not a plan:

- The trigger evals for the 20 descriptions that were trimmed to the 400-character budget have not been run since the trim.
- The behavior evals added in the research and critique work (for example `behavior-pipeline-research-pins`, `behavior-init-critique-recorded`, `behavior-load-research-limits`, `behavior-uiux-research-sources`) have no recorded run in `docs/product/evals.md`, whose last rows are 2026-10-25 for secrets, project, devsecops, repo and agents.
- Evals are not in CI, so a description change can reach `main` without a routing check.
- No coverage tool, no mutation testing, no CI flake tracking.
- Mods (specs 018 to 031) are covered by unit tests and `claude plugin test` only; none has been verified in a live session (`recap-2026-10.md`). The pipeline pane's close action and the guard asks' text cannot be tested with the kit.
- Docs-site gaps from `specs/README.md`: the e2e smoke covers only stories US1 and US4; the install, contribute, reference and changelog pages and the Markdown twins have no tests; the skip link under hash routing is unverified.
- `test-reporun-github.py` has no scheduled run; `reporun` is verified only when someone runs it.
- Interactive installer selection is untested.
- A local `check.sh` without `claude`, uv or pipx silently skips parts; only CI is complete.

## Sources

- `scripts/check.sh`: flags and environment 9-24; skill metadata 58-98; `claude plugin validate` and `test` 126-145; references and symlinks 147-167; shell lint 169-204; guard tests 206-248; Python unit tests 250-289; `compact:code` 291-316; consistency 318-330; release tests 332-348; OpenCode parity 350-363; installer and update-check 365-383.
- `.github/workflows/check.yml`: Claude Code pin 28, check 35, macOS job 39-55. `.github/workflows/scheduled.yml`: cron 8, engines 22-35, live smoke 39-71. `.github/workflows/docs-ci.yml`: verify 35-115, e2e 117-182.
- `scripts/run-evals.py`: options 66-71, cost accounting 90-102. `docs/product/evals.md`: history table and flake notes. `plugins/*/evals/*/` (13 plugins; 20 `scaffold.sh`).
- `CLAUDE.md`: "Before editing", "Hook scripts are POSIX sh", "Evals". `docs/product/recap-2026-10.md`: "Status: not verified interactively", "Known limitations", "Promotion checklist".
- `shared/secret-cases.json` (comment and `vars`); `plugins/agents/scripts/test_token_report.py:70`; `plugins/agents/scripts/test_wt.py:117-123`; `scripts/test-install.sh:15`; `scripts/test-release.sh:29-48`.
- `specs/README.md`: feature notes 001 to 005, intro paragraph on N/A sides. `shared/pipeline-digest.md`: statuses and rule 8.
