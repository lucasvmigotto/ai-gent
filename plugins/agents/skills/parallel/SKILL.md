---
name: parallel
description: Develop independent work at the same time — several features, or a feature's frontend, backend and QA tracks — each on its own branch in a git worktree under .worktrees/, only where dependencies and file ownership make it safe, then land the branches on dev one at a time in dependency order, checking after each. Use for "build these features in parallel", "use worktrees", "run the tracks at once".
---

# Parallel development in worktrees

Several tracks at once, each in its own worktree, landed one by one. It
sits one level above `agents:orchestrate`: orchestrate runs one feature's
cards; this runs several tracks, and a track may itself be orchestrated.
Read `../../references/pipeline.md` (who owns what, rule 7 on versioning,
rule 8 on CI) and `../../references/agent-roles.md`; branches follow
`git:workflow`. The worktree tool:

```bash
WT="python3 <plugin>/scripts/wt.py"               # plan · add · list · remove
TOKREP="python3 <plugin>/scripts/token_report.py"  # Claude Code only
```

## 1. Decide what can run in parallel

`wt.py plan` lists the features whose dependencies (`specs/README.md`'s
"Depends on") are Implemented, and what each still needs. From that,
propose the tracks:

- **Features** with no dependency path between them run side by side.
- **A feature's frontend, backend and QA tracks** run side by side once
  its contract (`contracts/openapi.yaml`) is settled by `backend:spec`;
  before that, the frontend waits.
- **Hot spots get one owner, or wait.** Lockfiles, migration numbering,
  `contracts/openapi.yaml`, `CHANGELOG.md`, shared config and
  `specs/README.md`'s status columns: name the one track that may change
  each, or keep it out of every track and change it yourself when
  landing. Two tracks editing the same file is the main source of
  conflicts.
- **Capacity:** at most `max_parallel` tracks (default **3**; set in
  `.ai-gent/parallel.yaml`), fewer when each runs a dev stack on a small
  machine, or the account's rate limits are near.
- When nothing is safely parallel, say so and run the work in sequence
  (or one feature with `agents:orchestrate`).

Show the plan — tracks, branch names, worktree paths, the files each may
change, hot-spot owners, the landing order — and start once the user
agrees. An approved plan covers each track's commits on its own branch;
it never covers merges, pushes or releases.

## 2. One worktree per track

`wt.py add <branch>` for each track: a new branch off the chain's `dev`
(named per `git:workflow`, with the tracker ref when the feature has a
`tracker.md`), checked out in `.worktrees/<branch-slug>/`. The script
keeps `.worktrees/` out of git through `.git/info/exclude` — no project
file changes — and prints the track's isolation:

- **Compose project** `COMPOSE_PROJECT_NAME=<repo>-<branch-slug>`, so
  containers, volumes and networks don't collide.
- **Port offset** (+100 per track): publish the track's ports through an
  untracked `compose.worktree.yaml` in its worktree (already excluded),
  never by editing the shared compose file.
- **Dependencies** install per worktree (each has its own
  `node_modules`, virtualenv, build output).

Never put two tracks in one worktree, and leave the main checkout on
`dev` for landing.

## 3. Run the tracks

Each track runs in its own context, inside its worktree only:

- Write it a card (`agents:orchestrate`'s format,
  `../orchestrate/references/cards.md`): the goal, the worktree path every
  command runs in, the files it may change, the acceptance commands, and
  what it must never do — merge, push, rebase, or touch another worktree.
- The track runs its stage skill (`frontend:build`, `backend:build`,
  `qa:e2e`), or `agents:orchestrate` for a feature with many cards, and
  commits on its branch as tasks finish (`git:workflow`).
- A track that finds it needs another's output stops and reports it as an
  `[UPSTREAM GAP: …]`; it never edits across tracks.
- Keep a ledger in `.ai-gent/parallel-status.md`: track, branch,
  worktree, state, last commit, checks, blockers. `wt.py list` shows each
  worktree's commits ahead of `dev` and uncommitted changes.

## 4. Land — one branch at a time

In dependency order, then smallest first, from the main checkout on
`dev`:

1. Report the branch: commits, their count, the merge type (`--ff-only`
   under 4, `--no-ff` from 4), its checks. **Ask before each merge**,
   unless the user gave explicit permission beforehand to run the project
   on its own (`git:workflow`).
2. Merge it — in remote mode through its PR, opened and merged on the
   platform (`repo:pr`) — then make sure `dev` is green (the project's
   checks, or the merge's CI) before the next one: each landing must be
   green on its own.
3. **A conflict stops the landing.** Report the files and both sides;
   never resolve it on your own. The fix happens in the track's worktree
   (merge `dev` into its branch there, resolve, re-run its checks), then
   it lands again.
4. Update the feature's status columns in `specs/README.md` (the owning
   stage's statuses), run `repo:sync` for a tracked feature, and after a
   push follow CI with `devsecops:watch` (pipeline rule 8). On Claude
   Code, `$TOKREP tasks --project=<slug> --specs-dir specs` links the
   track's cost to the feature's `tasks.md` and the project-wide roll-up
   (`../orchestrate/references/token-economy.md`).

## 5. Clean up

After a branch lands: `wt.py remove <branch>` (it refuses a worktree with
uncommitted changes or unlanded commits), and bring that track's
containers down and remove their images per
`../../references/containers.md` §9 —
`<engine> compose -p <compose project> down -v`.

## Other agents

Claude Code is the reference: each track is a subagent (Agent tool, in
the background) whose card names its worktree path. Elsewhere:

| Agent | One track per worktree |
|---|---|
| opencode | a session in the worktree directory (`opencode .worktrees/<slug>`) |
| Codex | a session started in the worktree (`codex --cd .worktrees/<slug>`) |
| Cursor | a window per worktree, or its parallel agents |
| Gemini CLI, Copilot, Amp, Kiro, Windsurf | a session or window opened in the worktree directory |

`wt.py` and the landing rules are the same everywhere. An agent that can't
run sessions side by side runs the tracks one after another — still one
worktree each, so nothing changes in how they land.

## Handoff

Per track: branch, worktree, state, commits, checks; what landed and in
which order, conflicts waiting on the user, worktrees removed, and the
next step.
