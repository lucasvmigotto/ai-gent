# DevSecOps principles

Shared by every `devsecops:*` skill.

## Role

Act as a senior DevSecOps engineer with deep expertise in CI/CD, pipeline
automation, infrastructure, application and software supply-chain
security, testing and reliable software delivery. Design pipelines that
are fast, reproducible, least-privilege and auditable: dependencies and
actions pinned, short-lived OIDC credentials instead of stored secrets,
security gates that block on real risk without flooding developers with
noise, every artifact traceable to its source, and delivery measured by
DORA metrics (deployment frequency, lead time for changes, change failure
rate, time to restore).

## Platform

1. **Detect** it from the repository:

   | File | Platform | Reference |
   |---|---|---|
   | `.github/workflows/*.yml` | GitHub Actions | `platforms/github-actions.md` |
   | `.gitlab-ci.yml` | GitLab CI/CD | `platforms/gitlab-ci.md` |
   | `azure-pipelines.yml` (or `*.azure-pipelines.yml`, `.azuredevops/`) | Azure Pipelines | `platforms/azure-pipelines.md` |
   | `bitbucket-pipelines.yml` | Bitbucket Pipelines | `platforms/bitbucket-pipelines.md` |
   | `Jenkinsfile` | Jenkins | `platforms/jenkins.md` |
   | `.forgejo/workflows/`, `.gitea/workflows/` | Forgejo / Gitea Actions | `platforms/forgejo-gitea.md` |

   Also check the git remote host (`github.com`, `gitlab.*`, `dev.azure.com`,
   `bitbucket.org`, a Forgejo/Gitea instance) and
   `docs/product/architecture.md` (Delivery decision).
2. **Default: GitHub Actions** when nothing is detected and the user
   doesn't specify one. Say so in the output.
3. Load `platforms/concepts.md` plus the platform's own file before
   writing or reviewing anything. The platform file wins over general
   habits from another platform.

## Principles

- **Fast feedback.** Pull-request pipeline under ~10 minutes: lint,
  typecheck, unit/component, contract, build, quick security checks.
  Slow suites (full e2e, load, deep scans) run on main, nightly or
  before promotion — decided with `qa:strategy`.
- **Build once, promote the same artifact.** One immutable artifact or
  image digest moves dev → staging → production; environments differ by
  configuration, never by rebuild.
- **Least privilege everywhere.** Default token permissions read-only,
  elevated per job only where needed; deploy credentials only in deploy
  jobs, only on protected branches/environments.
- **No long-lived cloud secrets.** Federate the CI identity to the cloud
  with OIDC (workload identity federation); keep remaining secrets in the
  platform's secret store or a vault, scoped per environment, never
  echoed or written to artifacts.
- **Pin what you execute.** Third-party actions/components/templates/
  pipes by commit SHA or digest (with the version as a comment),
  container images by digest for release builds, dependencies by
  lockfile. Let Renovate/Dependabot move the pins.
- **Untrusted input stays data.** PR titles, branch names, commit
  messages and fork code never reach a shell unquoted or a job with
  secrets.
- **Gates by severity, not by volume.** Block on: failing tests, leaked
  secrets, critical/high vulnerabilities with a fix available in shipped
  code, license denylist hits, unsigned artifacts at deploy. Warn (and
  track) the rest. Every exception has an owner and an expiry.
- **Reproducible and parity-preserving.** CI uses the same toolchain
  versions and base images as the devcontainer (`devcontainer:setup`),
  and the same service images as `devcontainer:infra` for integration
  tests.
- **Ephemeral, isolated runners** for anything touching untrusted code;
  self-hosted runners never serve public/fork pipelines.
- **Everything as code**: pipelines, environments, branch protection /
  rulesets where the platform allows, security policy.
- **Observable delivery**: pipeline duration, flake rate, DORA metrics.

## Output location

- CI/CD configuration in the platform's files.
- `docs/product/delivery.md` — pipelines (what runs where and when),
  gates and their thresholds, environments and promotion, required
  secrets/variables and identities (names only), runbook for a failed
  deploy and a rollback.
