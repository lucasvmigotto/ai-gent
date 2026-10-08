---
name: pipeline
description: Design and write CI/CD pipelines — pull request, main, release, scheduled — with gates, monorepo path filters, caching, build-once-promote, environments, OIDC and deploy strategies, plus docs/product/delivery.md. GitHub Actions by default, other platforms on request. Use for "set up CI/CD", "add a deploy pipeline", "create the workflows".
---

# CI/CD pipeline design

## Before starting

1. Read `../../references/principles.md` (role, platform detection and
   default, principles), `../../references/platforms/concepts.md`, and the
   detected platform's file in `../../references/platforms/`.
2. Read `../../references/pipeline.md`, `../../references/releasing.md`
   (the default release flow), and the inputs that exist:
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
   structure for OIDC. The release scheme defaults to
   `../../references/releasing.md` — ask only if the user wants
   something different.

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
     deploy of the already-built digest. Unless the user overrides,
     implement `../../references/releasing.md` as-is: Conventional
     Commits decide the bump, a plan script plans and applies it, a
     push-to-main workflow checks then releases, plain SemVer annotated
     tags cut by the automation.
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
- **Run the project's task recipes in its tools images**
  (`../../references/containers.md` §2, §6): CI jobs call `just lint`,
  `just test`… exactly as developers and agents do, so a local green run
  and a CI green run mean the same thing. Without a tools layer, reuse the
  devcontainer's toolchain versions and propose `devcontainer:setup`'s
  tools layer.
- **Registry logins** follow `containers.md` §3 and §8: `dhi.io` with the
  `DOCKER_HUB_PAT` secret; pushes to `ghcr.io` as the repository owner
  with the job's `GITHUB_TOKEN` (`packages: write` on that job only); to
  `docker.io` with `DOCKER_HUB_USERNAME` (default: the owner) and
  `DOCKER_HUB_PAT`. Image names lowercase; build with BuildKit (or
  buildah) cache mounts and a registry cache.
- **Cloudflare R2 configuration uses one shared scheme**, so a deploy reads
  the same way in every project:

  | Name | Kind | Holds |
  |---|---|---|
  | `CLOUDFLARE_R2_ACCOUNT_ID` | **variable** | the access-key **id** (fed to `AWS_ACCESS_KEY_ID`) |
  | `CLOUDFLARE_R2_ACCOUNT_SECRET` | **secret** | the secret half |
  | `CLOUDFLARE_R2_ENDPOINT_S3_CLIENT` | **variable** | the **S3-API** endpoint (`--endpoint-url`) |
  | `CLOUDFLARE_R2_BUCKET_ID` | **variable** | the bucket |

  Only the secret is a secret. The account id, endpoint and bucket are
  identifiers, not credentials: keeping them as variables makes them
  visible, reviewable and diffable, and keeps the secret store to the one
  value that needs it. A deploy that puts them in secrets anyway still
  works but hides configuration the next person has to debug blind.
  Some S3-compatible stores differ; keep the `CLOUDFLARE_R2_*` names when
  the provider is R2 and adapt the prefix for another, rather than
  inventing a per-project spelling.

  Pass the values through `env:` — never interpolate `${{ secrets.… }}`
  straight into a command path — and validate them in a step that fails
  loudly, naming the missing variable and where it is set. A deploy that
  silently syncs to the wrong bucket is worse than one that refuses to
  start.
- Supply-chain steps (SBOM, scanning, signing, provenance) come from
  `devsecops:supply-chain` — call it or apply its defaults.
- Infrastructure plan/apply jobs follow `devsecops:iac`'s delivery wiring
  when the project has an `infra/` directory.
- Keep frontend and backend pipelines independent when they're separate
  modules (path filters, separate workflows/jobs), matching the separate
  devcontainers.

## Verify

1. Lint every file with the platform's tools (`actionlint` + `zizmor`
   for GitHub/Forgejo, CI Lint for GitLab, validation for Azure/Bitbucket,
   the declarative linter for Jenkins) and fix all findings.
2. Run `devsecops:audit`'s checklist over what you wrote.
3. Exercise it: a run on a branch (pushing needs the user's go-ahead per
   `git:workflow`), confirm each gate fails when it should (e.g. a
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
durations, and follow-ups. Commits follow `git:workflow`.
