# The branch chain — getting `dev` ready

Part of `git:workflow` (`../SKILL.md`). Where the repository has
`develop` rather than `dev`, `develop` is that level.

## Get `dev` ready

`git fetch` first, then the first case that fits:

- **Neither local nor remote:** create it from the trunk —
  `git switch -c dev main`.
- **Only on the remote:** create it at the remote's point, tracking it —
  `git switch -c dev origin/dev`.
- **Local and behind its remote** (the remote has commits the local branch
  lacks, and the local branch has none of its own —
  `git rev-list --count origin/dev..dev` is 0): bring it to the remote's
  point with `git switch dev && git merge --ff-only origin/dev`. Nothing is
  lost.
- **Local and diverged from its remote** (commits on both sides): stop.
  Show the local-only commits (`git log origin/dev..dev --oneline`) and
  ask; suggest a backup branch first (`git branch backup/dev-<date> dev`)
  before `dev` is reset to the remote's point.
- **Local and up to date with its remote, or never pushed:** use it as it
  is — **even when it's behind `main`/`master`**. Don't update it from the
  trunk; if it looks far behind, see below.

Right after your own landing, `git pull --ff-only` of the parent is
routine. Any other update of a long-lived branch from its upstream —
before branching, say — is reported and asked.

## When `dev` is left behind

When `dev` lacks commits that `main`/`master` or `homolog`/`staging` have,
flag it before branching: how many commits it lacks from each level above
(`git rev-list --count dev..main`), its last commit date, and whether the
flow still passes through it (recent landings on it, or the upper levels
moving without it). Then ask whether to use the exception — there's no
fixed threshold, the user decides. If they agree:

- branch off the highest level that holds the current work —
  `homolog`/`staging` when it's ahead of `main`, otherwise `main`;
- land the work back on **that same level**, asking first as always;
- leave `dev` alone: don't update it, land on it or recreate it.
