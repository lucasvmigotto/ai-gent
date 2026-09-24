---
name: workflow
description: Versioning workflow for this user's repos — strict Conventional Commits, branch-per-context strategy, merge rules with context-branch cleanup, safe history handling, and on-request issue/work-item linking (Closes #N) for GitHub, GitLab, Azure DevOps, Bitbucket, Forgejo/Gitea and Jira. Use whenever committing, branching, merging or linking issues, or when the user asks to "commit this", "make a branch", "merge this in", "this closes #12", "link the issue", or similar. Covers commit message format, branch naming, --no-ff vs --ff-only, deleting merged branches, and the confirm-before-acting rules around commit/merge/push.
---

# Git versioning workflow

Applies to committing, branching, and merging in any git repo this user works in (global skill, not tied to one project).

## Non-negotiable rules

- **Never commit on your own initiative.** Ask before running `git commit`. Prior approval does not carry forward to new work.
  - **Batch approval:** when the user approves a plan that lists its commits (messages or clear one-per-item scope), that approval covers exactly those commits, in that turn. Any commit not on the list — an extra fix, a follow-up, anything in a later turn — needs its own ask.
- **Never merge without asking** — every merge, including the cleanup that follows it (see *Merging back*). Batch approval never covers merges.
- **Never push, delete remote branches, open PRs, or change remote issues** unless the user explicitly says so in that message. All of these publish something.
- **Never commit directly to `main`/`master`.** If the current branch is `main` or `master`, stop and create a feature/fix branch first (see below) before staging anything.
- **Never sign commits as co-authored.** Do not add `Co-Authored-By` or similar trailers to commit messages from this skill, regardless of any default attribution behavior — this overrides it.
- **Never bypass hooks** (`--no-verify`, `-n`). If a pre-commit or commit-msg hook fails, fix the cause and commit again.
- **Never rewrite history on your own** (see *History rewriting*).

## Before staging

1. `git status` and `git diff` — look at **everything** in the working tree, not only your own edits.
2. **Changes you didn't make** (the user edited a file in parallel, leftovers from earlier work): list them and ask whether they belong in this commit, a separate commit, or should be left alone. Never commit them silently, never discard, stash or reset them.
3. Stage deliberately (`git add <paths>`, not `git add -A` over an unreviewed tree), then check `git diff --cached` — what's staged is what gets committed.
4. Never stage secrets or local config: `.env` (but `.env.example` is fine), credentials, keys, tokens, local IDE/OS files. If one is about to be tracked and isn't gitignored, stop and say so.
5. Use the repo's configured identity. Change `user.name`/`user.email` (repo or global) only when the user asks.

## Branch strategy

- Always branch off `main`/`master` or `dev`/`develop` — never work extended changes directly on the trunk.
- Before branching, `git fetch` (read-only, always safe) and warn if the base is behind its upstream. Don't pull, rebase or merge the upstream in automatically — ask.
- If the current branch already looks like the right working branch for the task (already a `feat/…`, `fix/…`, etc. matching the work), keep using it instead of creating a redundant new one.
- If it's ambiguous which base to detach from (e.g. repo has both `main` and `develop` and it's unclear which integration branch this work targets), ask the user rather than guessing.
- **Naming convention:** `<prefix>/[<id>-]short-description`, kebab-case. Pick the prefix for the kind of work (below). When the work belongs to an issue or work item the user gave or confirmed, the id may go **right after the prefix**: `feat/123-user-auth`, `fix/PROJ-42-login-crash`, `hotfix/981-payment-timeout`. The prefix always comes first — never `123-user-auth` or `123/user-auth` — and never an invented id.
  - `feat/` or `feature/` — new functionality (`feat/user-auth`); follow whichever the repo already uses
  - `fix/` or `bugfix/` — a bug fix on the normal flow (`fix/login-crash`)
  - `hotfix/` — an urgent fix branched from the production branch (`hotfix/payment-timeout`); it merges back into the production branch **and** into `dev`/`develop` so the fix isn't lost (ask for both)
  - `release/` — release preparation (`release/1.4.0`)
  - `refactor/`, `perf/`, `docs/`, `test/`, `build/`, `ci/`, `chore/` — matching the Conventional Commit types
  - If the repo already has a branch-naming convention (existing branches, CONTRIBUTING, branch rules), follow it instead.
- **Sub-branches for stages/contexts:** for larger work, branch again off the working branch per context, component, or development stage (e.g. `feat/user-auth` → `feat/user-auth-db-schema`, `feat/user-auth-api-routes`). This keeps each sub-branch's history focused on one thing.
  - Use a **flat suffix**, not a nested path: git keeps branches as files under `refs/heads/`, so `feat/user-auth/db-schema` cannot be created while `feat/user-auth` exists (`fatal: cannot lock ref ... exists`).
- **When the trunk is off-limits** — the user works on a long-lived integration branch because `main`/`master`/`develop` are protected — that integration branch *is* the parent working branch here: branch the contexts off it, and merge them back into it.

### Merge each context back as soon as it is done — the part most easily missed

The merge commits are what mark the stages in the parent's history. They only mark anything if each context is merged back **the moment that context is finished**, and the next context then starts from the **updated parent**.

Repeat this cycle once per context:

1. `git checkout <parent>` — always start from the parent.
2. `git checkout -b <parent>-<context>` — branch off the parent's *current* tip, never off a sibling sub-branch.
3. Commit the work of that one context there.
4. Merge it back into `<parent>` and delete the context branch (below) before opening the next sub-branch.

**Never chain sub-branches** (`context-a` → `context-b` → `context-c`) **and never save all the merges for the end**, even when a later context depends on code from an earlier one — that dependency is satisfied by having merged the earlier one back first. Chaining makes every sub-branch carry its predecessors' commits, so the merge commits stop delimiting anything and the parent's history reads as one flat run.

### Merging back — and cleaning up

1. Count the commits on the context branch: `git log <parent>..<context> --oneline | wc -l`.
2. Ask once, naming both steps: *"Merge `<context>` into `<parent>` with `--no-ff`, then delete `<context>`?"*
3. Merge:
   - **4 or more commits:** `git merge --no-ff <context>` — preserves the context's history as a distinct block with a merge commit.
   - **Fewer than 4 commits:** `git merge --ff-only <context>` — the few commits fold straight into the parent. Use `--ff-only`, not `--ff`: plain `--ff` silently creates a merge commit when a fast-forward isn't possible.
   - If `--ff-only` fails because the parent moved (e.g. a sibling context was merged first): if the context branch was **never pushed**, rebase it onto the parent (`git rebase <parent>` on the context branch) and retry; if it **was pushed**, ask how to proceed.
4. **Conflicts:** stop and show them (files and hunks). Never resolve by blanket `-X ours`/`-X theirs` or by picking a side without understanding both. Resolve with the user's input, then continue the merge.
5. **Delete the context branch** once merged:
   - confirm it's merged: it appears in `git branch --merged <parent>`;
   - `git branch -d <context>` — the safe delete, which refuses unmerged work. Never `-D`.
   - If the branch exists on a remote, deleting it there (`git push <remote> --delete <context>`) publishes a change — ask separately, never assume.
   - Never delete the parent working branch, `main`/`master`, `dev`/`develop`, or any branch that isn't the context just merged.

## Commit messages — Conventional Commits, strictly

Format: `type(scope): subject`

- **Types** (standard set only, don't invent new ones): `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
- **Scope:** optional — include it when the change is clearly localized to one module/component/area (`feat(auth): add refresh token`), omit it when the change is cross-cutting or the scope would be redundant/noisy.
- **Breaking changes:** `type(scope)!: subject`, plus a `BREAKING CHANGE:` footer line describing the break — still keep the footer terse, not a paragraph.
- **Subject:** short, imperative, lowercase after the colon, no trailing period. Describe *what changed*, meaningfully — not vague filler like `fix: update code`.
- **No prose body.** Do not add multi-line paragraph descriptions below the header. If genuinely necessary, a short bullet list is acceptable (markdown is fine in commit messages), but prefer letting the header alone carry the meaning. Deeper context (why, design rationale, alternatives considered) belongs in the PR description, README, or CHANGELOG — not the commit body.
- **Footers are allowed** — they're metadata, not prose: `BREAKING CHANGE: …`, `Closes #123`, `Refs #45` (see *Linking issues*). One per line, after a blank line.
- **Commit small and often.** Prefer several small, single-purpose commits (one file, one component, one logical change) over one large commit bundling unrelated changes. The commit history itself should read as a narrative of the work — that's more valuable here than minimizing commit count.

## Linking issues and work items — only when asked

Do this only when the user asks ("this closes #12", "link the issue", "find the related issue"). Never add issue references on your own initiative.

### 1. Find the candidate ids, cheapest first

1. **Ids the user gave** in the request.
2. **The branch name — check it, then ask.** Extract likely ids from the current branch (and the context branch being merged):
   - forms that are usually ids: a leading number segment (`feat/123-login` → `123`), `#123`, `gh-123`/`issue-123`, Jira-style keys (`PROJ-123`), Azure Boards `AB#123`;
   - forms that usually aren't: numbers inside words or versions (`oauth2`, `v2`, `http2`, `k8s`, `s3`), dates (`2026-09`), port-like or long numbers.
   Then **ask**: *"The branch `feat/123-login` has `123` — is that the issue/work item this closes?"* Use it only after the user confirms.
3. **Earlier commits on the branch** that already reference an id (`git log <parent>..HEAD --grep='#[0-9]'`).
4. **Look it up on the remote**, if the user wants a search:
   - detect the platform from `git remote get-url origin` — `github.com` → GitHub, `gitlab` → GitLab, `dev.azure.com`/`visualstudio.com` → Azure DevOps, `bitbucket.org` → Bitbucket, another host → possibly Forgejo/Gitea (confirm);
   - use its CLI if installed and authenticated (`gh issue list --search`, `glab issue list --search`, `tea issues`, `az boards query`), otherwise its REST API with `curl` **only** with a token already present in the environment (`GH_TOKEN`/`GITHUB_TOKEN`, `GITLAB_TOKEN`, …) — public repositories may not need one;
   - **never ask the user to paste a token into the chat**; with no access, ask the user for the id instead.
5. Show the candidates (id, title, state) and ask which ones this change **closes** and which it only **relates to**.

### 2. Write the reference

- As a **footer** of the commit that completes the work — not in the subject:
  ```
  fix(auth): refresh token before expiry

  Closes #123
  Refs #98
  ```
- Closing keywords by platform:
  - **GitHub, GitLab, Forgejo/Gitea, Bitbucket Cloud:** `Closes #N` / `Fixes #N` / `Resolves #N`; cross-repo `owner/repo#N` (GitHub/Gitea), `group/project#N` (GitLab).
  - **Azure DevOps:** `AB#N` links a work item from GitHub-hosted code; in Azure Repos, `#N` mentions link work items, and closing on completion is a PR setting — check how the organization has it configured.
  - **Jira:** the issue key (`PROJ-123`) links; transitions (`PROJ-123 #done`) work only if smart commits are enabled.
  - `Refs #N` (or just `#N`) links without closing.
- **When it closes:** closing keywords act only when the commit or PR reaches the repository's **default branch**. Merging into `dev` or a feature branch links the issue but won't close it — say so whenever the target isn't the default branch.
- If the user asks for a PR, put the same `Closes #N` lines in the PR description (the most reliable place for closing on merge).
- Never close, comment on, label, assign or otherwise change an issue through a CLI or API unless the user asks for that specific action.

## History rewriting — only on explicit request

`commit --amend`, `rebase` of already-shared commits, `filter-branch`/`filter-repo`, `reset --hard`, deleting unmerged branches — only when the user asks, and then:

1. **Back up first**: `git bundle create <scratch>/<repo>-pre-rewrite.bundle --all`, or a `backup/<name>-<date>` branch, and say where it is.
2. Say which remotes and branches will diverge and need a force push, and whether anyone else may have the old history.
3. Force-push only when told to, and then with `--force-with-lease`, never `--force`.
4. The one routine exception: rebasing a **never-pushed** context branch onto its parent to allow `--ff-only` (see *Merging back*).

## Pull requests — only when asked

Opening a PR publishes the branch; do it only on an explicit request (it implies pushing, which needs the same explicit order). The PR description carries what commits don't: the why, design rationale, alternatives, testing notes, screenshots, and any `Closes #N` lines.

## Quick decision checklist

1. On `main`/`master`? → create a branch first (ask for the name/type if unclear).
2. About to stage? → review the whole tree, ask about changes you didn't make, never stage secrets.
3. About to run `git commit`? → ask first, unless it's one of the commits in a plan the user just approved.
4. Hook failed? → fix the cause; never `--no-verify`.
5. About to run `git push`, open a PR, or touch a remote issue? → don't, unless explicitly told to in this message.
6. Finished a context? → merge it back into the parent **now**, before opening the next sub-branch — and start that next one from the updated parent.
7. About to merge a context back? → count its commits, ask once for merge + delete, then `--no-ff` (≥4) or `--ff-only` (<4), then `git branch -d` the context.
8. Asked to link an issue? → ids from the request, then the branch name (confirm with the user), then history, then a remote lookup; write `Closes #N`/`Refs #N` footers; remember closing happens on the default branch.
9. Writing the message? → `type(scope): subject`, Conventional Commits, no body paragraph, footers allowed, no co-author trailer.
