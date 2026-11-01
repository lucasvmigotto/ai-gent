# The release flow

Shared by the `git` and `devsecops` plugins. **The default: the pipeline
cuts every release, and nobody tags by hand.** Conventional Commits
decide the bump, a script plans and applies it, and the CI platform runs
it on every push to the default branch. This is the pattern ai-gent and
dottod run in production, adapted to the platform and to whatever the
project ships.

Every new project gets it from `devsecops:pipeline`: a `scripts/release.py`,
a `CHANGELOG.md` with `## Unreleased`, and a release job on pushes to the
default branch. The script, the choice between its single-version and
multi-package forms, and the job for GitHub Actions, Azure Pipelines,
GitLab CI, Bitbucket Pipelines, Forgejo/Gitea Actions and Jenkins are in
`release-platforms.md`; this file is the model they implement. A project
that opts out says so in `docs/product/delivery.md`; a project that cannot
let CI push uses the manual fallback in `release-platforms.md` instead.

Read this when designing or setting up the release stage, when asked
about tagging, or when a release misbehaves. The pipeline a release runs
in lives in `devsecops:pipeline`; the branch and commit discipline in
`git:workflow`.

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
- **`--dry-run --json`.** Instead of the text plan, prints one JSON
  object on stdout: `version`, `bump`, `since` (the base tag, `null`
  when none), `targets[]` (`kind` `plugin`, `manifest` or `source`,
  `label`, `from`, `to`, `bump`) and `changelog` (how the CHANGELOG
  changes, for display). Exit codes are unchanged; exit 3 and errors
  print nothing on stdout. `--json` without `--dry-run` is a usage error
  (exit 2). `scripts/release.py` and `shared/release-single.py` print the
  same keys; `scripts/consistency.py` checks it.
- **Back-merge.** The release commit and tag land on the default branch
  only, so the job then merges the release commit back into the
  integration branch (`dev`/`develop`) with
  `git merge --no-ff -m "chore(release): merge X.Y.Z back into <integ>"`
  and pushes it, when the project has one — otherwise the integration
  branch's version and CHANGELOG lag and the next plan counts from an old
  tag. A conflict or refused push warns with the manual command and does
  not fail the release (`release-platforms.md`).
- **No loop.** The release commit must not release again. On GitHub
  Actions the bot pushes with `GITHUB_TOKEN`, whose pushes trigger no
  workflow, and the `chore(release)` prefix check on the check job is the
  second guard; on every other platform that prefix check is the guard
  (`release-platforms.md`).
- **The release never races itself.** On GitHub Actions
  `concurrency: group: release, cancel-in-progress: false` (other
  platforms: `release-platforms.md`) — a slow run finishes, the next queues,
  and a moved `main` is caught by the atomic push instead of a merge
  conflict.

## Product shape decides the rest

The core above is identical everywhere; the two variants differ only in
what a release changes:

- **Versioned sub-packages** (ai-gent's plugins): the script bumps the
  `version` of each sub-package whose files changed since the tag, and
  — critically — follows symlinks, so a change to a shared file
  (`shared/pipeline.md`) bumps every package that links it. A package
  new since the tag, or already bumped by hand, is left alone. Other
  versioned parts — a docs site's `docs/site/package.json` — are listed
  in `release.json` (`{"manifests": [{"file": "…"}]}`) and bumped the same
  way, alongside the plugins.
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

## If the release ships a container image

Tag the already-built, already-promoted digest with the release's
SemVer — never rebuild it; "build once, promote the same artifact"
holds through the release too (`principles.md`). The release job's last
step retags that digest in the registry:

- `X.Y.Z` always.
- `latest` for the newest release on the default/stable track — skip it
  on a pre-release or a maintenance-branch patch.
- `X` and `X.Y` (floating major and major.minor tags), moved to point
  at the newest release in that line, only when the project publishes
  them — decide once per project and record it in `delivery.md`.

Retagging a digest is a registry push of a new manifest reference, not
a build step: no Containerfile runs again, no new layer, same digest.
It reuses the registry logins and image-naming conventions
`devsecops:pipeline`'s Render step already sets up (`containers.md` §3,
§8).

## Other platforms

The flow above is the platform-neutral model; its exact mechanics are in
`release-platforms.md`, which renders it for each platform and answers,
per platform, how the job pushes with the CI identity, how it avoids
looping, whether the tag it pushes starts the tag pipeline, and which
permissions a human grants once. Two points hold everywhere and bite when
forgotten:

- **Never `[skip ci]` in the release commit.** Most platforms skip the tag
  pipeline with it; the `chore(release):` check on the head commit is the
  loop guard instead.
- **GitHub Actions is the odd one out**: pushes made with `GITHUB_TOKEN`
  start no workflows, tags included. If something must run for the tag
  (package, publish, deploy), do it in the release workflow itself or push
  with a GitHub App token.

`concepts.md`'s "Release / tag publishing" row maps each platform's
release object (and says plainly where there isn't one).

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

## How ai-gent itself releases

- `.github/workflows/release.yml` runs on every push to `main`: the checks,
  then `scripts/release.py`. A release bumps the `version` of each plugin
  whose files (or the shared files it links) changed, promotes
  `CHANGELOG.md`'s `## Unreleased` section (or generates one from the
  commits), commits `chore(release): X.Y.Z`, tags `X.Y.Z` and publishes a
  GitHub Release.
- Don't bump plugin versions or tag by hand; a version already changed by
  hand since the last tag is left alone.
- Preview with `scripts/release.py --dry-run`.
- The same flow is what the skills set up in other projects: the
  single-version template is `shared/release-single.py` (tested by
  `scripts/test-release-single.sh`) and the per-platform release jobs are
  in `release-platforms.md`. Change the bump rules in both scripts
  together.
- After pushing, the bot's release commit lands on `main`: run
  `git pull --ff-only` before the next local commit.
