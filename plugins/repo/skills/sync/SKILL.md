---
name: sync
description: Mirror Spec Kit features into the tracker — a GitHub issue, Azure Boards Feature, GitLab issue or Jira Epic per feature and a child item per user story with its tasks as a checklist — keeping states in step with tasks.md and specs/README.md and closing at Verified once the work is on the integration branch. One confirmation per batch. Use for "create the issues for the specs", "sync the tracker".
---

# Specs ↔ tracker sync

Read `../../references/sync.md` (items, states, per-platform mapping, the
batch, `tracker.md`) and `../../references/runner.md` first. Two tools do
the deterministic parts, both in `../../scripts/`:

```bash
SPECSYNC="python3 <plugin>/scripts/specsync.py"   # what the specs imply
REPORUN="python3 <plugin>/scripts/reporun.py"     # the tracker calls
```

Run it when the user asks to track the specs, and at the hand-off of a
pipeline stage for a feature that already has a `tracker.md` (pipeline
rule 10). It never decides what's done: the specs do.

## 1. Where

1. `reporun detect` — the platform, its CLI and token. The tracker is
   the platform's own unless Jira is configured (`ai-gent.jira.url`) and
   the user wants it there; a feature already synced keeps the platform
   in its `tracker.md`. No token: `secrets:pass`.
2. `reporun chain` — the integration branch where work lands
   (`integrated_on`) and where closing keywords act
   (`keywords_close_on`). On `ask`, ask the user which branches make the
   flow before anything closes.
3. Platform specifics, once per repository: the Azure process's work
   item types and state categories, the Jira project key and its
   issue types, the GitHub labels that already exist (`sync.md`).

## 2. Plan

`specsync features` for the overview; then, for each feature the user
wants tracked, `specsync plan NNN` (`--json` for the bodies and hashes).
Show the plan before anything is written: per item, the action
(`create`, `update`, `state`), the state change, and the title. Report
orphans (in `tracker.md`, gone from the specs) and ask what to do; never
delete.

Before items move to `done`, check the work is on `integrated_on`
(`sync.md` → *States*); if it isn't yet, hold them at `review` and say
why.

Read the remote state of existing items first (one `reporun` read per
item, or a search). An item closed or reopened on the tracker against
what the specs say is drift: report it and ask, don't overwrite.

## 3. Apply — one batch, one confirmation

1. Write the batch file in the scratchpad from the plan: creates (feature
   first, then each story with its parent link), body updates for
   `update`, state moves for `state` — using the platform's calls from
   `sync.md`. Every step that creates or changes an item carries a
   `record` with the plan's `key`, `state` and `hash`.
2. `reporun batch FILE --dry-run`, and show the list.
3. When the user confirms the batch (or the run has their up-front
   permission to carry out the project on its own — `git:workflow`),
   `reporun batch FILE --write --out RESULT`. The `repo` guard asks once.
4. `specsync record NNN --platform <p> --where <owner/repo|org/project|KEY> --from RESULT`
   — even when the batch stopped part-way: what it created is recorded,
   so the next plan doesn't create it again. Fix the failure (a missing
   field, a state name) and plan again.
5. Commit `tracker.md` with the stage's work, or on its own:
   `docs(specs): sync NNN with the tracker` (`git:workflow`).

## 4. Using the refs

Once a feature is tracked, its `tracker.md` gives the refs everyone
uses — confirmed ids, so `git:workflow` may use them without asking
again:

- **Branch:** `feat/<ref number>-<story-slug>` for a story's work
  (`feat/121-export-timesheet`).
- **Commits and PRs into the integration branch:** `Refs <ref>` footers
  (`Refs #121`, `Refs AB#4512`, `Refs KEY-12`).
- **The promotion PR to production:** `Closes <ref>` for each story it
  carries (a backstop where the platform closes on merge).

## Handoff

Items created and changed with links, what's held at `review` and why,
drift and orphans waiting on the user, the refs to use, and the
`tracker.md` commit.
