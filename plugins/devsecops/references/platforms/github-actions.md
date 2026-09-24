# GitHub Actions

The default platform when none is detected or specified.

## Layout

- Workflows in `.github/workflows/*.yml`; one per concern (`ci.yml`,
  `release.yml`, `deploy.yml`, `security.yml`, `docs.yml`) rather than one
  giant file.
- Reuse: **reusable workflows** (`on: workflow_call`, called with
  `uses: ./.github/workflows/x.yml` or `org/repo/.github/workflows/x.yml@<sha>`)
  for whole jobs; **composite actions** (`.github/actions/<name>/action.yml`)
  for step sequences.
- Monorepo: `on.pull_request.paths` per workflow, or a single workflow with
  a changes-detection job and `if:` on downstream jobs. Remember required
  status checks and path-filtered workflows: a skipped workflow never
  reports, so gate on a job that always runs.

## Skeleton

```yaml
name: ci
on:
  pull_request:
  push:
    branches: [main]
permissions:
  contents: read            # default for every job; elevate per job
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}
jobs:
  test:
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@<sha> # vX.Y.Z
        with:
          persist-credentials: false
      # setup-<toolchain> with built-in cache, then lint/typecheck/test/build
```

## Secrets and identity

- Repository, organization and **environment** secrets; environment
  secrets are only exposed to jobs that declare `environment:` and pass
  its protection rules.
- **OIDC** to clouds: job-level `permissions: id-token: write` plus the
  cloud's login action (`aws-actions/configure-aws-credentials`,
  `azure/login`, `google-github-actions/auth`). Restrict the cloud-side
  trust to the repo **and** the environment or ref (`sub` claim, e.g.
  `repo:org/repo:environment:production`) — never the whole org.
- `GITHUB_TOKEN`: set the repo/org default to read-only; grant scopes per
  job (`contents: write` only for release jobs, `packages: write` only for
  publishing, `pull-requests: write` only for commenting bots).

## Environments and deployment

- Environments with **required reviewers**, **deployment branch/tag
  policies** (only `main` or release tags reach production) and optional
  wait timers.
- `concurrency:` per environment for deploy jobs (`cancel-in-progress:
  false` so a deploy is never killed midway).
- Deploy by image digest produced by the build job (job `outputs:`).

## Caching and artifacts

- Prefer `actions/setup-*` built-in `cache:`; otherwise `actions/cache`
  with a lockfile-hash key. Never cache secrets or build outputs that a
  PR from a fork could poison for `main` (caches are scoped by branch, but
  default-branch caches are readable by PR runs).
- `actions/upload-artifact`/`download-artifact` with `retention-days`.

## Supply chain features

- Dependabot (`.github/dependabot.yml`, include `package-ecosystem:
  github-actions`) or Renovate to keep SHA pins current.
- Code scanning with `github/codeql-action`; `actions/dependency-review-action`
  on PRs; secret scanning with push protection (repository setting).
- Artifact attestations (`actions/attest-build-provenance`, needs
  `id-token: write` + `attestations: write`) for SLSA build provenance;
  `gh attestation verify` at deploy.
- Rulesets / branch protection: required reviews, required status checks,
  signed commits if policy wants them, no force-push to `main`.

## Attack surface (check every workflow)

1. **Script injection** — `${{ github.event.pull_request.title }}`,
   `head_ref`, issue/comment bodies, commit messages interpolated into
   `run:`. Pass them through `env:` and quote `"$VAR"`.
2. **`pull_request_target` / `workflow_run` with untrusted checkout** —
   runs with secrets and a write token; checking out and building the PR
   head is a "pwn request". Use `pull_request` for building untrusted code;
   keep `pull_request_target` for labeling/commenting without checkout.
3. **Unpinned third-party actions** (`@v4`, `@main`) — pin by full commit
   SHA with a version comment.
4. **Over-broad `permissions`** — missing top-level `permissions:` means
   the repo default applies.
5. **`persist-credentials`** left true on checkout in jobs that run
   untrusted scripts.
6. **Self-hosted runners on public repositories** or shared between trust
   levels; non-ephemeral runners keep state between jobs.
7. **Artifact / cache poisoning** across trust boundaries
   (`workflow_run` consuming artifacts from a fork-triggered run).
8. **Secrets in logs** — `echo`ing secrets, `set -x`, debug logging; use
   `::add-mask::` for derived values.
9. **Dangerous triggers** — `issue_comment` commands without an author
   association check.

## Lint and verify

- `actionlint` (syntax, expressions, shellcheck of `run:`), `zizmor`
  (security audit), `poutine` / OpenSSF Scorecard for repo-level posture.
- `gh workflow run` / `gh run watch` to exercise a workflow on a branch
  (pushing a branch needs the user's go-ahead per `git:workflow`).
