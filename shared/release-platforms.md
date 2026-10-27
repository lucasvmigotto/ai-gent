# The release job, per platform

Shared by the `git` and `devsecops` plugins. `releasing.md` says what a
release is and that the pipeline cuts it by default; this file holds the
two things a project copies in — **the script** and **the release job for
its CI platform** — so no skill carries its own copy.

Read it when a project gets its pipeline (`devsecops:pipeline`), when a
pipeline moves platforms (`devsecops:migrate`), and when a release job
fails (`devsecops:watch`).

## What every new project gets

| File | Where it comes from |
|---|---|
| `scripts/release.py` | **one version** (the usual case): `release-single.py` next to this file, copied as is. **Several versioned packages** (a plugin marketplace, a monorepo with per-package versions): ai-gent's own `scripts/release.py` (`https://github.com/lucasvmigotto/ai-gent/blob/main/scripts/release.py`), which bumps each package whose files changed and follows symlinks, plus a `release.json` listing extra manifests. |
| `CHANGELOG.md` | `# Changelog`, then an empty `## Unreleased`. Hand-written notes go there; the release promotes them. |
| a release job | the platform's section below, in the pipeline that runs on pushes to the default branch |

The single-version template finds its version source by itself
(`pyproject.toml`, `package.json`, `Cargo.toml` or `VERSION`); anything
else — `src/app/__init__.py`, a `.csproj` — is named in `release.json`:
`{"version_file": "src/app/__init__.py"}`. It has one rule beyond the
flow in `releasing.md`: the source must equal the last tag, or it stops
with exit 1 (somebody bumped by hand — move the tag or the file). Keep
the CLI, the exit codes and the CHANGELOG format identical across a
project's repositories; test the bump rules in the project's own test
suite (ai-gent's `scripts/test-release-single.sh` is a model).

A lock file that records the version (`uv.lock`, `package-lock.json`,
`Cargo.lock`) is re-locked **after** `release.py` and committed with it —
the `<relock>` line in the snippets below. Delete the line when there is
none.

The default branch is whatever the project's `git:workflow` chain calls
the top one (`main`/`master`). Work lands there through the promotion; the
release job reads what arrived.

## The contract each platform must meet

Every rendering below does the same nine things; a platform that cannot
do one says so in `delivery.md` and names the workaround.

1. **Trigger** on pushes to the default branch, after the checks pass
   (the same suite humans and PRs run, unskippable by path filters).
2. **Loop guard** — never release for the bot's own `chore(release):`
   commit. Prefer a condition on the head commit's message.
3. **Never `[skip ci]` / `-o ci.skip` in the release commit.** Most
   platforms read it for the tag's pipeline too, and the tag must run its
   own pipeline (package, publish, deploy). The loop guard replaces it.
4. **Full history and tags** in the checkout: `fetch-depth: 0` or the
   equivalent, tags included. A shallow clone sees no previous tag and
   plans a first release.
5. **Plan, then apply.** `release.py --dry-run` first; exit `3` means no
   release — the job ends green and does nothing; any other non-zero
   code fails the job.
6. **Commit and tag as the CI identity**: `chore(release): X.Y.Z`, then
   an annotated tag `X.Y.Z`, then one push of branch and tag (atomic where
   Git allows), so a branch that moved is rejected, not merged.
7. **Serialise**: one release at a time per repository; a queued run
   re-plans against the new head.
8. **Fail clearly on missing permissions.** The push is where an
   ungranted permission surfaces; the job prints which permission and where
   a human grants it (the table below) instead of a bare `403`.

9. **Back-merge** into the integration branch (`dev`/`develop`), when
   the project has one. The release commit and tag land on the default
   branch only; unless they come back, the integration branch's version
   file and CHANGELOG lag behind and `release.py` run there counts from an
   older tag. After the push of step 6, in the same job: fetch the
   integration branch, check it out from `origin/<integ>`, run
   `git merge --no-ff -m "chore(release): merge X.Y.Z back into <integ>" X.Y.Z`
   and push it. A conflict or a refused push **warns** with the manual
   command (`git switch <integ> && git merge --no-ff X.Y.Z && git push`)
   and leaves the job **succeeded**: the release is already done, so a
   warning (not a silent pass, not a failure) is the right outcome. The
   merge message starts with `chore(release):`, so the loop guard already
   covers it. The identity that pushed the default branch pushes this too,
   and needs the same permission on the integration branch; each platform
   section says which.

### At a glance

| | GitHub Actions | Azure Pipelines | GitLab CI | Bitbucket | Forgejo / Gitea | Jenkins |
|---|---|---|---|---|---|---|
| Push identity | `GITHUB_TOKEN` (`contents: write`) | `System.AccessToken` (Build Service) | project or group access token (`write_repository`), or `CI_JOB_TOKEN` where the instance allows push | repository access token (write) | job token, or a bot user's token | credentials of a bot user or deploy key |
| Loop guard | `startsWith(head_commit.message, 'chore(release):')` | `Build.SourceVersionMessage` | `$CI_COMMIT_MESSAGE` rule | first step checks `git log -1` (no message condition) | `github.event.head_commit.message` | `git log -1` check in the stage |
| Does the CI-pushed tag start the tag pipeline? | **No** with `GITHUB_TOKEN` (use a GitHub App/PAT token, or release in the same run) | **Yes** | **Yes** (token user) | **Yes** | verify on the instance version | only with a webhook and tag discovery |
| Release object | `gh release create` | none (tag; notes in the pipeline summary or a feed package) | `release:` keyword | none (tag only) | Releases REST API | none native (call the SCM's API) |
| One-time grant | `contents: write` on the job; the ruleset lets the bot bypass | Contribute, Create tag, Bypass policies for the Build Service | token role Maintainer; protected branch and protected tags allow it | token user may bypass branch restrictions | bot user whitelisted on the protected branch | bot credential with push rights |

## GitHub Actions

The reference implementation is ai-gent's `.github/workflows/release.yml`.

```yaml
name: release
on:
  push:
    branches: [main]
  workflow_dispatch:
permissions:
  contents: read
concurrency:
  group: release
  cancel-in-progress: false
jobs:
  check:
    if: ${{ !startsWith(github.event.head_commit.message, 'chore(release):') }}
    uses: ./.github/workflows/check.yml        # the suite humans and PRs run
  release:
    needs: check
    if: ${{ github.ref == 'refs/heads/main' }}
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    permissions:
      contents: write                            # push the commit and tag, create the release
    steps:
      - uses: actions/checkout@<sha> # vX.Y.Z
        with:
          fetch-depth: 0
          fetch-tags: true
          persist-credentials: true              # the push below uses the checkout's token
      - id: plan
        run: |
          set +e; python3 scripts/release.py --dry-run; code=$?; set -e
          case "$code" in
            0) echo "release=true" >>"$GITHUB_OUTPUT" ;;
            3) echo "release=false" >>"$GITHUB_OUTPUT"; echo "No release needed." ;;
            *) exit "$code" ;;
          esac
      - if: ${{ steps.plan.outputs.release == 'true' }}
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "41898496+github-actions[bot]@users.noreply.github.com"
          version="$(python3 scripts/release.py)"
          python3 scripts/release.py --notes "$version" >"$RUNNER_TEMP/notes.md"
          <relock>
          git add CHANGELOG.md <version files> <lock file>
          git commit -m "chore(release): $version"
          git tag -a "$version" -m "$version"
          git push --atomic origin HEAD:refs/heads/main "refs/tags/$version"
          gh release create "$version" --title "$version" --notes-file "$RUNNER_TEMP/notes.md" --verify-tag
          integ=dev                                        # the integration branch; skip when the project has none
          if git ls-remote --exit-code --heads origin "$integ" >/dev/null 2>&1; then
            git fetch -q origin "$integ"
            git switch -q -C "$integ" "origin/$integ"
            if ! { git merge --no-ff -m "chore(release): merge $version back into $integ" "$version" && git push origin "$integ"; }; then
              git merge --abort 2>/dev/null || true
              echo "::warning::back-merge of $version into $integ failed; run: git switch $integ && git merge --no-ff $version && git push"
            fi
          fi
```

- **Loop guard.** Pushes made with `GITHUB_TOKEN` start no workflow, so
  the release commit cannot re-enter; the `chore(release)` condition is
  the second guard (and is the only one when a PAT or App token pushes).
- **Tag pipeline.** Because that same property applies to the tag, a
  workflow `on: push: tags` does **not** run for it. Either do the tag's
  work (package, publish, image retag) in the same workflow after
  `release` with `needs: release`, or push with a GitHub App installation
  token (`actions/create-github-app-token`, pinned) instead of
  `GITHUB_TOKEN` — then the tag workflow runs and the `chore(release)`
  guard is the only loop guard, so keep it.
- **Back-merge.** The same `GITHUB_TOKEN` pushes the integration branch
  (`contents: write`); if `dev` is protected, add `github-actions` to its
  bypass list too, or the step only warns. The merge push starts no
  workflow, so `dev` gets no CI run for it.
- **Human grants, once.** If `main` is protected by a ruleset or branch
  protection that requires pull requests, add `github-actions` (or the
  App) to the bypass list; allow the workflow write permission under
  Settings → Actions → General if the org default is read-only.
- Lint with `actionlint` and `zizmor`; the `Release` step touches no
  untrusted input (the version comes from the script).

## Azure Pipelines

The suite standard (Release stage on `main`, `System.AccessToken`).

```yaml
trigger:
  branches: { include: [main] }
  tags: { include: ['*.*.*'] }                   # the release's tag runs its own pipeline

stages:
  - stage: Test
    jobs: [ ... ]                                # runs on every push
  - stage: Release
    dependsOn: Test
    condition: |
      and(succeeded(),
          eq(variables['Build.SourceBranch'], 'refs/heads/main'),
          not(startsWith(variables['Build.SourceVersionMessage'], 'chore(release):')))
    jobs:
      - job: release
        steps:
          - checkout: self
            fetchDepth: 0
            fetchTags: true
            persistCredentials: true             # System.AccessToken for the push
          - script: |
              set -eu
              git config user.name "Azure Pipelines release"
              git config user.email "azure-pipelines-release@users.noreply.invalid"
              git switch -C main
              set +e; python3 scripts/release.py --dry-run; code=$?; set -e
              [ "$code" = 3 ] && { echo "No release needed."; exit 0; }
              [ "$code" = 0 ] || exit "$code"
              version="$(python3 scripts/release.py)"
              <relock>
              git add CHANGELOG.md <version files> <lock file>
              git commit -m "chore(release): $version"
              git tag -a "$version" -m "$version"
              git push origin HEAD:refs/heads/main "refs/tags/$version" || {
                echo "##vso[task.logissue type=error]push rejected: grant the Build Service Contribute, Create tag and (if main has policies) Bypass policies on this repository" >&2
                exit 1; }
              integ=dev                                        # the integration branch; skip when the project has none
              if git ls-remote --exit-code --heads origin "$integ" >/dev/null 2>&1; then
                git fetch -q origin "$integ"
                git switch -q -C "$integ" "origin/$integ"
                if ! { git merge --no-ff -m "chore(release): merge $version back into $integ" "$version" && git push origin "$integ"; }; then
                  git merge --abort 2>/dev/null || true
                  echo "##vso[task.logissue type=warning]back-merge of $version into $integ failed; run: git switch $integ && git merge --no-ff $version && git push"
                fi
              fi
            displayName: Plan and release
  # on refs/tags/*.*.*: Package -> Publish stages, gated on the tag equalling the version
```

- **Loop guard.** The Release stage condition on
  `Build.SourceVersionMessage` (the head commit's message). The
  `chore(release)` commit still builds and tests; it just does not release.
- **Never `[skip ci]`.** Azure would skip the tag's build as well.
- **Tag pipeline.** A tag pushed with `System.AccessToken` starts a build
  for the tag trigger. Check in the tag build that the tag equals the
  version source before packaging.
- **Back-merge.** The same Build Service identity pushes the integration
  branch, so it also needs **Contribute** there and, if that branch has
  policies, **Bypass policies when pushing**. A failure is a warning
  (`task.logissue type=warning`), and the stage stays green.
- **Human grants, once per repository.** Project settings → Repositories →
  the repo → Security, for "`<Project>` Build Service (`<org>`)":
  **Contribute: Allow**, **Create tag: Allow**, and **Bypass policies when
  pushing: Allow** (only if `main` has branch policies).
- Azure has no Release object; "Releases" are legacy deployment pipelines.
  Notes go in the tag build's summary or the published package.

## GitLab CI

```yaml
stages: [check, release]

release-commit:
  stage: release
  image: <pinned image with python3 and git>
  rules:
    - if: >-
        $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH &&
        $CI_COMMIT_MESSAGE !~ /^chore\(release\):/
  resource_group: release                        # one at a time
  variables:
    GIT_DEPTH: "0"                               # full history; tags come with it
  script:
    - set +e; python3 scripts/release.py --dry-run; code=$?; set -e
    - '[ "$code" = 3 ] && { echo "No release needed."; exit 0; }'
    - '[ "$code" = 0 ] || exit "$code"'
    - git config user.name "release bot"
    - git config user.email "release-bot@users.noreply.invalid"
    - version="$(python3 scripts/release.py)"
    - <relock>
    - git add CHANGELOG.md <version files> <lock file>
    - git commit -m "chore(release): $version"
    - git tag -a "$version" -m "$version"
    - git push "https://release-bot:${RELEASE_TOKEN}@${CI_SERVER_HOST}/${CI_PROJECT_PATH}.git" HEAD:"$CI_DEFAULT_BRANCH" "refs/tags/$version"
    - |
      integ=dev                                        # the integration branch; skip when the project has none
      if git ls-remote --exit-code --heads origin "$integ" >/dev/null 2>&1; then
        git fetch -q origin "$integ"
        git switch -q -C "$integ" "origin/$integ"
        if ! { git merge --no-ff -m "chore(release): merge $version back into $integ" "$version" && git push "https://release-bot:${RELEASE_TOKEN}@${CI_SERVER_HOST}/${CI_PROJECT_PATH}.git" "$integ"; }; then
          git merge --abort 2>/dev/null || true
          echo "WARNING: back-merge of $version into $integ failed; run: git switch $integ && git merge --no-ff $version && git push"
        fi
      fi

release-publish:                                 # runs in the tag's pipeline
  stage: release
  image: registry.gitlab.com/gitlab-org/cli:<pinned>
  rules:
    - if: $CI_COMMIT_TAG =~ /^\d+\.\d+\.\d+$/
  script:
    - python3 scripts/release.py --notes "$CI_COMMIT_TAG" > notes.md
  release:
    tag_name: $CI_COMMIT_TAG
    description: ./notes.md
```

(The notes step needs `python3`; use an image that has both, or
`artifacts:` a `notes.md` from a job that does.)

- **Push identity.** `CI_JOB_TOKEN` can push only where the instance
  version and the project's token settings allow it — verify; otherwise
  create a **project access token** (role Maintainer, scope
  `write_repository`) and store it as a masked, protected CI/CD variable
  `RELEASE_TOKEN` (`secrets:pass` names it `ai-gent/gitlab/<project>-release`).
- **Back-merge.** The same `RELEASE_TOKEN` pushes the integration branch;
  add its user to that branch's "Allowed to push" if it is protected.
  Failure prints a warning and the job stays green.
- **Loop guard.** The `rules:` regex on `$CI_COMMIT_MESSAGE`.
- **`[skip ci]` caveat.** It, and `git push -o ci.skip`, skip the pipeline
  of the pushed commit and of a tag on it — never use them here.
- **Tag pipeline.** A tag pushed with an access token starts a tag
  pipeline (`$CI_COMMIT_TAG` set); `release-publish` and any package/
  publish jobs hang off it. Pushes with `CI_JOB_TOKEN` follow the
  instance's rules for triggering pipelines — verify.
- **Human grants, once.** The token's user (or role) must be allowed to
  push to the protected default branch (Settings → Repository → Protected
  branches → Allowed to push) and to create the protected tags (Protected
  tags → Allowed to create). Protected variable needs the branch/tags
  protected too.

## Bitbucket Pipelines

```yaml
pipelines:
  branches:
    main:
      - step:
          name: check
          script: [ <the suite> ]
      - step:
          name: release
          script:
            - |
              if git log -1 --format=%s | grep -q '^chore(release):'; then
                echo "Release commit, nothing to do."; exit 0
              fi
            - git fetch --tags --unshallow || git fetch --tags
            - set +e; python3 scripts/release.py --dry-run; code=$?; set -e
            - '[ "$code" = 3 ] && { echo "No release needed."; exit 0; }'
            - '[ "$code" = 0 ] || exit "$code"'
            - git config user.name "release bot"
            - git config user.email "release-bot@users.noreply.invalid"
            - version="$(python3 scripts/release.py)"
            - <relock>
            - git add CHANGELOG.md <version files> <lock file>
            - git commit -m "chore(release): $version"
            - git tag -a "$version" -m "$version"
            - git push "https://x-token-auth:${RELEASE_TOKEN}@bitbucket.org/${BITBUCKET_REPO_FULL_NAME}.git" HEAD:main "refs/tags/$version"
            - |
              integ=dev                                        # the integration branch; skip when the project has none
              if git ls-remote --exit-code --heads origin "$integ" >/dev/null 2>&1; then
                git fetch -q origin "$integ"
                git switch -q -C "$integ" "origin/$integ"
                if ! { git merge --no-ff -m "chore(release): merge $version back into $integ" "$version" && git push "https://x-token-auth:${RELEASE_TOKEN}@bitbucket.org/${BITBUCKET_REPO_FULL_NAME}.git" "$integ"; }; then
                  git merge --abort 2>/dev/null || true
                  echo "WARNING: back-merge of $version into $integ failed; run: git switch $integ && git merge --no-ff $version && git push"
                fi
              fi
  tags:
    '*.*.*':
      - step:
          name: package and publish
          script: [ <package>, <publish> ]
```

- **Push identity.** A **repository access token** with repository write,
  stored as a secured repository variable `RELEASE_TOKEN`.
- **Loop guard.** Bitbucket has no commit-message condition for steps, so
  the first lines of the step check the head subject and end green.
  Skipping with `[skip ci]` would also skip the tag's pipeline — avoid it.
- **Tag pipeline.** The `tags:` pipeline runs for the tag the token
  pushes (any push does). No Release object: the tag is the release;
  attach files through the repository Downloads API if needed.
- **Back-merge.** The same repository access token pushes the integration
  branch; if branch restrictions protect it, let the token's user push
  there too. Failure prints a warning and the step stays green.
- **Human grants, once.** If branch restrictions protect `main`, let the
  token's user push to it (and bypass the merge-check requirement);
  create the secured variable.

## Forgejo / Gitea Actions

Start from the GitHub Actions job: same workflow syntax, under
`.forgejo/workflows/` or `.gitea/workflows/`. Differences:

```yaml
      - name: Publish the release object
        env:
          TOKEN: ${{ secrets.RELEASE_TOKEN }}     # a bot user's token with repo write
        run: |
          curl -fsS -X POST "${GITHUB_SERVER_URL}/api/v1/repos/${GITHUB_REPOSITORY}/releases" \
            -H "Authorization: token ${TOKEN}" -H "Content-Type: application/json" \
            -d "$(python3 -c 'import json,sys; print(json.dumps({"tag_name": sys.argv[1], "name": sys.argv[1], "body": open(sys.argv[2]).read()}))' "$version" "$RUNNER_TEMP/notes.md")"
```

- **Back-merge.** After the release push, add the block from the GitHub
  section (same `git` commands; warn with `echo "::warning::…"`, which the
  runner shows as an annotation) in the same step, pushing with the same
  bot user's token. Whitelist that user on the integration branch too, or
  the step only warns.
- **Push identity.** Push with a bot user's token (stored as the secret
  `RELEASE_TOKEN`) over HTTPS rather than the job token, so branch
  protection can whitelist exactly that user and the tag's pipeline is
  predictable.
- **Loop guard.** The same `startsWith(github.event.head_commit.message,
  'chore(release):')` condition; verify the context is populated on the
  instance version, else check `git log -1` in the job.
- **Tag pipeline.** A tag pushed by a real user token starts workflows
  `on: push: tags`; verify for the job token on the instance version.
  Do the tag's work in the same workflow if in doubt.
- **Release object.** The Releases REST API call above (no CLI assumed).
- **Human grants, once.** Whitelist the bot user for pushing to the
  protected default branch and for tag creation; create the secret.
- Check `concurrency:` support on the instance; without it, serialise by
  repository setting (one runner for the release label).

## Jenkins

Multibranch Pipeline with the default branch building. In the
`Jenkinsfile`:

```groovy
stage('release') {
  when {
    branch 'main'
    not { expression { sh(returnStdout: true, script: 'git log -1 --format=%s').startsWith('chore(release):') } }
  }
  options { lock('release') }                    // Lockable Resources plugin
  steps {
    withCredentials([gitUsernamePassword(credentialsId: 'release-bot')]) {
      sh '''
        set -eu
        git fetch --tags --unshallow || git fetch --tags
        code=0; python3 scripts/release.py --dry-run || code=$?
        [ "$code" = 3 ] && { echo "No release needed."; exit 0; }
        [ "$code" = 0 ] || exit "$code"
        git config user.name "release bot"; git config user.email "release-bot@users.noreply.invalid"
        version="$(python3 scripts/release.py)"
        <relock>
        git add CHANGELOG.md <version files> <lock file>
        git commit -m "chore(release): $version"
        git tag -a "$version" -m "$version"
        git push origin HEAD:main "refs/tags/$version"
        integ=dev                                        # the integration branch; skip when the project has none
        if git ls-remote --exit-code --heads origin "$integ" >/dev/null 2>&1; then
          git fetch -q origin "$integ"
          git switch -q -C "$integ" "origin/$integ"
          if ! { git merge --no-ff -m "chore(release): merge $version back into $integ" "$version" && git push origin "$integ"; }; then
            git merge --abort 2>/dev/null || true
            echo "WARNING: back-merge of $version into $integ failed; run: git switch $integ && git merge --no-ff $version && git push"
          fi
        fi
      '''
    }
  }
}
// a later stage: when { buildingTag() } -> package and publish
```

- **Push identity.** A credential for a bot user (or a deploy key with
  write access) in the Jenkins credential store; the checkout step's
  read-only credential is not enough.
- **Loop guard.** The `when` expression on the head subject. The
  `changelog` directive matches any commit in the build, not the head
  alone — prefer the explicit check.
- **Tag pipeline.** Jenkins builds a tag only when the Multibranch job
  has *Discover tags* and receives a webhook (or the scan interval finds
  it). Without it, do the tag's work in the same pipeline after the push
  (`when { buildingTag() }` stages never run for it).
- **Back-merge.** The same bot credential pushes the integration branch;
  grant it push there too. A failure prints a warning (use
  `unstable('…')` in a `script` step if the build should show yellow); the
  build stays green.
- **Release object.** Jenkins has none; call the SCM's release API from
  the pipeline (GitHub, GitLab or Forgejo section above) or stop at the
  tag.
- **Human grants, once.** Create the credential; allow its user to push
  to the protected branch and create tags on the SCM; enable *Discover
  tags* and the webhook if a tag pipeline is wanted.
- `lock('release')` (Lockable Resources) serialises releases.

## Manual fallback: `scripts/release.sh`

When CI cannot push (protected branch with no bypass, a platform with no
permission model for a bot, an air-gapped runner), a person cuts the
release from the same script. This is the fallback, never the default;
the pipeline stays the way releases are normally made, and a repository
that cannot grant the push says so in `delivery.md`.

```bash
#!/usr/bin/env bash
# Cut a release by hand: plan, commit, tag, push. Run on the default branch.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
branch=main
[ "$(git branch --show-current)" = "$branch" ] || { echo "run on $branch" >&2; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "working tree is not clean" >&2; exit 1; }
git fetch -q origin "$branch" --tags
[ "$(git rev-parse HEAD)" = "$(git rev-parse "origin/$branch")" ] || { echo "HEAD is not origin/$branch" >&2; exit 1; }
scripts/check.sh                                  # the project's own suite
set +e; python3 scripts/release.py --dry-run; code=$?; set -e
[ "$code" = 3 ] && { echo "nothing releasable since the last tag" >&2; exit 3; }
[ "$code" = 0 ] || exit "$code"
read -r -p "Release this? [y/N] " answer; [[ "$answer" =~ ^[Yy]$ ]] || exit 1
version="$(python3 scripts/release.py)"
<relock>
git add CHANGELOG.md <version files> <lock file>
git commit -q -m "chore(release): $version"
git tag -a "$version" -m "$version"
git push --atomic origin "HEAD:refs/heads/$branch" "refs/tags/$version"
integ=dev                                         # skip when there is no integration branch
if git ls-remote --exit-code --heads origin "$integ" >/dev/null 2>&1; then
  git fetch -q origin "$integ"; git switch -q -C "$integ" "origin/$integ"
  git merge --no-ff -m "chore(release): merge $version back into $integ" "$version" && git push origin "$integ" \
    || { git merge --abort 2>/dev/null; echo "back-merge failed; run: git switch $integ && git merge --no-ff $version && git push" >&2; }
  git switch -q "$branch"
fi
```

It pushes, so `git:workflow` applies: only on an explicit request. The
tag it pushes starts the tag pipeline like any human-pushed tag.
