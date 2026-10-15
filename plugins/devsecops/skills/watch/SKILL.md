---
name: watch
description: Follow CI/CD runs to the end and explain failures — find a commit's runs, watch them without blocking, read only the failed steps' logs, classify the cause with evidence, check secrets (names only), variables, environments and permissions — on GitHub Actions, GitLab CI, Azure Pipelines, Bitbucket, Jenkins, Forgejo/Gitea. Use for "watch the CI", "why did the pipeline fail", "is the deploy done".
---

# Watch a pipeline run

Follows what a push, merge or release actually did in CI, and turns a red
run into a cause and a fix. Read-only by default: it observes, reads logs
and settings, and acts (re-run, cancel, dispatch) only when asked.
Designing pipelines is `devsecops:pipeline`; a security review of their
configuration is `devsecops:audit`.

## Before starting

1. Find the platform from the repo (`.github/workflows/`,
   `.gitlab-ci.yml`, `azure-pipelines.yml`, `bitbucket-pipelines.yml`,
   `Jenkinsfile`, `.forgejo/workflows/`) and read the **Observe a run**
   section of its file in `../../references/platforms/` — the commands
   differ per platform. A repo can have more than one.
2. Check the CLI is signed in (`gh auth status`, `glab auth status`,
   `az account show` plus `az devops configure --list`). Without a CLI,
   use the REST API with a token the user already has; never ask for a
   token to be pasted into the chat.

## Find the right run

- **By commit, not "the latest".** Take the SHA that was pushed or merged
  and list the runs for it (`gh run list --commit <sha>`). "Latest" picks
  up a neighbour's run, a bot's release commit or a scheduled job.
- **Follow the chain.** A push can start runs that start others: a release
  workflow that tags and then triggers a deploy (`workflow_run`, pipeline
  triggers, downstream pipelines), a bot commit (a release's
  `chore(release)`) that runs nothing because pushes made with the
  platform token don't trigger workflows. Name every run the change
  started, and the ones that were expected but never ran.
- **Path filters and branches.** A workflow that didn't run may be
  filtered out (`paths`, `branches`, `rules:changes`), not broken. Check
  the trigger before reporting it missing.

## Watch without blocking

- Poll in the background at a pace matched to the job (a 30-second
  interval for a 5-minute build; a single check after the expected
  duration for a long one), never in a tight loop, and keep working on
  other things meanwhile. The platform's own watch command (`gh run
  watch --exit-status --interval 30`) is fine run in the background.
- Report the result per job, not just per run: a green run can hide a
  skipped deploy, and a red one a single flaky job.

## When a run fails

1. **Read only the failed steps' logs** (`gh run view <id>
   --log-failed`, `glab ci trace <job>`, the failed timeline record on
   Azure). Search them (`grep -iE "error|fatal|denied|not found"`); never
   paste a whole log into the conversation — logs can carry hostnames,
   paths and data the user didn't mean to share.
2. **Classify the cause**, with the log lines as evidence:
   - **code or test** — the change broke something; name the test and
     the assertion;
   - **flaky** — passes on retry or fails the same test intermittently;
     check the previous runs before calling it flaky;
   - **runner or infrastructure** — no runner, out of disk, image pull
     limits, network timeouts;
   - **identity and permissions** — token scopes, `permissions:`,
     protected branches, a git identity a step needs (`git tag -a`,
     `git commit` on the runner);
   - **secrets and variables** — one the workflow names that doesn't
     exist in that scope (repository, environment, organisation), or an
     expired one; check by **name** only;
   - **a dependency that moved** — a deleted action tag or image, a
     yanked package, a pinned version gone from the registry;
   - **a missing tool** on the runner or in the container (`jq`,
     `shfmt`), or a toolchain the job doesn't pin;
   - **an external outage** — the platform's status page, a registry or
     an API down.
3. **Was it this change?** Compare with the previous run of the same
   workflow on the default branch: already red there means the change
   inherited the failure; say so instead of fixing someone else's bug
   unasked.
4. **Reproduce locally when it's code**, in the same container image or
   toolchain version the job uses (`devcontainer:workflow`), before
   proposing the fix.

## Configuration that explains a failure

Read these to explain a failure, not to audit them (that's
`devsecops:audit`):

- **Secrets** — list names and scopes only; no platform returns values,
  and none may be printed, logged or echoed to check one.
- **Variables** — names first; read a value only when the diagnosis needs
  it, and treat it as possibly sensitive.
- **Environments** — protection rules, required reviewers, branch
  policies, wait timers: a deploy "stuck" waiting may be waiting for a
  person.
- **Token permissions** — default workflow permissions, job
  `permissions:`, whether workflows may push or create releases.
- **Branch protection and required checks** — a merge blocked by a check
  that never runs on that path.

## Acting on a run

Only when the user asks, and with what it will do said first:

- **Re-run failed jobs** (`gh run rerun <id> --failed`) for a flaky or
  infrastructure failure — never for a code failure, which needs a fix.
- **Never re-run, dispatch or cancel a job that deploys, publishes or
  tags** without an explicit go-ahead for that job: it changes what users
  get.
- A fix to the pipeline itself goes through `devsecops:pipeline` and
  `git:workflow` like any other change.
- **Inside the product pipeline**, the stage that pushed owns the fix
  (pipeline rule 8 in `../../references/pipeline.md`): this skill
  explains the failure, the build stage fixes and commits it, and the
  push of the fix is asked — unless the user gave explicit permission
  beforehand to run the project on its own (`git:workflow`). Three rounds
  still red: stop and report.

## Report

```text
<workflow> · <run id> · <commit> · <result>
  <job> ✓/✗/skipped   (one line per job; the failed step for ✗)
Cause      <class>: <one line>, with the log line that shows it
Since      new with this change | already failing on <branch> since <run>
Fix        <what to change, where> — or the question to ask
Next       the run to watch next (a chained deploy, the re-run)
```

Keep watching chained runs until the change has fully landed (released,
deployed), or say which ones are still pending.
