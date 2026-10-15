# Version footer

Read this when building a docs site's footer, or when its version is
wrong (`dev` in production, one release behind, no link).

A repository versions more than one thing: the project — or each of its
parts — and the docs site. The footer names them all, so a reader knows
exactly which code and which docs they are looking at:

```text
ai-gent@v1.12.0 · docs@v0.1.0
app@v1.2.0 · api@v1.2.0 · docs@v1.0.0
```

## Entries

- **One per versioned part, then `docs`.** Find the parts from the tags
  and manifests (`app-1.2.0` and `api-1.2.0` tags, or one `Cargo.toml`).
  A single-product repo has one project entry. The name is the project's
  (or part's) short name, as the tags or manifests spell it.
- **Project version: its source of truth at build time.** The release tag
  for a tag-released project, otherwise the manifest (`Cargo.toml`,
  `package.json`, `pom.xml`). Never a constant in the site's code.
- **Docs version: `docs/site/package.json`.** The site's own SemVer, raised
  by the release flow when `docs/site/` changes (with the shared release
  flow `devsecops:pipeline` sets up, list it in `release.json`), never by
  hand on every edit.
- **Display `name@vX.Y.Z`.** Tags stay plain SemVer (`1.2.0`, `app-1.2.0`);
  the `v` is display text only.

## Links

Each entry links to the repository tree it was built from, so the version
is something a reader can open:

| Build | Text | Link |
| --- | --- | --- |
| HEAD is the part's release tag | `app@v1.2.0` | `<repo>/tree/<tag>` (`app-1.2.0`) |
| HEAD is past the tag | `app@v1.2.0+abc1234` | `<repo>/tree/<sha>` |
| no release tag yet | `app@dev+abc1234` | `<repo>/tree/<sha>` |
| no git (a source tarball) | `app@dev` | none |

The docs entry links to the same ref plus the site's path
(`<repo>/tree/<ref>/docs/site`). Resolve with git at build time —
`git describe --tags --exact-match --match '<tag pattern>'` for the exact
tag, `git describe --tags --abbrev=0 --match '<pattern>'` for the last one,
`git rev-parse HEAD` for the sha — matching each part's own tag pattern
(`app-[0-9]*`) so one part's tag never labels another. Every lookup falls
back quietly; a build never fails for lack of git.

**Every other source link follows the same ref.** "View source" or "edit
on GitHub" links and links to files the docs cite use
`<repo>/blob/<ref>/<path>` with the footer's ref (tag, else commit), never
`blob/main`. A page built from a release must show that release's file, not
whatever `main` holds later. Fall back to the default branch only without
git.

Keep the resolution in one small pure function (git facts in, entries
out) with a unit test per row of the table, plus a render test that the
footer shows each entry and links it.

## CI

- **Full history.** `actions/checkout` with `fetch-depth: 0` in every job
  that builds the site, or `git describe` sees no tags.
- **Deploy after the release.** When a release workflow tags the default
  branch after the push (a bot's release commit and tag), a deploy
  triggered by the push builds before the tag exists and shows the
  previous version. Deploy on the release workflow's completion instead:

  ```yaml
  on:
    push: { branches: [main] }   # verify only
    workflow_run:
      workflows: [release]
      types: [completed]
      branches: [main]
  jobs:
    deploy:
      if: >-
        github.event_name == 'workflow_run' &&
        github.event.workflow_run.conclusion == 'success'
  ```

  A bot push made with `GITHUB_TOKEN` triggers no workflow, but
  `workflow_run` still fires when the release run ends. The run checks out
  the default branch's newest commit, the release commit included.
- **Accessibility.** The group gets a `title` (`Versions`); each link's
  text is its label, which needs no extra wording.
