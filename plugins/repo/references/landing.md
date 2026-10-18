# Landing through the platform — remote mode

How work lands on a long-lived branch when the code platform is reachable:
through a pull or merge request the agent opens and merges on the
platform, instead of a local merge. `git:workflow` decides when;
`repo:pr` and `reporun` do it.

## When remote mode is on

- `reporun detect` finds the platform, and its CLI is signed in or its
  token is in pass;
- and the user hasn't said beforehand that merges are local.

Otherwise merges stay local (`git:workflow` → *Merging back*), and the
handoff says why ("no GitHub token and gh isn't signed in"). A
repository with no remote is always local.

In remote mode **every merge into a long-lived branch** is a PR: a work
branch into `dev`/`develop`, and each promotion — `dev` into
`homolog`/`staging`, that into `main`/`master`. Nothing is merged locally
into one of them.

## The flow

1. **Push the branch** — asked, or covered by the user's up-front
   permission to run the project on its own (`git:workflow`).
2. **Open the PR** into its parent (`repo:pr` → *Open a PR*): the
   template, what changed and why, how it was tested, `Refs <ref>` for
   tracked stories; a promotion PR into production lists `Closes <ref>`
   for every story it carries (pipeline rule 10).
3. **Wait for it** — the required checks (follow them with
   `devsecops:watch`) and the required reviews. A red check caused by the
   change is fixed on the branch and pushed (pipeline rule 8); the PR
   updates itself. A required review the agent can't give waits for a
   person: report it and move on to other work. **Never self-approve**,
   never bypass branch protection, never merge a red PR.
4. **Merge on the platform** — asked like any merge, unless the run has
   the user's up-front permission (which covers `dev`; promotions only
   when it names them). The merge style follows the commit count, as
   locally:

| Platform | 4 or more commits (`--no-ff`) | Fewer than 4 (`--ff-only`) |
|---|---|---|
| GitHub | `gh pr merge <n> --merge --delete-branch` · API `merge_method: "merge"` | `gh pr merge <n> --rebase --delete-branch` · `merge_method: "rebase"` (linear; GitHub has no true fast-forward) |
| Azure Repos | `completionOptions.mergeStrategy: "noFastForward"`, `deleteSourceBranch: true` (`PATCH …/pullrequests/<id>`, `status: completed`) | `mergeStrategy: "rebase"` (rebase and fast-forward) |
| GitLab | `glab mr merge <n> --remove-source-branch` — the project's merge method decides merge commit vs fast-forward; say when it overrides the count rule | rebase first (`PUT …/merge_requests/<iid>/rebase`) where the project allows fast-forward |
| Bitbucket | `POST …/pullrequests/<id>/merge` `{"merge_strategy":"merge_commit","close_source_branch":true}` | `"merge_strategy":"fast_forward"` |

   A method the repository disallows (GitHub's *Allow rebase merging*
   off, say): use the closest allowed one and say so in the report.
5. **Clean up** — the platform deletes the source branch on merge; then
   locally: `git fetch --prune`, switch to the parent, `git pull
   --ff-only`, `git branch -d <branch>`. After a rebase merge the commits
   were rewritten and `-d` refuses: confirm the PR shows as merged, then
   ask before `git branch -D`. Long-lived branches are never deleted.

## Conflicts

The PR says it can't merge: in the branch's checkout (or worktree), merge
the parent into it (`git merge <parent>`), resolve with the user's input
— never by picking a side blindly — re-run the checks, and push; the PR
updates. Never rebase a branch that's already pushed unless the user
says so.

## Several branches (agents:parallel)

One PR per track, landed one at a time in dependency order: the next PR
is merged only after the previous one's merge is green on the target, so
each landing stands on its own.
