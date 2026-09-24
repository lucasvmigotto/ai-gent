# Azure Pipelines (Azure DevOps)

## Layout

- YAML pipelines (`azure-pipelines.yml` or any path registered as a
  pipeline); classic/release pipelines are legacy — migrate to YAML
  multi-stage.
- Hierarchy: `stages` → `jobs` (or `deployment` jobs) → `steps`.
- Reuse with **templates** (`template: templates/build.yml` for
  stages/jobs/steps/variables) and **`extends:` templates** — an extends
  template plus the "required template" check lets the organization
  enforce security steps every pipeline must run.
- Expressions: `${{ }}` compile-time (templates, parameters), `$[ ]`
  runtime, `$(var)` macro. Mixing them wrongly is the most common bug.
- Path filters with `trigger: paths:` and `pr: paths:` (the `pr:` trigger
  applies to GitHub/Bitbucket repos; Azure Repos uses branch policies for
  PR validation).

## Skeleton

```yaml
trigger:
  branches: { include: [main] }
pr:
  branches: { include: [main] }

pool:
  vmImage: ubuntu-latest

stages:
  - stage: Check
    jobs:
      - job: test
        timeoutInMinutes: 15
        steps:
          - checkout: self
            persistCredentials: false
          - task: Cache@2
            inputs:
              key: '"deps" | "$(Agent.OS)" | <lockfile>'
              path: <dependency-cache-dir>
          - script: <lint> && <typecheck> && <test>
```

## Secrets and identity

- Secret variables (never mapped to env automatically — map explicitly
  per step), **variable groups** linked to **Azure Key Vault**.
- **Service connections with workload identity federation** (OIDC) to
  Azure (and other clouds via the same mechanism) instead of client
  secrets; scope each connection to the resource group it deploys and
  restrict which pipelines may use it (pipeline permissions).
- Project settings: **limit job authorization scope** to the current
  project, and protect access to repositories in YAML pipelines.

## Environments and deployment

- `deployment` jobs targeting an **environment** with **approvals and
  checks** (manual approval, branch control, business hours, required
  template, exclusive lock).
- Strategies: `runOnce`, `rolling`, `canary` with lifecycle hooks.
- Deploy from the artifact produced in the build stage
  (`PublishPipelineArtifact` / `download:` in deployment jobs).

## Supply chain features

- Microsoft Security DevOps / GitHub Advanced Security for Azure DevOps
  (licensing-dependent) or open tools (Semgrep, gitleaks, Trivy,
  osv-scanner) as script steps.
- Azure Artifacts feeds with upstream sources; lock them down to avoid
  dependency confusion (scoped/namespaced packages, no public fallback
  for internal names).
- Sign images with Notation (Azure Key Vault) or cosign.

## Attack surface

1. **Fork PR builds** — keep "make secrets available to builds of forks"
   off and "require a team member's comment before building a pull
   request" on.
2. **Settable-at-queue-time variables** that feed scripts — restrict or
   validate them (and the "limit variables that can be set at queue time"
   setting).
3. **Service connections** usable by any pipeline, or with
   subscription-wide rights.
4. **Macro injection** — `$(Build.SourceBranchName)` or PR fields
   inserted into inline scripts; pass via `env:` and quote.
5. **Self-hosted agents** shared across projects of different trust, or
   not reset between jobs (prefer scale-set agents with fresh images).
6. **Marketplace tasks/extensions** — install only trusted publishers;
   pin task major versions (`Task@2`) and review updates.
7. **Missing required-template checks** on production environments,
   letting a pipeline skip the security template.

## Lint and verify

- Validate via "Validate" in the editor or a preview run
  (`az pipelines run --branch <b>`), `poutine` for security posture.
  Pushing a branch needs the user's go-ahead.
