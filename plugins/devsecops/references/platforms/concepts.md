# CI/CD concept map

Equivalent concepts across platforms. Use it to design once and render per
platform (`devsecops:pipeline`), to review with the same checklist
(`devsecops:audit`), and to translate (`devsecops:migrate`). Details and
exact syntax live in each platform file — verify against the platform's
current documentation before relying on a feature, since tiers and
features change.

| Concept | GitHub Actions | GitLab CI/CD | Azure Pipelines | Bitbucket Pipelines | Jenkins | Forgejo / Gitea Actions |
|---|---|---|---|---|---|---|
| Config file | `.github/workflows/*.yml` (many) | `.gitlab-ci.yml` (+ `include:`) | `azure-pipelines.yml` (per pipeline) | `bitbucket-pipelines.yml` | `Jenkinsfile` | `.forgejo/workflows/` / `.gitea/workflows/` |
| Unit of work | job → steps | job → `script` | stage → job → steps | step → `script` | stage → steps | job → steps |
| Ordering | `needs:` | `stages:` + `needs:` (DAG) | `dependsOn:` | step order, `parallel:`, `stages:` | `stages`, `parallel` | `needs:` |
| Conditions | `if:` | `rules:` | `condition:` | branch/PR sections, `condition: changesets` | `when { }` | `if:` |
| Path filters | `on.<event>.paths` | `rules: changes:` | `trigger.paths` | `condition: changesets: includePaths` | `when { changeset }` | `on.<event>.paths` |
| Matrix | `strategy.matrix` | `parallel: matrix:` | `strategy: matrix:` | `parallel:` (explicit) | `matrix { }` | `strategy.matrix` |
| Reuse | reusable workflows (`workflow_call`), composite actions | `include:` (local/project/remote/template), CI/CD components, `extends:`, `!reference` | templates (`template:`, `extends:`) | pipes, YAML anchors, shared pipelines (plan-dependent) | shared libraries (`@Library`) | reusable workflows, actions |
| Secrets | repo/org/environment secrets | CI/CD variables (masked, protected), external secrets | secret variables, variable groups (Key Vault-linked) | secured repository/workspace/deployment variables | Credentials + `withCredentials` | repo/org secrets |
| Cloud OIDC | `permissions: id-token: write` + cloud login action | `id_tokens:` keyword | service connection with workload identity federation | `oidc: true` → `BITBUCKET_STEP_OIDC_TOKEN` | plugin-dependent; prefer a vault | instance/version-dependent — verify |
| Environments & approvals | environments with required reviewers, branch policies, wait timers | environments, protected environments, deployment approvals | environments with approvals and checks | deployment environments with permissions | `input` step, folder/credential scoping | limited — verify |
| Deploy concurrency | `concurrency:` | `resource_group:` | exclusive lock check on environment | deployment concurrency per environment | `lock` (Lockable Resources) | `concurrency:` (verify support) |
| Cache | `actions/cache`, setup-* `cache:` | `cache:` | `Cache@2` | `caches:` | agent workspace / plugins | `actions/cache` (runner-dependent) |
| Artifacts | `actions/upload-artifact` / `download-artifact` | `artifacts:` + `dependencies`/`needs` | `PublishPipelineArtifact` / `DownloadPipelineArtifact` | `artifacts:` | `archiveArtifacts` / `stash` | upload/download artifact actions |
| Runners | GitHub-hosted, self-hosted, larger runners | shared/group/project runners (`tags:`) | Microsoft-hosted, self-hosted, scale-set agents | Atlassian cloud, self-hosted runners | controller + agents (docker/k8s) | `forgejo-runner` / `act_runner` |
| Workflow lint | `actionlint`, `zizmor` | CI Lint (UI / API / `glab ci lint`) | pipeline validation on run, `az pipelines` | online validator | `Jenkinsfile` linter (`/pipeline-model-converter/validate`) | `actionlint` (mostly compatible) |
| Security scanner for pipelines | `zizmor`, `poutine`, OpenSSF Scorecard | `poutine` | `poutine` | — (manual checklist) | — (manual checklist) | `zizmor` (partial) |

## Portable pipeline model

Design in these terms first, then render to the platform:

1. **Triggers** — PR/MR, push to main, tags/releases, schedule, manual.
2. **Jobs** — each with inputs, outputs (artifacts), runner/image,
   permissions, secrets needed, timeout, and the paths that trigger it
   (frontend vs. backend in a monorepo).
3. **Gates** — which job results block which next step, and thresholds.
4. **Environments** — dev/preview, staging, production: who can deploy,
   approvals, which identity each uses.
5. **Promotion** — the artifact/digest that moves between environments.
6. **Schedules** — nightly deep scans, load tests, dependency updates.
