# The branch chain — getting `develop` ready

Part of `git:workflow` (`../SKILL.md`). A repository whose integration level is still
named `dev` is read the same way: `dev` is that level there.

## Get `develop` ready

`git fetch` first, then the first case that fits:

- **Neither local nor remote:** create it from the trunk —
  `git switch -c develop main`.
- **Only on the remote:** create it at the remote's point, tracking it —
  `git switch -c develop origin/develop`.
- **Local and behind its remote** (the remote has commits the local branch
  lacks, and the local branch has none of its own —
  `git rev-list --count origin/develop..develop` is 0): bring it to the remote's
  point with `git switch develop && git merge --ff-only origin/develop`. Nothing is
  lost.
- **Local and diverged from its remote** (commits on both sides): stop.
  Show the local-only commits (`git log origin/develop..develop --oneline`) and
  ask; suggest a backup branch first (`git branch backup/develop-<date> develop`)
  before `develop` is reset to the remote's point.
- **Local and up to date with its remote, or never pushed:** use it as it
  is — **even when it's behind `main`/`master`**. Don't update it from the
  trunk; if it looks far behind, see below.

Right after your own landing, `git pull --ff-only` of the parent is
routine. Any other update of a long-lived branch from its upstream —
before branching, say — is reported and asked.

## When `develop` is left behind

When `develop` lacks commits that `main`/`master` or `homolog`/`staging` have,
flag it before branching: how many commits it lacks from each level above
(`git rev-list --count develop..main`), its last commit date, and whether the
flow still passes through it (recent landings on it, or the upper levels
moving without it). Then ask whether to use the exception — there's no
fixed threshold, the user decides. If they agree:

- branch off the highest level that holds the current work —
  `homolog`/`staging` when it's ahead of `main`, otherwise `main`;
- land the work back on **that same level**, asking first as always;
- leave `develop` alone: don't update it, land on it or recreate it.
