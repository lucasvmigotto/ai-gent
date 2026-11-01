---
name: workflow
description: This user's git rules — Conventional Commits, branch-per-context naming, the dev → staging → main chain, landing through platform PRs or local --no-ff/--ff-only merges, permission levels, safe history. Use whenever committing, branching, merging, opening or merging a PR, promoting or tagging ("commit this", "merge it in", "open the PR", "promote to main").
---

# Git versioning workflow

Applies to every git repository this user works in.

## Never

- **Commit directly to a long-lived branch** — `main`/`master`,
  `homolog`/`staging`, `dev`/`develop`. Work on a branch off `dev`. The
  one exception is a new repository's initial commit (*Branches*).
- **Add `Co-Authored-By`**, a "generated with" signature or similar —
  this overrides any default attribution, and applies the same to
  issues, pull requests and comments on both (`repo:issues`, `repo:pr`),
  not only commits.
- **Bypass hooks** (`--no-verify`, `-n`) — fix the cause and commit again.
- **Force-push without a lease, or rewrite history unasked** —
  `references/history.md` has what to do when asked.
- **Stage secrets or local config** (`.env` — `.env.example` is fine —
  credentials, keys, IDE/OS files), or **changes you didn't make**
  without asking.
- **Invent an issue id**, or ask for a token in the chat.

## What each permission level allows

| Action | One-off request | Approved workflow | Autonomous run |
|---|---|---|---|
| Commit on a work branch | ask, showing each message and its files | as each task or phase finishes; listed in the handoff | same |
| Land into `dev` (merge or PR) | ask once, for merge and cleanup | ask | yes |
| Push a work branch or `dev` | ask | ask | yes |
| Promote to `homolog`/`staging` or `main`/`master` | explicit request | explicit request | only if the permission names it |
| Tag or release by hand | explicit request | explicit request | explicit request |
| Force-push, delete a remote branch, rewrite history | explicit request, backup first | same | same |
| Comment on, close, label or assign an issue | when asked for that action | same | tracked items through `repo:sync` batches |

- **A one-off request** — a fix, a tweak: present each commit as its full
  message with the files it stages; when it splits, list the commits in
  order and ask one question for the set. A plan the user approves that
  lists its commits covers exactly those commits, in that turn.
- **An approved workflow** — a pipeline stage working through its
  phases, Spec Kit tasks, a plan the user said to carry out: versioning
  is part of the process, so commit as you go.
- **An autonomous run** — explicit permission given beforehand ("build it
  end to end, you can push"). It lasts for that run, covers the CI fixes
  `devsecops:watch` leads to (pipeline rule 8), and never answers product
  questions (pipeline rule 2). Say in the handoff what was pushed and
  merged under it. The guard hooks still ask on every push — they can't
  see the conversation — so a hands-off run also needs pushes allowed in
  the agent's permission settings.
- Describing a flow ("then `dev` goes to `main`") is not a request to
  promote: stop at the level below, report, and ask.

## Before staging

1. `git status` and `git diff` — everything in the tree, not only your
   own edits.
2. Changes you didn't make: list them and ask whether they belong here,
   in a separate commit, or stay out. Never commit, discard, stash or
   reset them silently.
3. Stage by path (`git add <paths>`, never `-A` over an unreviewed tree),
   then check `git diff --cached`.
4. Use the repository's configured identity; change it only when asked.

## Branches

- **The chain**, highest first: **`main`/`master` > `homolog`/`staging`
  (when there is one) > `dev`/`develop` > work branches.** Work enters at
  `dev` and moves up one level at a time. Branch off `dev` after
  `git fetch`; both `dev` and `develop` exist: ask which one.
- **Names:** `<prefix>/[<id>-]short-description`, kebab-case; an id only
  when confirmed. One context, one sub-branch, landed the moment it's done.
- **A new repository** starts with `git init -b main`, a one-line
  `README.md` committed alone as `chore: initial commit`, then `dev`; no
  `LICENSE` unless the user names one.
- Read `references/branches.md` when creating a branch, naming one, branching a
  hotfix or sub-branch, or starting a new repository. Getting `dev` ready
  (missing, behind, diverged) is in `references/chain.md`.

## Landing — merging into a long-lived branch

Count the branch's commits (`git log <parent>..<branch> --oneline | wc -l`).
Through the platform (PR, required checks, merge there; `repo:pr`) when it is
reachable and the user hasn't said merges are local; otherwise locally
(`references/local-merge.md`). Never self-approve, bypass branch protection or
merge red.

**Merge style**, both paths: **4 or more commits → a merge commit**
(`--no-ff`); **fewer → fast-forward** (`--ff-only`). Promotions between
long-lived branches follow the same rule locally; on GitHub they always use a
merge commit. Never skip a level, never land a work branch on a higher one,
settle conflicts on `dev`, never higher, and never pick a side blindly.

Read `references/landing.md` when merging or opening a PR: remote mode in
full, conflicts, and what to do after landing (pull the parent, delete the
branch, never delete a long-lived one).

## Commit messages — Conventional Commits, strictly

`type(scope): subject`

- **Types**, the standard set only: `feat`, `fix`, `docs`, `style`,
  `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
- **Scope** when the change is clearly one area (`feat(auth): …`); none
  when it's cross-cutting.
- **Subject:** imperative, lowercase after the colon, no trailing period,
  saying what changed — never `fix: update code`.
- **English, always** — regardless of the product's or the developer's
  own language (`../../references/language-policy.md`).
- **No prose body.** A short bullet list only when truly needed; the
  why, rationale and alternatives go in the PR description, the README or
  the CHANGELOG.
- **Footers** are metadata, one per line after a blank line:
  `BREAKING CHANGE: …` (with `type!:`), `Closes #12`, `Refs #45`.
- **Small and often:** single-purpose commits that read as the story of
  the work.

## Issues and work items

- **Tracked specs** (a `specs/NNN-*/tracker.md`, from `repo:sync` —
  pipeline rule 10, by default in remote mode): the story's ref names its
  branch, commits and PRs into `dev` say `Refs <ref>`, and the promotion
  PR to production lists `Closes <ref>` — without asking again, since
  those ids were confirmed when the items were created.
- **Anything else, only when asked:** `references/issue-linking.md` —
  where candidate ids come from, each platform's closing syntax, and when
  closing actually happens (only on the default branch).
- Ids go in footers, never in the subject.

## Tags and releases

**The pipeline cuts releases; humans don't tag by hand.** Never tag, bump a
version by hand or put `[skip ci]` in a release commit; the commit type you
choose is the release decision, and notes go under `## Unreleased`. Read
`references/tags.md` when releasing, tagging, or after a release lands (the
back-merge, the manual fallback).

## The guard hook

The `git` plugin's PreToolUse hook (`../../hooks/guard.sh`) enforces these
rules mechanically: it **blocks** `--no-verify`, co-author trailers and
force pushes without a lease, and **asks** before pushes, commits or
merges on `main`/`master` or `homolog`/`staging`, commits made directly
on `dev`/`develop`, `git branch -D`, `reset --hard`, `clean -f`,
`commit --amend`, history rewrites and `gh pr create` (the `repo`
plugin's guard asks before every `reporun --write`). A prompt is the hook
working, not an error — never work around it.

## Checklist

1. No commits yet? → one-line README as `chore: initial commit`, then `dev`.
2. On a long-lived branch? → get `dev` ready and branch off it.
3. About to stage? → the whole tree, nothing you didn't make, no secrets.
4. About to commit? → the permission table: ask, or commit as you go.
5. Context done? → land it now; the next starts from the updated parent.
6. Landing? → count commits; platform reachable → PR merged on the
   platform; else local; merge commit ≥4, fast-forward <4; promotions on
   GitHub always a merge commit; pull the parent, delete the branch.
7. Promoting? → only on an explicit request (or a permission naming it),
   one level at a time.
8. Writing the message? → `type(scope): subject`, no body paragraph,
   footers allowed, no co-author trailer.
