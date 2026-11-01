# Tags and releases

- **The default: the pipeline cuts releases; humans don't tag by hand.** A
  project's push-to-default-branch job runs `scripts/release.py`, which
  reads the Conventional Commits since the last tag to pick the bump,
  writes the CHANGELOG section, commits `chore(release): X.Y.Z` and tags
  `X.Y.Z` (`../../../references/releasing.md`; per platform,
  `../../../references/release-platforms.md`). So the commit type you choose
  is the release decision, and release notes go under `## Unreleased` on
  the branch that lands. Never tag the same commit by hand, never bump a
  version by hand, never put `[skip ci]` in a release commit.
- **The release comes back down the chain.** After tagging on the top
  branch, the job merges the `chore(release): X.Y.Z` commit back into
  `dev`/`develop` (`--no-ff`, message `chore(release): merge X.Y.Z back
  into dev`). So after a release, `git pull --ff-only` on both; if the
  back-merge warned, run the command it names. Don't re-tag or re-bump.
- A repository without that automation gets it from `devsecops:pipeline`
  rather than from habit; don't start tagging by hand to fill the gap.
- **The manual fallback**, only when CI cannot push (the repository says
  so in `docs/product/delivery.md`) and only when asked: the project's
  `scripts/release.sh`, which plans with `release.py`, commits, tags and
  pushes. Without one, an annotated tag on the released commit
  (`git tag -a 1.4.0 -m "1.4.0"`), plain SemVer without a `v`. Pushing a
  tag is a push.
