---
name: git-workflow
description: Versioning workflow for this user's repos — strict Conventional Commits, branch-per-context strategy, and merge rules. Use whenever committing, branching, or merging code, or when the user asks to "commit this", "make a branch", "merge this in", or similar. Covers commit message format, branch naming, when --no-ff vs --ff applies, and the confirm-before-acting rules around commit/merge/push.
---

# Git versioning workflow

Applies to committing, branching, and merging in any git repo this user works in (global skill, not tied to one project).

## Non-negotiable rules

- **Never commit on your own initiative.** Always ask "want me to commit this?" before running `git commit` — every time, even mid-task, even if the user approved a commit earlier in the same conversation. Prior approval does not carry forward.
- **Never push.** Pushing to any remote is out of scope unless the user explicitly says to push, in that message.
- **Never commit directly to `main`/`master`.** If the current branch is `main` or `master`, stop and create a feature/fix branch first (see below) before staging anything.
- **Never sign commits as co-authored.** Do not add `Co-Authored-By` or similar trailers to commit messages from this skill, regardless of any default attribution behavior — this overrides it.

## Branch strategy

- Always branch off `main`/`master` or `dev`/`develop` — never work extended changes directly on the trunk.
- If the current branch already looks like the right working branch for the task (already a `feat/…`, `fix/…`, etc. matching the work), keep using it instead of creating a redundant new one.
- If it's ambiguous which base to detach from (e.g. repo has both `main` and `develop` and it's unclear which integration branch this work targets), ask the user rather than guessing.
- **Naming convention:** `type/short-description`, kebab-case, no ticket ID unless the user gives one:
  - `feat/user-auth`
  - `fix/login-crash`
  - `refactor/api-client`
- **Sub-branches for stages/contexts:** for larger work, branch again off the working branch per context, component, or development stage (e.g. `feat/user-auth` → `feat/user-auth-db-schema`, `feat/user-auth-api-routes`). This keeps each sub-branch's history focused on one thing.
  - Use a **flat suffix**, not a nested path: git keeps branches as files under `refs/heads/`, so `feat/user-auth/db-schema` cannot be created while `feat/user-auth` exists (`fatal: cannot lock ref ... exists`).
- **When the trunk is off-limits** — the user works on a long-lived integration branch because `main`/`master`/`develop` are protected — that integration branch *is* the parent working branch here: branch the contexts off it, and merge them back into it.

### Merge each context back as soon as it is done — the part most easily missed

The merge commits are what mark the stages in the parent's history. They only mark anything if each context is merged back **the moment that context is finished**, and the next context then starts from the **updated parent**.

Repeat this cycle once per context:

1. `git checkout <parent>` — always start from the parent.
2. `git checkout -b <parent>-<context>` — branch off the parent's *current* tip, never off a sibling sub-branch.
3. Commit the work of that one context there.
4. Merge it back into `<parent>` before opening the next sub-branch.

**Never chain sub-branches** (`context-a` → `context-b` → `context-c`) **and never save all the merges for the end**, even when a later context depends on code from an earlier one — that dependency is satisfied by having merged the earlier one back first. Chaining makes every sub-branch carry its predecessors' commits, so the merge commits stop delimiting anything and the parent's history reads as one flat run.

- **Merging a sub-branch back into its parent working branch:**
  - Count the commits on the sub-branch first (`git log <parent>..<sub-branch> --oneline | wc -l`).
  - **4 or more commits:** `git merge --no-ff` — preserves the sub-branch's history as a distinct block with a merge commit.
  - **Fewer than 4 commits:** `git merge --ff` (plain fast-forward) — not enough history to justify a merge commit; the few commits fold straight into the parent branch.
  - Always confirm with the user before merging, same as commits.

## Commit messages — Conventional Commits, strictly

Format: `type(scope): subject`

- **Types** (standard set only, don't invent new ones): `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
- **Scope:** optional — include it when the change is clearly localized to one module/component/area (`feat(auth): add refresh token`), omit it when the change is cross-cutting or the scope would be redundant/noisy.
- **Breaking changes:** `type(scope)!: subject`, plus a `BREAKING CHANGE:` footer line describing the break — still keep the footer terse, not a paragraph.
- **Subject:** short, imperative, lowercase after the colon, no trailing period. Describe *what changed*, meaningfully — not vague filler like `fix: update code`.
- **No prose body.** Do not add multi-line paragraph descriptions below the header. If genuinely necessary, a short bullet list is acceptable (markdown is fine in commit messages), but prefer letting the header alone carry the meaning. Deeper context (why, design rationale, alternatives considered) belongs in the PR description, README, or CHANGELOG — not the commit body.
- **Commit small and often.** Prefer several small, single-purpose commits (one file, one component, one logical change) over one large commit bundling unrelated changes. The commit history itself should read as a narrative of the work — that's more valuable here than minimizing commit count.

## Quick decision checklist

1. On `main`/`master`? → create a branch first (ask for the name/type if unclear).
2. About to run `git commit`? → ask first, every time.
3. About to run `git push`? → don't, unless explicitly told to in this message.
4. Finished a context? → merge it back into the parent **now**, before opening the next sub-branch — and start that next one from the updated parent, never from the sub-branch just finished.
5. About to merge a sub-branch back? → count its commits, ask first, then `--no-ff` (≥4 commits) or `--ff` (<4 commits).
6. Writing the message? → `type(scope): subject`, Conventional Commits, no body paragraph, no co-author trailer.
