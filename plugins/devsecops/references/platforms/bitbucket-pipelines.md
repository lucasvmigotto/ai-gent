# Bitbucket Pipelines

## Layout

- One `bitbucket-pipelines.yml` at the root with `pipelines:` sections:
  `default`, `branches:`, `pull-requests:`, `tags:`, `custom:` (manual or
  scheduled).
- A `step` has an `image`, `script`, optional `caches`, `artifacts`,
  `services` (sidecar containers, e.g. a database), `size` (memory), and
  `max-time`. Group with `parallel:` and `stage:`.
- Reuse with YAML anchors (`definitions:` + `&anchor` / `*anchor`) and
  **pipes** (`pipe: atlassian/<name>:<version>`); shared pipeline
  configuration across repos depends on the plan — verify availability.
- Monorepo paths: `condition: changesets: includePaths:` on a step.

## Skeleton

```yaml
image: <toolchain-image>@sha256:<digest>

definitions:
  caches:
    deps: <dependency-cache-dir>
  steps:
    - step: &test
        name: test
        max-time: 15
        caches: [deps]
        script:
          - <lint> && <typecheck> && <test>

pipelines:
  pull-requests:
    '**':
      - step: *test
  branches:
    main:
      - step: *test
      - step:
          name: deploy staging
          deployment: staging
          oidc: true
          script:
            - <deploy using $BITBUCKET_STEP_OIDC_TOKEN>
```

## Secrets and identity

- **Secured** repository, workspace and **deployment** variables (masked
  in logs); deployment variables are scoped to one environment.
- **OIDC**: `oidc: true` on the step exposes `BITBUCKET_STEP_OIDC_TOKEN`;
  configure the cloud trust on the repository UUID and deployment
  environment claims.

## Environments and deployment

- `deployment: test | staging | production` (or custom environment
  names) on a step; **deployment permissions** (who can deploy, which
  branches) are plan-dependent — verify.
- Promotion between environments with manual `trigger: manual` steps.
- Concurrency: only one deployment per environment runs at a time;
  later ones pause.

## Supply chain features

- No built-in SAST/SCA suite on all plans — run open tools (Semgrep,
  gitleaks, Trivy, osv-scanner) as steps or pipes; Snyk/other vendor pipes
  where licensed.
- Renovate (Mend-hosted app or self-hosted) for dependency updates.

## Attack surface

1. **Pipes and images** pinned only by tag — pin pipes to an exact
   version and images by digest; a pipe is arbitrary code in your step.
2. **Variable injection** — `$BITBUCKET_BRANCH`, PR titles interpolated
   unquoted into scripts.
3. **Pull requests from forks** — check how the workspace handles them;
   never expose secured or deployment variables to untrusted code.
4. **Deployment variables** reachable from branches that shouldn't
   deploy (missing deployment permissions).
5. **Self-hosted runners** shared across repositories of different trust.
6. **Artifacts** carrying `.env` or credentials between steps.
7. **`after-script`** leaking secrets on failure paths.

## Lint and verify

- The Bitbucket Pipelines validator (online or in the editor); run a
  `custom:` pipeline on a branch to exercise changes (push only with the
  user's go-ahead). No mainstream security linter exists — apply the
  checklist above manually.
