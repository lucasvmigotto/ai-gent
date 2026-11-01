# Delivery — ai-gent

Status: Draft
Date: 2026-10-31
Owner stage: `devsecops:pipeline` (format), written by hand for this repository from its own workflows. Nothing here was web-researched; everything is read from the files cited under Sources. What could not be read from the repository was labelled `[ASSUMPTION]` and has since been confirmed by the maintainer (2026-10-31), marked `[CONFIRMED 2026-10-31]`.

## Scope

ai-gent ships two things, and this document covers how both reach users:

- **The toolkit**: skills and plugins (`plugins/<name>/`), hooks modules in TypeScript, POSIX `sh` guards and Python standard-library scripts, installed from this repository by `install.sh` and `setup.sh`. A release is a git tag and a GitHub Release with notes; it ships **no files** (no archive, binary or container image), so there is nothing to checksum, sign or promote.
- **The docs site** (`docs/site/`, Bun, React, Vite), built and uploaded to Cloudflare R2 under the prefix `/ai-gent/`.

There is **no HTTP API, no database and no running service**. The OpenAPI contract is N/A (`specs/README.md` says so too), and so are staging and production environments, deploy strategy (rolling, blue/green, canary), database migrations and OIDC cloud trust.

## Workflows

| Workflow | File | Triggers | What it does |
|---|---|---|---|
| check | `.github/workflows/check.yml` | `pull_request` (any base), `workflow_dispatch`, `workflow_call` | Job `check` on ubuntu-24.04 installs Claude Code 2.1.312 and runs `scripts/check.sh` (full, installer tests included); job `macos` on macos-15 runs `scripts/test-install.sh` under the system bash 3.2 |
| release | `.github/workflows/release.yml` | push to `main`, `workflow_dispatch` | Calls `check`, then `release`: plan, bump, tag, GitHub Release, back-merge into `dev`; a failure opens an issue |
| Docs CI | `.github/workflows/docs-ci.yml` | push to `main` and `dev` and pull requests, filtered by path (`docs/site/**`, `docs/product/**`, and for pushes `CHANGELOG.md` and the workflow itself); `workflow_run` of `release` on `main` | `verify` (lint, typecheck, tests, build, assertions, SBOM artifact), `e2e` (containerized Chrome), `deploy` (R2, only after a successful release) |
| scheduled | `.github/workflows/scheduled.yml` | cron `17 5 * * 1` (Mondays 05:17 UTC), `workflow_dispatch` | `check` again, `engines` (`scripts/test-dbrun-engines.py` against real PostgreSQL, MySQL, MariaDB, SQL Server and Oracle), `site` (e2e smoke against the live site) |
| report-failure | `.github/workflows/report-failure.yml` | `workflow_call` only | Opens one `CI failing: <workflow> on <branch>` issue (label `ci-failure`) or comments on the open one |
| Dependabot | `.github/dependabot.yml` | monthly | Update PRs for `github-actions` (prefix `ci`) and `bun` in `/docs/site` (grouped, prefix `chore`) |

Every workflow sets top-level `permissions: contents: read`; `release` raises `contents: write` on its `release` job only, and the failure reporter gets `issues: write`. Every job has a `timeout-minutes` (5 to 60). `check` and `Docs CI` use `concurrency` with cancel-in-progress; `release` and `scheduled` queue instead of cancelling.

The `check` workflow has no path filter: any pull request, including one that only touches docs, runs the whole suite. `scripts/check.sh` is the same entry point developers run locally (`--quick` skips the installer and update-check tests, `--quiet` prints only failures).

## How a change flows

```
work branch (off dev) ──PR──▶ dev ──promotion PR (merge commit)──▶ main ──push──▶ release.yml
        check (+ Docs CI by path)         check (+ Docs CI)                 check ▶ release.py ▶ tag ▶ Release
                                                                            then Docs CI deploy (workflow_run)
```

1. **Work branch to `dev`.** Work happens on a branch cut from `dev` (`git:workflow`); the PR targets `dev`. `check` runs on the PR, and `Docs CI` too when it touches `docs/site/**` or `docs/product/**`. Merge style follows `git:workflow`: four or more commits merge with a merge commit, fewer fast-forward.
2. **Promotion `dev` to `main`.** A pull request from `dev` into `main`, always merged with a merge commit (`git:workflow`, "Merge style"). Promotion needs an explicit request from the user. The chain in `git:workflow` includes a `staging` level; this repository has none, so the chain here is `work branch → dev → main`.
3. **Push to `main`.** `release.yml` runs `check` unless the head commit starts with `chore(release):`, then the `release` job (below).
4. **Docs deploy.** When `release` completes successfully on `main`, `Docs CI` is triggered by `workflow_run` and its `deploy` job publishes the site, so the footer shows the new tag.
5. **After the release.** The bot's `chore(release)` commit lands on `main` only; the job tries to merge it back into `dev`, and CLAUDE.md asks local clones to `git pull --ff-only` before the next commit.

**Required checks.** The workflow files do not say which checks branch protection requires; that is a repository setting, not a file. `[CONFIRMED 2026-10-31]` by the maintainer: `check` is required on `dev` and `main`. The comments in `release.yml` do state that `dev` has a ruleset requiring pull requests that the bot cannot bypass, and that `main` must let `github-actions[bot]` push (README repeats this).

## Release

`release.yml` job `release` (needs `check`, runs only on `refs/heads/main`):

1. **Plan**: `python3 scripts/release.py --dry-run`. Exit 0 sets `release=true`; exit 3 means nothing releasable, the job stops green; any other code fails the job.
2. **Apply**: `scripts/release.py` bumps and prints the version; `--notes` renders the GitHub Release body; the job commits `CHANGELOG.md`, each changed `plugins/*/.claude-plugin/plugin.json` and `docs/site/package.json` as `chore(release): X.Y.Z`, creates the annotated tag `X.Y.Z`, then `git push --atomic origin HEAD:refs/heads/main refs/tags/X.Y.Z` (rejected, nothing published, if `main` moved during the run) and `gh release create ... --verify-tag`.
3. **Back-merge** of the tag into `dev` with `--no-ff`. It is refused on purpose by `dev`'s ruleset (no deploy key, no PAT); the job then only warns with the command the owner runs.

Rules (`scripts/release.py`, `shared/releasing.md`, CLAUDE.md "Releases"):

- Conventional Commits since the last tag decide the bump: `feat` minor; `fix`, `perf`, `refactor` patch; `type!:` or a `BREAKING CHANGE:` footer major; `docs`, `chore`, `ci`, `test`, `style`, `build`, `revert` no release. Merge-commit subjects and `chore(release):` commits are ignored.
- Tags are plain SemVer without `v` (`2.22.0`), annotated, one per release.
- Each plugin is bumped only if its files, **or files it links** (symlinked `shared/` references), changed since the tag; a plugin already bumped by hand is left alone. `docs/site/package.json` is bumped alongside, listed in `release.json`. Consequence seen in practice: a change to a shared file bumps every plugin that links it (`docs/product/recap-2026-10.md` notes `db` and `devsecops` bumping beyond the six expected).
- A hand-written `## Unreleased` section in `CHANGELOG.md` is promoted as written; without one, a section is generated from commit subjects (intermediate subjects included).
- Nobody bumps versions or tags by hand. `scripts/release.py --dry-run` previews; `--dry-run --json` is machine-readable.
- No loop: the bot pushes with `GITHUB_TOKEN`, whose pushes trigger no workflow, and `check` is skipped for `chore(release):` commits.
- `concurrency: release`, `cancel-in-progress: false`: runs queue.

## Docs site delivery

- **`verify`** (`Docs CI`): `bun install --frozen-lockfile` (Bun 1.4.2), SBOM, Biome lint, typecheck, Vitest unit tests, build with `VITE_BASE_PATH=/<repo>/`, then three assertions: the built `index.html` references `/<repo>/assets/`, `og-image.png` exists and `og:image` and `twitter:image` are advertised, and every `aws s3 sync|cp` line in the workflow mentions `R2_PREFIX` (a guard that a deploy can never target the bucket root). The `dist/` is uploaded as an artifact for 7 days.
- **`e2e`** (needs `verify`): builds the site, starts `selenium/standalone-chrome:153.0` pinned by digest with `--network host`, serves `vite preview` on 127.0.0.1:4173, runs `bun run e2e` (`docs/site/tests/e2e/smoke.mjs`), removes the container in an `always()` step. The workflow comment calls it advisory because deploy runs after the release and does not depend on it; but as a job in the pull-request run it still shows red.
- **`deploy`** (only `workflow_run` of `release` with conclusion success): environment `ai-gent-docs` (url `https://docs.lucasvmigotto.me/ai-gent/`), checks credentials and R2 configuration loudly (an empty deploy must not look green), then in this order: sync hashed assets without deleting, copy `index.html`, favicon and OG image, copy `llms*.txt`, sync the Markdown pages with `--delete`, and last prune old assets. All targets are `s3://$R2_BUCKET${R2_PREFIX}...` with `R2_PREFIX=/<repository name>/`; **never the bucket root**, because sibling docs share the bucket. Hashed assets get `immutable` caching, entry files `no-cache`.
- A failed deploy opens a failure issue; a failed `verify` or `e2e` fails the PR or push that caused it.
- Weekly, `scheduled.yml` `site` runs the same smoke against the live URL.

## Secrets and variables (names only)

| Name | Kind | Used by | Where configured |
|---|---|---|---|
| `CLOUDFLARE_R2_ACCOUNT_SECRET` | secret | `deploy` (as `AWS_SECRET_ACCESS_KEY`) | `[CONFIRMED 2026-10-31]` as stated by the maintainer: environment `ai-gent-docs` or repository secrets (the file does not say which) |
| `CLOUDFLARE_R2_ACCOUNT_ID` | variable | `deploy` (as `AWS_ACCESS_KEY_ID`) | repository or environment variable, same caveat |
| `CLOUDFLARE_R2_ENDPOINT_S3_CLIENT` | variable | `deploy` (`R2_ENDPOINT`, must start with `https://`) | same |
| `CLOUDFLARE_R2_BUCKET_ID` | variable | `deploy` (`R2_BUCKET`) | same |
| `GITHUB_TOKEN` (`github.token`) | automatic | release push and Release, failure issues | GitHub |

No other secret appears in the workflow files. Local runners use `pass` (`shared/secrets.md`), never CI secrets. The R2 access key id is stored as a variable, not a secret, in the workflow's own wiring.

## Supply chain

Observed:

- **Actions pinned by commit SHA** with a version comment in all four workflows (`actions/checkout`, `oven-sh/setup-bun`, `anchore/sbom-action`, `actions/upload-artifact`). `specs/README.md` row 005 still says "actions pinned by tag only"; that note is stale.
- **Browser image pinned by digest** (`selenium/standalone-chrome:153.0@sha256:...`).
- **Tool versions pinned**: Claude Code 2.1.312, Bun 1.4.2, uv 0.12.20 for the engine job, shellcheck-py and Pygments through `scripts/check.sh` defaults (`0.11.0.1`, `2.21.0`), Bun dependencies by `bun.lock` with `--frozen-lockfile`.
- **Dependabot** monthly for actions and the Bun dependencies. Python and shell have no third-party dependencies to update (stdlib only).
- **SBOM**: two different things exist. (a) `Docs CI` `verify` generates a CycloneDX JSON SBOM of `docs/site` with `anchore/sbom-action` and keeps it as a workflow artifact; it is not attached to the GitHub Release. (b) `docs/product/sbom.cdx.json` is committed, was reconstructed on 2026-10-18 by `project:introspec` from `bun.lock` (its own metadata says syft and cdxgen were not installed) and records `ai-gent-docs` version 1.1.0 at commit 384c771; `docs/site/package.json` is now 1.6.0, so the committed file is stale and nothing regenerates it.
- `persist-credentials: true` only in the `release` job (it pushes); `check` and `scheduled` checkouts set it false, the `Docs CI` checkouts do not set it.

Not present: dependency or vulnerability scanning (SCA), SAST, secret scanning in CI, signing, SLSA provenance, a license policy. See Gaps.

## Rollback

- **Toolkit**: git is the only mechanism. Revert the offending change in a PR to `dev` and promote; the next release publishes the revert (`revert` alone does not trigger a release: `fix` or `refactor` does, so a revert is committed as `fix: revert ...` when a release is wanted). `[CONFIRMED 2026-10-31]` by the maintainer: tags and GitHub Releases are not protected; the repository does not configure tag protection or immutable releases, and by project rule are never moved or deleted by hand. Users pin or roll back with `install.sh` pinning (`spec 006`, "clone/update/pin").
- **Docs site**: no versioned deploy. Each deploy overwrites the prefix; to roll back, revert on `main` and let the next release trigger the deploy, or re-run the `Docs CI` deploy from a prior run of the `release` workflow. Previous hashed assets are pruned at the end of every deploy, so there is no old copy in R2.
- A release that fails mid-way is safe by construction: the atomic push publishes the commit and tag together or not at all; `gh release create` runs after the push, so a failure there leaves a tag without a Release, which is completed by hand.

## Environments, deployment strategy, infrastructure

N/A: no application runs anywhere, so there are no dev, staging or production environments, approvals, canaries or database changes. The only protected environment is `ai-gent-docs` for the R2 upload. There is no `infra/` directory and no infrastructure as code; the R2 bucket and the `docs.lucasvmigotto.me` hub are managed outside this repository.

## Gaps

Stated as facts from the repository, not as a plan:

- No automated release on `dev`: nothing runs on a push to `dev` except `Docs CI` (path-filtered). Only the PR `check` guards `dev`.
- No staging branch; the `git:workflow` staging level is unused.
- Evals (`claude plugin eval`) are not in any workflow; interactive probes of hooks modules cannot run in CI. `scripts/test-reporun-github.py` is not in any workflow. `scripts/test-dbrun-engines.py` runs weekly only (`scheduled.yml`), not on pull requests.
- No workflow linter (`actionlint`, `zizmor`) runs in CI; no dependency or secret scanning; no signing or provenance; no release checksums (nothing to checksum).
- The committed `docs/product/sbom.cdx.json` is stale and unmanaged; the CI SBOM is a 90-day-default workflow artifact (retention not set in the file).
- `Docs CI` `verify` and `e2e` do not run on a push to `main` that touches nothing under the path filters, and `deploy` depends on `release` succeeding: a release failure leaves the site on its previous version.
- Required checks, tag protection and the `ai-gent-docs` environment's protection rules are repository settings with no file in the repository, so they are unverified here.
- The back-merge into `dev` is refused by design and left to the owner on every release.
- Pipeline duration is not recorded; the job timeouts (10 minutes for `check` and `release`, 20 for the docs jobs, 60 for engines) are the only bounds in the files.
- Monitoring, alerting and SLOs: none; there is no service. The signals are CI status, failure issues and the weekly live-site smoke.

## Sources

- `.github/workflows/check.yml`: triggers 4-7; permissions 9-10; Claude Code version 28; `scripts/check.sh` 35; macOS job 39-55; SHA pin 21.
- `.github/workflows/release.yml`: trigger 17-20; concurrency 25-27; `chore(release)` guard 31; `release` job 34-40; plan 48; release step 61-76; atomic push 75; back-merge 82-90; failure report 92-97.
- `.github/workflows/docs-ci.yml`: triggers 3-21; concurrency 30-32; verify 35-115 (SBOM 52-58, assertions 79-107); e2e 117-182 (image digest 146-147); deploy 184-287 (environment 193-195, secrets 197-205, ordered syncs 244-287); failure report 291-296.
- `.github/workflows/scheduled.yml`: cron 8; engines 22-35; live-site smoke 39-71; reporter 73-79.
- `.github/workflows/report-failure.yml`: 1-29. `.github/dependabot.yml`: 1-17.
- `scripts/release.py`: bump table 62; breaking footer 64; plugin planning 216; changelog promotion 265; exit codes 35-47; no-release message 322. `release.json`: 1. `shared/releasing.md`: flow 24-32, bump rules 34-59, back-merge and loop 60-80, sub-package variant.
- `plugins/git/skills/workflow/SKILL.md`: merge style 90-94, permissions table 34, chain 69.
- `CLAUDE.md`: checks and real-engine tests ("Before editing"), "Releases", "Evals". Its statement that the real-engine tests are "not part of CI" is true of `test-reporun-github.py`; `test-dbrun-engines.py` also runs weekly in `scheduled.yml`.
- `docs/product/sbom.cdx.json`: metadata (timestamp 2026-10-18, tool description, component version 1.1.0); `docs/site/package.json` version 1.6.0, `e2e` script line 16.
- `specs/README.md`: features 005 (docs-site-delivery) and 008 notes.
- `docs/product/recap-2026-10.md`: promotion checklist and release prediction.
