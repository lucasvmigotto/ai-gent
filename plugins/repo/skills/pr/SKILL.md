---
name: pr
description: Work with pull and merge requests on GitHub, Azure Repos, GitLab and Bitbucket — read them with their checks, diffs and review threads, open one with the project's template and linked issues, comment, approve and merge — via the platform's CLI or REST API, tokens from pass, every change asked first. Use for "open a PR", "status of my PR", "read the review comments", "merge it".
---

# Pull and merge requests

Read `../../references/runner.md` first: every call goes through
`reporun`, the CLI when it's installed and the REST API otherwise. Then
the platform's file in `../../references/platforms/`. Branches, commits
and merge rules are `git:workflow`'s; this skill is the platform side.

## Read — freely

- **Which PRs:** open ones for this repository, mine, or the one for the
  current branch (`git branch --show-current` → the PR whose head it is).
- **One PR:** title, description, state, draft, target branch,
  mergeability, the review decision and the **checks** (GitHub check
  runs, Azure policies, GitLab pipelines, Bitbucket statuses). A failing
  check is a run to explain: hand it to `devsecops:watch`.
- **Review threads:** read them all, group by file, and say which are
  resolved. When asked to address them, the fixes are commits on the
  branch (`git:workflow`); replying on the thread is a write.
- **The diff:** `pr diff` or the API's diff, for a review the user asked
  for.

## Open a PR — when the user asks

1. The branch is pushed (pushing is `git:workflow`'s, and asked there).
2. **Target:** the branch the workflow says — a work branch goes to
   `dev`/`develop`, not `main`, unless the user said so.
3. **Title:** the change in one line — the Conventional Commit subject
   when the branch has one commit, a summary otherwise.
4. **Body:** the project's template (`.github/pull_request_template.md`,
   `.gitlab/merge_request_templates/`, `.azuredevops/pull_request_template.md`)
   filled in: what changed and why, how it was tested, and the linked
   items — `Closes #N` / `AB#N` / `KEY-12` for the ones the user confirmed
   (`git:workflow`'s linking rules; Azure: `--work-items` too).
5. **Draft** when the work isn't ready for review.
6. Show the title, target and body, then `reporun … --write`; the `repo`
   guard confirms it with the user. Report the link, then follow its
   checks with `devsecops:watch`.

## Other writes — the specific one asked for

Comment, reply to a thread, approve, request changes, update the
description, mark ready, close, and **merge**: each only when the user
asks for that action, said first in one line, through `--write`.

- **Merge** follows `git:workflow`: the strategy it sets for that branch
  (`--no-ff` merges or `--ff-only`, as the repository's rules map to
  merge, squash or rebase), only with green required checks and the
  approvals the branch needs, and the source branch deleted when the
  workflow says. Never bypass branch protection or merge a red PR.
- **Approve** only when the user says they reviewed it or asks you to
  approve after a review you did together; never self-approve to get past
  a policy.

## Handoff

The PR's link and state, its checks (and the `devsecops:watch` result for
any that ran), open review threads, what's still to confirm, and the next
step.
