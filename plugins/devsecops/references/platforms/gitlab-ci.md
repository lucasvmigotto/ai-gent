# GitLab CI/CD

## Layout

- One `.gitlab-ci.yml` at the root, split with `include:` (`local:`,
  `project:` + `ref:`, `remote:`, `template:`, and **CI/CD components**
  via `include: - component: $CI_SERVER_FQDN/<group>/<project>/<name>@<version>`).
- `workflow: rules:` decides whether a pipeline runs at all (avoid
  duplicate branch + MR pipelines); job `rules:` (not the legacy
  `only/except`) decide each job; `rules: changes:` for monorepo paths.
- `stages:` for coarse order, `needs:` for a DAG; `extends:` and
  `!reference` for reuse inside the file.

## Skeleton

```yaml
workflow:
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
    - if: $CI_COMMIT_TAG

stages: [check, build, deploy]

default:
  image: <toolchain-image>@sha256:<digest>
  interruptible: true
  timeout: 15m

test:
  stage: check
  script:
    - <lint> && <typecheck> && <test>
  cache:
    key:
      files: [<lockfile>]
    paths: [<dependency-cache-dir>]
```

## Secrets and identity

- CI/CD variables: **masked** (hidden in logs), **protected** (only on
  protected branches/tags), environment-scoped; prefer file-type variables
  for certificates. External secrets (Vault, cloud secret managers) via
  the `secrets:` keyword.
- **OIDC**: `id_tokens:` keyword issues a JWT per job with a chosen `aud`;
  configure the cloud trust on `project_path`, `ref`, `ref_protected`
  and `environment` claims.
- `CI_JOB_TOKEN`: restrict the **job token allowlist** so other projects
  can't be accessed with it.

## Environments and deployment

- `environment: name:` with **protected environments** (who can deploy)
  and **deployment approvals** (tier-dependent — check the instance).
- `resource_group:` serializes deploys per environment.
- Deploy only from protected branches/tags so protected variables and
  runners are available only there.

## Caching and artifacts

- `cache:` for dependencies (keyed by lockfile), `artifacts:` for build
  outputs with `expire_in`; pass them with `needs:` (`artifacts: true`).

## Supply chain features

- Security templates/components for SAST, secret detection, dependency
  and container scanning; several reports and the security dashboard are
  tier-dependent — fall back to open tools (Semgrep, gitleaks, Trivy,
  osv-scanner) when the tier lacks them.
- Container registry per project; sign with cosign using `id_tokens` for
  keyless signing.
- Renovate (self-hosted or the Renovate runner) for dependency updates.

## Attack surface

1. **Variable injection** — `$CI_COMMIT_MESSAGE`, `$CI_MERGE_REQUEST_TITLE`,
   branch names used unquoted in `script:`.
2. **Fork MR pipelines** — decide whether they run in the fork (default,
   no parent secrets) or the parent project; running untrusted code in the
   parent needs an explicit, reviewed trigger.
3. **Unprotected variables** holding deploy credentials, exposed to any
   branch.
4. **Unpinned `include:`** (`remote:` URLs, `project:` without a SHA/tag
   `ref:`, components without a version) — anyone controlling them
   controls the pipeline.
5. **Shared runners with privileged Docker-in-Docker** or shell executors
   reused across projects; untagged jobs landing on privileged runners.
6. **Over-permissive `CI_JOB_TOKEN`** access across projects.
7. **Artifacts** exposing secrets or `.env` files (artifacts are
   downloadable by anyone with pipeline access).

## Lint and verify

- CI Lint (Pipeline editor, API, or `glab ci lint`), `poutine` for
  security posture; run on a branch with `glab ci run` / `glab ci view`
  (push only with the user's go-ahead).
