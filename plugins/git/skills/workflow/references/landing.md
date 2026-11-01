# Landing — merging into a long-lived branch

Count the branch's commits (`git log <parent>..<branch> --oneline | wc -l`),
then take one of two paths:

- **Through the platform — remote mode.** When `reporun detect`
  (`repo:pr`) finds the platform's CLI signed in or its token in pass,
  and the user hasn't said merges are local: push, open a PR into the
  parent, wait for its required checks (`devsecops:watch`) and reviews,
  then merge it on the platform — `repo:pr` has the procedure and each
  platform's merge options. Never self-approve, never bypass branch
  protection, never merge red.
- **Locally**, when the platform isn't reachable (say why):
  `local-merge.md`.

**Merge style**, both paths: **4 or more commits → a merge commit**
(`--no-ff`); **fewer → fast-forward** (`--ff-only`; GitHub's closest is a
rebase merge). **Promotions** between long-lived branches follow the same
rule locally; on a platform that can't fast-forward (GitHub) they always
use a merge commit, so the lower branch can still fast-forward to the
upper one. Never skip a level, never land a work branch on a higher one,
and settle conflicts on `dev`, never higher.

**Conflicts** stop the landing: show the files and hunks and resolve them
with the user — never `-X ours`/`-X theirs` or a side picked blindly. In
remote mode, merge the parent into the branch, resolve, re-run the
checks, and push; the PR updates.

**After landing:** bring the parent up to date with `git pull --ff-only`
— routine right after your own merge; a parent that diverged is reported
and asked. Delete the landed branch: the platform removes the remote
one (on a local merge, deleting it on the remote is asked); locally
`git branch -d`. If a rebase merge rewrote the commits and `-d` refuses,
confirm the PR shows as merged, then `git branch -D` (the guard
confirms it). Never delete a long-lived branch.
