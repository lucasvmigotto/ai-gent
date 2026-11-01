# Working on ai-gent

Skills and plugins for Claude Code (the default) and, opt-in, opencode,
Codex, Cursor, Antigravity, GitHub Copilot, Amp, Kiro and Windsurf, installed
from this repo by symlink or generated entry files (`setup.sh`,
`install.sh`). The README covers installing and the product pipeline; this
file covers changing things.

## Before editing

- **Propose first.** For any change to a skill, plugin, hook or shared
  reference, present what will change and wait for approval before writing.
- Versioning follows `git:workflow` (`plugins/git/skills/workflow/SKILL.md`),
  and its guard hook is active here too.
- Run `scripts/check.sh` before every commit; CI runs the same script.
  `--quick` skips the installer tests, `--quiet` prints only failures and the
  final line (combine them inside a loop; run the full script once at the end).
- Before pushing a change to `plugins/repo/scripts/reporun.py` or
  `plugins/db/scripts/dbrun.py`, also run `scripts/test-reporun-github.py`
  or `scripts/test-dbrun-engines.py` (real GitHub repo / real engine
  containers; their headers say what they need). `test-reporun-github.py` is
  not part of CI; `test-dbrun-engines.py` runs weekly in `scheduled.yml`.

## Layout rules

- A plugin is `plugins/<name>/` with `.claude-plugin/plugin.json` (name,
  version, description, author, SPDX license) and `skills/<skill>/SKILL.md`.
  It is invoked as `/<name>:<skill>`, and in opencode and the other agents
  as `<name>-<skill>` (a generated stub), so `<name>-<skill>` must be a valid
  skill name everywhere: lowercase letters, digits and single hyphens.
  OpenCode V2 takes a skill's ID from its directory, not from `name:`, so the
  stub keeps the two equal.
- The agents `setup.sh` supports are listed in its `AGENT_IDS` registry;
  adding one means its label, detection probes, skills directory (with an
  env override) and a case in `scripts/test-install.sh`. Prefer the shared
  `~/.agents/skills` when the agent reads it.
- OpenCode-native code lives in `opencode/` — `opencode/guard/` is the V2 guard
  plugin, its shared rules and the parity test — and `scripts/opencode-config.py`
  edits the OpenCode V2 configuration. Neither is a `plugins/<name>/` plugin.
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
  `references/`. The exception is code a plugin's hooks module imports:
  the engine refuses an import through a symlink that leaves the plugin,
  so such a file (`shared/guard-ux.ts`, `shared/hook-lines.ts`) is copied into
  each plugin's `hooks/`, and `scripts/consistency.py` fails if a copy differs
  (`--sync-shared-code` rewrites them). A hooks module's tests can import only
  code files, so data they need from `shared/` (the secret patterns, the
  fixture cases) is a generated `.ts` twin kept equal the same way.
- Refer to other skills as `plugin:skill` and to files with backticked
  relative paths (`../../references/pipeline.md`); `scripts/check.sh`
  verifies both resolve.
- The OpenCode guard rules (`opencode/guard/rules.mjs`) and the shell guards
  (`plugins/*/hooks/guard.sh`) must decide the same; `scripts/check.sh` runs
  their parity test. Change one, change the other.
- `shared/pipeline.md` is the product pipeline's contract (artifact paths,
  owners, statuses). Change it deliberately and update every stage that
  depends on what changed.
- `shared/secrets.md` is the contract for credentials: pass first
  (`ai-gent/<area>/<name>`), a plain-text file last, and only a runner
  ever reads a value. A skill that needs a secret follows it, and the
  `secrets` guard keeps values out of the agent's reach.
- Token tracking (`plugins/agents/scripts/token_report.py`, tag vocabulary `shared/token-tags.json`): see `shared/token-tracking.md`; adding a tag changes the JSON and `shared/token-tags.md` together.
- Hook scripts are POSIX `sh`, tested by `scripts/test-guard.sh` (git),
  `scripts/test-db-guard.sh`, `scripts/test-secrets-guard.sh`,
  `scripts/test-repo-guard.sh` and `scripts/test-containers-guard.sh`
  (the devcontainer guard and its SessionStart leftovers report).
- Container resources a skill creates carry the project's ownership label and go by ID or label, never a prune: `shared/containers.md` §9a.

## Releases

Automated on every push to `main` (`scripts/release.py`); commit types decide
the bump. Full flow: `shared/releasing.md`.

- Tags are plain SemVer without a `v` (`1.1.0`).
- Don't bump plugin versions or tag by hand.
- Notes written under `## Unreleased` are used as they are.
- Preview with `scripts/release.py --dry-run`.
- The release ships a package (`scripts/package.py`, an allowlist): a file
  the installed skills or hooks need at runtime must be added to it, and
  `scripts/package.py --check` (part of `check.sh`) must stay green.
  Details: `shared/releasing.md`.
- After pushing, run `git pull --ff-only` before the next local commit (the bot's release commit).

## Evals

`plugins/<name>/evals/` holds `claude plugin eval` suites: trigger cases
(does a request reach the right skill, and not a neighbor) and behavior
cases. Run one plugin's suite with
`claude plugin eval plugins/<name> --runs 1 --ablation none` (trigger
graders count only with the plugin loaded). Behavior cases that need a
shell (tag `behavior`, with a `scaffold.sh`) also need `--scaffold
--allow-tools Bash`, and the eval sandbox's dependencies (`bubblewrap`,
`socat`) installed. `scripts/run-evals.py [plugin…] [--behavior]` runs the suites for you,
prints a table and records the scores in `docs/product/evals.md`.
Eval runs cost real tokens and aren't part of CI.

## Docs site

`docs/site/` (Bun · React · Vite): commands, generated content, i18n, deploy,
e2e and OG image rules are in `docs/site/README.md`.
