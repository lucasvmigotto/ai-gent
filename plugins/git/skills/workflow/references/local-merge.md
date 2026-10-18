# Landing locally

Part of `git:workflow` (`../SKILL.md`): the path when the code platform
isn't reachable, or the user said merges are local. With the platform
reachable, work lands through PRs instead (`repo:pr`).

1. **Count** the branch's commits:
   `git log <parent>..<branch> --oneline | wc -l`.
2. **Ask once**, naming both steps: *"Merge `<branch>` into `<parent>`
   with `--no-ff`, then delete `<branch>`?"* (or as the permission table
   allows).
3. **Merge**, from the parent:
   - **4 or more commits:** `git merge --no-ff <branch>` — the branch's
     history stays a distinct block behind a merge commit.
   - **Fewer than 4:** `git merge --ff-only <branch>` — never plain `--ff`,
     which silently makes a merge commit when it can't fast-forward.
   - `--ff-only` fails because the parent moved (a sibling landed first):
     a branch **never pushed** is rebased onto the parent
     (`git rebase <parent>` on the branch) and merged again; a branch
     already pushed — ask how to proceed.
   - **Promotions** (`dev` → `homolog`/`staging` → `main`) use the same
     rule: `git switch main && git merge --no-ff dev`, or `--ff-only`
     under 4 commits.
4. **Conflicts:** stop and show them (files and hunks). Never resolve by
   blanket `-X ours`/`-X theirs` or by picking a side without understanding
   both; resolve with the user, then `git merge --continue`.
5. **Delete the landed branch:**
   - confirm it's merged (`git branch --merged <parent>` lists it);
   - `git branch -d <branch>` — the safe delete, which refuses unmerged
     work;
   - deleting it on a remote (`git push <remote> --delete <branch>`)
     publishes a change: ask separately;
   - never delete the parent, `main`/`master`, `dev`/`develop`, or any
     branch other than the one just landed.
