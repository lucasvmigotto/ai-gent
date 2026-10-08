# The release flow

Shared by the `git` and `devsecops` plugins. This is the default way a
project in this context versions, tags and releases: Conventional
Commits decide the bump, a script plans and applies it, GitHub Actions
runs it on every push to `main`. It is the pattern both ai-gent and
dottod run in production, adapted to whatever the project ships.

Read this when designing or setting up the release stage, when asked
about tagging, or when a release misbehaves. The portable model and the
pipeline a release runs in live in `devsecops:pipeline`; the branch and
commit discipline in `git:workflow`.

## The one flow

```
push to main
  └─ check job (skip only for chore(release) commits)
       └─ plan: release.py --dry-run   ── exit 3 → nothing to do, stop
            └─ apply: bump, changelog, commit "chore(release): X.Y.Z",
               tag -a X.Y.Z, push --atomic main + tag, create release
```

- **Commits decide the bump.** `feat` → minor; `fix`, `perf`, `refactor`
  → patch; `type!:` or a `BREAKING CHANGE:` footer → major; anything
  else (`docs`, `chore`, `ci`, `test`, `style`, `build`, `revert`) → no
  release. Merge-commit subjects and `chore(release):` commits are
  ignored — the commits a merge brings in are counted on their own.
- **Plain SemVer tags, no `v`.** `1.4.0`, never `v1.4.0`; a `v` belongs
  only in display text. Annotated tags (`-a`), one per release.
- **Hand-written notes win.** The `## Unreleased` section of
  `CHANGELOG.md` is promoted to `## X.Y.Z — YYYY-MM-DD` exactly as
  written; only when it is absent does the script generate a section
  from the commit subjects. So write the notes under `## Unreleased` on
  the branch that lands on `main` — that is where the release reads
  them.
- **The plan gate.** The workflow runs the script in `--dry-run` first
  and only proceeds on exit 0. The contract: `0` release planned, `3`
  nothing to do, `1` error, `2` bad arguments. A release commit that
  lands while the run is in flight is rejected by the atomic push and
  released on the next run — nothing half-publishes.
- **No loop, twice over.** The bot pushes with `GITHUB_TOKEN`, whose
  pushes do not trigger workflows, so the `chore(release)` commit cannot
  re-enter; the `chore(release)` prefix check on the check job is the
  second guard.
- **The release never races itself.** `concurrency: group: release,
  cancel-in-progress: false` — a slow run finishes, the next queues,
  and a moved `main` is caught by the atomic push instead of a merge
  conflict.

## Product shape decides the rest

The core above is identical everywhere; the two variants differ only in
what a release changes:

- **Versioned sub-packages** (ai-gent's plugins): the script bumps the
  `version` of each sub-package whose files changed since the tag, and
  — critically — follows symlinks, so a change to a shared file
  (`shared/pipeline.md`) bumps every package that links it. A package
  new since the tag, or already bumped by hand, is left alone.
- **Single product** (dottod): there is nothing to bump — the tag and
  the CHANGELOG section are the whole release.

Pick the variant by one question: *does the repository version more than
one thing?*

## If the release ships files

When a release publishes bytes beyond the git objects — an archive, a
binary, an installer — checksum them before publishing: build the
archive deterministically, `sha256sum` it, and append the sums to the
release notes (dottod's `dottod-scripts-<v>.tar.gz` + `.sha256sums`
is the reference shape). Checksums prove integrity, not provenance: hand
off to `devsecops:supply-chain` for signing and SLSA. A source-only
release (ai-gent) ships no files, so it has nothing to checksum — the
obligation is conditional on the artifact shape, not a ritual.

## The gate must be unskippable

The check job runs the same suite CI and humans run (one script), and it
must be reachable on every path to a release: if it lives in a separate
workflow file, call it with `workflow_call`; if it is inline and the
repo uses path filters, keep the gate unfiltered — a skipped `needs`
dependency would skip the release with it. Heavy integration (real
containers, a GUI, a live database) belongs on `dev` pushes or the
scheduled pipeline, never in the path that releases.

## No `release/*` branches

There is no release-preparation branch in this flow. The automation
reads commits on `main` since the last tag, so work on an unmerged
`release/*` branch is invisible to it, and `git:workflow`'s `release/`
prefix is a naming slot, not a trigger. Prepare the release the way you
prepare any change: on a working branch off `dev`, with the notes under
`## Unreleased`, merged up like everything else.
