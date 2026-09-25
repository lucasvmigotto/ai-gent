# Working on ai-gent

Skills and plugins for Claude Code and opencode, installed by symlink from
this repo (`setup.sh`, `install.sh`). The README covers installing and the
product pipeline; this file covers changing things.

## Before editing

- **Propose first.** For any change to a skill, plugin, hook or shared
  reference, present what will change and wait for approval before writing.
- Versioning follows `git:workflow` (`plugins/git/skills/workflow/SKILL.md`),
  and its guard hook is active here too.
- Run `scripts/check.sh` before every commit; CI runs the same script.

## Layout rules

- A plugin is `plugins/<name>/` with `.claude-plugin/plugin.json` (name,
  version, description, author, SPDX license) and `skills/<skill>/SKILL.md`.
  It is invoked as `/<name>:<skill>`, and in opencode as `<name>-<skill>`
  (a generated stub), so `<name>-<skill>` must be a valid opencode name:
  lowercase letters, digits and single hyphens.
- `name:` in a SKILL.md equals its directory name.
- **Descriptions** are single-line, at most 400 characters (every session
  loads them), with no `: ` or ` #` (they break the unquoted YAML value).
  Say what the skill does and produces, then the phrases that should
  trigger it. Details belong in the body.
- **Keep bodies lean.** Material only some runs need — a vendor's details,
  a worked example, a troubleshooting deep-dive — goes in
  `skills/<skill>/references/<topic>.md`, with a pointer in SKILL.md
  saying when to read it. Files several skills of one plugin share go in
  `plugins/<name>/references/`; files several plugins share go in
  `shared/` and are symlinked (relative links) into each plugin's
  `references/`.
- Refer to other skills as `plugin:skill` and to files with backticked
  relative paths (`../../references/pipeline.md`); `scripts/check.sh`
  verifies both resolve.
- `shared/pipeline.md` is the product pipeline's contract (artifact paths,
  owners, statuses). Change it deliberately and update every stage that
  depends on what changed.
- Hook scripts are POSIX `sh`, tested by `scripts/test-guard.sh`.

## Releases

Automated: every push to `main` runs `.github/workflows/release.yml`, which
runs the checks and then `scripts/release.py`. The commits since the last
tag decide: `feat` → minor; `fix`, `perf`, `refactor` → patch; `type!:` or
a `BREAKING CHANGE:` footer → major; anything else (docs, chore, ci, test,
style) → no release. A release bumps the `version` of each plugin whose
files (or the shared files it links) changed, promotes `CHANGELOG.md`'s
`## Unreleased` section (or generates one from the commits), commits
`chore(release): X.Y.Z`, tags `X.Y.Z` and publishes a GitHub Release.

- Tags are plain SemVer without a `v` (`1.1.0`); a `v` only in display text.
- Write release notes by hand under `## Unreleased` when the commit
  subjects aren't enough; they're used as they are.
- Don't bump plugin versions or tag by hand; a version already changed by
  hand since the last tag is left alone.
- Preview with `scripts/release.py --dry-run`.
- After pushing, the bot's release commit lands on `main`: run
  `git pull --ff-only` before the next local commit.

## Evals

`plugins/<name>/evals/` holds `claude plugin eval` suites: trigger cases
(does a request reach the right skill, and not a neighbor) and behavior
cases. Run one plugin's suite with
`claude plugin eval plugins/<name> --runs 1 --ablation none` (trigger
graders count only with the plugin loaded). Behavior cases that need a
shell (tag `behavior`, with a `scaffold.sh`) also need `--scaffold
--allow-tools Bash`, and the eval sandbox's dependencies (`bubblewrap`,
`socat`) installed. Eval runs cost real tokens and aren't part of CI.
