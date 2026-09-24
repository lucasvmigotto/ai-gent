---
name: audit
description: Audit existing CI/CD pipelines and repository delivery settings for security and reliability problems — script injection, untrusted-code triggers (pull_request_target, fork MRs/PRs), over-broad tokens and permissions, unpinned actions/components/pipes/images/libraries, long-lived cloud secrets instead of OIDC, unprotected deploy credentials and environments, self-hosted runner exposure, cache/artifact poisoning, secrets in logs, missing gates, slow or flaky pipelines — on GitHub Actions, GitLab CI, Azure Pipelines, Bitbucket Pipelines, Jenkins or Forgejo/Gitea. Produces findings ranked by severity with evidence (file:line), an attack or failure scenario and the fix. Use for "review our pipelines", "is our CI secure", "audit GitHub Actions workflows", or /devsecops:audit.
---

# CI/CD audit

## Before starting

1. Read `../../references/principles.md`,
   `../../references/platforms/concepts.md` and the file for **each**
   platform found in the repo (a repo can have more than one).
2. Inventory: every pipeline file, included templates/components/shared
   libraries (resolve what they point to and whether it's pinned), the
   runners/agents referenced, environments and deploy jobs, secrets and
   variables referenced by name.
3. **Settings outside the repo** matter as much as the files (default
   token permissions, branch protection/rulesets, fork PR policy,
   environment protection, runner registration). Read them with the
   platform CLI if authenticated (`gh api`, `glab api`, `az devops`/`az
   pipelines`), otherwise list what the user must check and ask.

## Run the tools first

Where available: `zizmor` and `actionlint` (GitHub, Forgejo/Gitea),
`poutine` (GitHub, GitLab, Azure Pipelines), OpenSSF Scorecard (GitHub),
the platform linter. Treat tool output as leads to confirm, not as the
report.

## Checklist

Work through the platform file's **Attack surface** list, then:

- **Identity & secrets** — OIDC vs. static keys; trust scoped to
  repo+environment/ref; secrets only in jobs that need them; no secrets in
  logs, artifacts, caches or PR-visible outputs.
- **Untrusted input** — every place PR/branch/commit/issue text reaches a
  shell; every trigger that runs fork code with secrets or write access.
- **Pinning** — actions/components/pipes/libraries/images/tasks by
  SHA/digest/exact version; update automation present.
- **Permissions** — token defaults, per-job elevation, service connection
  scope, job-token cross-project access.
- **Runners** — ephemeral vs. persistent, isolation between trust levels,
  host socket/privileged mode, public-repo exposure.
- **Deployment** — protected environments, approvals, protected refs
  only, concurrency locks, artifact promotion vs. rebuild, rollback.
- **Gates** — tests, scans and their thresholds actually block; nothing
  marked `continue-on-error`/`allow_failure` without a reason.
- **Reliability & speed** — timeouts, caching, flaky retries hiding
  failures, duplicate pipelines, missing path filters, PR duration.

## Findings

For each finding:

| Field | Content |
|---|---|
| Severity | Critical · High · Medium · Low · Info (by exploitability × impact) |
| Location | `file:line` (or the setting's path in the UI/API) |
| Evidence | the exact snippet or setting value |
| Scenario | how it's exploited or how it fails, concretely |
| Fix | the corrected snippet or setting, platform-correct |
| Effort | S · M · L |

Order by severity, then effort. Group duplicates (one finding, many
locations). Mark anything you couldn't verify (settings you couldn't
read) as **Unverified** with what to check.

## Output

`docs/product/pipeline-audit-<date>.md` (or the location the user
prefers): summary with counts per severity, the findings table, quick
wins, and a remediation plan. Offer to apply fixes; apply them only when
the user agrees, one commit per finding group, and re-run the tools
afterwards.

## Handoff

Top risks in one line each, what needs settings access or a decision
from the user, and the remediation plan. Commits follow `git-workflow`.
