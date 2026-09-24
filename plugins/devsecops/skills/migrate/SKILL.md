---
name: migrate
description: Migrate CI/CD pipelines from one platform to another — between GitHub Actions, GitLab CI, Azure Pipelines, Bitbucket Pipelines, Jenkins and Forgejo/Gitea Actions (and from classic Azure release pipelines to YAML) — by inventorying every job, trigger, template, secret, environment and runner, mapping each through a shared concept model, flagging features with no equivalent, re-establishing OIDC trust and protections on the target, running both in parallel until parity, and cutting over with a checklist. Use for "move our pipelines from Jenkins to GitHub Actions", "convert .gitlab-ci.yml to Azure Pipelines", "migrate Bitbucket Pipelines to GitLab", or /devsecops:migrate.
---

# CI/CD migration

## Before starting

1. Read `../../references/principles.md`,
   `../../references/platforms/concepts.md`, and the platform files for
   **both** source and target.
2. Confirm source and target with the user, plus: the cutover date, the
   repository hosting (is the code moving too?), and whether the
   migration may also improve the pipelines or must be a faithful
   translation first (recommended: translate first, improve after).

## Process

1. **Inventory the source** — for every pipeline: triggers, jobs and
   their order, conditions and path filters, matrices, reused
   templates/libraries/components/pipes (resolved), images and runners,
   caches and artifacts, environments and approvals, deploy targets,
   secrets and variables (names and scopes only — never copy values),
   schedules, and integrations (status checks, chat notifications, issue
   links).
2. **Map through the concept model** — a table per pipeline:

   | Source element | Target equivalent | Notes / gap |
   |---|---|---|

   Name every **gap** explicitly (a feature the target lacks or gates
   behind a tier) and its workaround.
3. **Render the target** following its platform file — idiomatic for the
   target (its reuse mechanism, its expression syntax), not a literal
   line-by-line transliteration. Apply the principles as you go
   (permissions, pinning, OIDC), marking each change beyond translation.
4. **Re-create what isn't in files** — a checklist for the user:
   secrets/variables to create (names, scopes), environments and
   approvers, protected branches/rulesets, runners/agents, **cloud OIDC
   trust for the new CI identity** (new issuer, audience and subject
   claims), status checks required for merging, webhooks/integrations.
5. **Parallel run** — run old and new side by side on the same commits
   (target in non-deploying mode) until results match: same tests, same
   artifacts (compare checksums/SBOMs), same durations or better.
6. **Cutover** — freeze pipeline changes on the source, switch deploys to
   the target, make the target's checks required, disable (don't delete)
   the source pipelines, remove the source's cloud trust and credentials
   after a stabilization period.

## Output

- Target pipeline files.
- `docs/product/pipeline-migration.md` — inventory, mapping tables,
  gaps and workarounds, the settings checklist, parity results, cutover
  and rollback plan.
- Updated `docs/product/delivery.md` for the target platform.

## Coverage checklist

- [ ] every source job, trigger and schedule mapped or explicitly dropped with a reason
- [ ] every secret/variable listed by name and scope for re-creation
- [ ] OIDC trust re-established for the target identity; source trust scheduled for removal
- [ ] environments, approvals and protected refs reproduced
- [ ] parity run compared tests, artifacts and durations
- [ ] cutover and rollback steps written

## Handoff

Gaps that need a decision, the settings checklist for the user, parity
status, and the cutover plan. Commits follow `git:workflow`.
