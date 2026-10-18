# History rewriting — only on an explicit request

Part of `git:workflow` (`../SKILL.md`). `commit --amend`, rebasing
commits already shared, `filter-branch`/`filter-repo`, `reset --hard`,
force pushes and deleting unlanded branches happen only when the user
asks for them, and then:

1. **Back up first:** `git bundle create <scratch>/<repo>-pre-rewrite.bundle --all`,
   or a `backup/<name>-<date>` branch — and say where it is.
2. **Say who diverges:** which remotes and branches will need a force
   push, and whether anyone else may have the old history.
3. **Force-push only when told to,** and then `--force-with-lease`, never
   `--force`.

Two routine cases need no extra ask beyond the landing itself: rebasing a
**never-pushed** branch onto its parent so `--ff-only` can land it, and
`git branch -D` of a branch whose PR the platform merged by rebase (after
confirming it shows as merged — the guard still asks).
