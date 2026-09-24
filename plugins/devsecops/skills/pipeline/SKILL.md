---
name: pipeline
description: Design and create CI/CD pipelines for a project — pull-request, main, release and scheduled pipelines with fast feedback, quality and security gates, monorepo path filters (separate frontend and backend pipelines), caching, build-once-promote-by-digest, environments with approvals, OIDC cloud credentials and deployment strategies — rendered for GitHub Actions (default), GitLab CI, Azure Pipelines, Bitbucket Pipelines, Jenkins or Forgejo/Gitea Actions. Reads the architecture, test strategy, specs and devcontainers, and writes the pipeline files plus docs/product/delivery.md. Use for "set up CI/CD", "create the GitHub Actions workflows", "add a deploy pipeline", "pipeline for GitLab/Azure DevOps/Bitbucket/Jenkins", or /devsecops:pipeline.
---

# CI/CD pipeline design

## Before starting

1. Read `../../references/principles.md` (role, platform detection and
   default, principles), `../../references/platforms/concepts.md`, and the
   detected platform's file in `../../references/platforms/`.
2. Read `../../references/pipeline.md` and the inputs that exist:
   - `docs/product/architecture.md` / ADRs — environments, hosting,
     deploy targets, deployment strategy, regions;
   - `docs/product/test-strategy.md` and `specs/*/qa.md` — which suites
     run at which stage and the gates (`qa:strategy`);
   - the constitution and each `plan.md` — stack, versions, structure;
   - `.devcontainer/` — toolchain versions and base images to mirror;
   - existing CI files — **update** them; never silently replace a
     pipeline someone relies on.
3. Ask what's missing (in one round): platform if undetectable and the
   user may not want the GitHub Actions default, deploy targets and
   environments, who approves production, the cloud account/subscription
   structure for OIDC, release/versioning scheme.

## Design — the portable model first

Write the model (see `concepts.md`) in `docs/product/delivery.md` before
rendering any YAML:

1. **Pipelines**
   - **PR / MR** — per changed area (frontend, backend, docs, infra):
     lint · format check · typecheck · unit/component · contract tests ·
     build · quick security (secret scan, dependency review / SCA on the
     diff, SAST fast rules) · preview environment if the architecture has
     one. Target under ~10 minutes.
   - **Main** — everything in PR plus integration (Testcontainers with
     the `devcontainer:infra` images), e2e (`qa:e2e`), package, SBOM,
     sign, provenance, deploy to staging, smoke test, then promotion to
     production behind approval.
   - **Release** — tags/versions, changelog, publishing, production
     deploy of the already-built digest.
   - **Scheduled** — full SAST/SCA/container scans, DAST baseline against
     staging, load tests (`qa:load`), dependency update PRs, cache warmup
     if useful.
2. **Gates** — per stage, what blocks and what warns, with thresholds
   (from `principles.md` and `test-strategy.md`).
3. **Environments** — dev/preview, staging, production: protection,
   approvers, identity (OIDC trust subject), secrets and variables
   (names only), deployment strategy (rolling, blue/green, canary) from
   the architecture, rollback procedure.
4. **Promotion** — the digest/artifact id carried from build to deploy.
5. **Performance** — caching keys, parallelism, matrix, path filters,
   concurrency/cancel rules, timeouts on every job.

## Render for the platform

- Follow the platform file's layout, reuse mechanism and skeleton.
- Apply every principle: top-level least-privilege token permissions,
  pinned third-party steps (SHA/digest + version comment), untrusted
  input only via environment variables, OIDC for cloud access, deploy
  secrets only in environment-protected deploy jobs.
- Reuse the devcontainer's toolchain (same versions, or the same base
  image as the job container) so CI and local dev agree.
- Supply-chain steps (SBOM, scanning, signing, provenance) come from
  `devsecops:supply-chain` — call it or apply its defaults.
- Keep frontend and backend pipelines independent when they're separate
  modules (path filters, separate workflows/jobs), matching the separate
  devcontainers.

## Verify

1. Lint every file with the platform's tools (`actionlint` + `zizmor`
   for GitHub/Forgejo, CI Lint for GitLab, validation for Azure/Bitbucket,
   the declarative linter for Jenkins) and fix all findings.
2. Run `devsecops:audit`'s checklist over what you wrote.
3. Exercise it: a run on a branch (pushing needs the user's go-ahead per
   `git-workflow`), confirm each gate fails when it should (e.g. a
   deliberately failing test on a throwaway branch) and passes otherwise.
4. Record measured durations of the PR pipeline in `delivery.md`.

## Coverage checklist

- [ ] every module has lint, typecheck, tests and build on PRs, filtered by path
- [ ] every suite in `test-strategy.md` runs at its declared stage
- [ ] token permissions minimal at top level, elevated per job
- [ ] all third-party steps/images pinned; update bot configured for pins
- [ ] cloud access via OIDC, trust scoped to repo + environment/ref
- [ ] production deploy: protected environment, approval, protected ref only, concurrency lock
- [ ] one artifact/digest promoted through environments; rollback documented
- [ ] SBOM, signing and provenance on release artifacts
- [ ] every job has a timeout; PR pipeline duration measured
- [ ] `delivery.md` lists every required secret/variable by name and where it's configured

## Handoff

The files written, gates, what the user must configure outside the repo
(secrets, environments, cloud trust, branch protection), measured
durations, and follow-ups. Commits follow `git-workflow`.
