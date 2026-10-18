# ai-gent — improvement plan

Status: Draft · 2026-10-18 · from a review of the repository, the
introspec report (`docs/product/introspec.md`) and this cycle's work.

ai-gent is in good shape: every skill has trigger evals, the guards are
parity-tested, releases are automated and land through PRs, and the docs
site and the runners have reconstructed specs tracked on GitHub. What it
lacks is mostly **evidence that skills behave** (not only that they're
reached), **lean bodies** for its largest skills, and a few loops left
open in CI and in the tracker. This plan closes them in five phases.

Each item says why, what, when it's done, its size (S ≤ half a day, M ≤
two days, L more) and what it waits on. Items in the same phase marked
**∥** can run side by side (`agents:parallel`); each lands through its own
PR (`git:workflow`, remote mode) and the phase ends with one promotion and
release.

## Phase 0 — finish what's decided

Answers already given (brief decision log, 2026-10-18); nothing to decide.

| # | Item | Done when | Size |
|---|---|---|---|
| 0.1 ∥ | **Pin dbrun's client image and drivers** (R3): the `uv` image by digest; `psycopg`, `pymysql`, `pymssql`, `oracledb` to exact versions; a comment saying how to bump them. They receive database credentials. | `dbrun.py` names no floating tag or range; `scripts/test-dbrun-engines.py` passes on all five engines | S |
| 0.2 ∥ | **macOS support in the installer** (R8): `setup.sh` checks for bash ≥ 4 and re-executes under a newer one (Homebrew's) or says how to install it; replace the GNU-only spots (7 found: `sed -i`, `readlink -f`, `find -printf`, `stat -c`…); `install.sh` checks the same. A `macos-latest` job runs `scripts/test-install.sh`. | `test-install.sh` passes on Ubuntu and macOS runners | M |
| 0.3 ∥ | **Shared uninstall** (assumption 8, corrected): uninstalling one of Codex, Cursor, Gemini, Copilot, Amp keeps the `~/.agents/skills` entries while another selected agent still uses them. Also `setup.sh --target` with no value fails with a message (inferred item 6). | a `test-install.sh` case for each | S |

## Phase 1 — evidence that skills behave

Today all 52 eval cases are **trigger** cases. Nothing checks that a skill,
once loaded, follows its rules.

| # | Item | Done when | Size | Waits on |
|---|---|---|---|---|
| 1.1 | **Behavior evals for the rules that matter most** — each a sandboxed case with a `scaffold.sh` and graders on the tool calls: (a) `git:workflow` lands through a PR in remote mode and locally otherwise, with the count rule; (b) `db:*` never writes to a remote profile and goes through `dbrun`; (c) `secrets:pass` never reads a value; (d) `project:init` asks before writing — at most four questions per prompt, as options; (e) `devsecops:watch` hands back a fix instead of editing; (f) `repo:sync` creates items in one batch and records them; (g) `agents:parallel` refuses tracks that share a hot spot; (h) `project:spec`'s clarify re-runs until markers are answered or delegated. | 8 behavior cases pass with `--scaffold --allow-tools Bash`; their cost is recorded | L | — |
| 1.2 | **One command to run the evals** — `scripts/run-evals.sh [plugin…] [--behavior]`: runs the suites, prints a table, and appends the scores and cost to `docs/product/evals.md` (what `project:pitch` already reads). Manual or monthly; never in CI (cost). | the full suite runs with one command; the history file shows the last run | S | 1.1 |
| 1.3 ∥ | **Verified for tooling** — pipeline rule 6 defines Verified through e2e suites, which tooling doesn't have, so ai-gent's 59 tracker issues can never close. Add: a feature without a UI is Verified when its integration suites run in CI and pass (installer: `test-install.sh`; runners: their unit suites plus, for dbrun, the scheduled engine suite of 3.4). | rule 6 says so; `project:status` and `specsync` treat it the same; the features whose suites run in CI move to Verified and their issues close through `repo:sync` | M | 3.4 for dbrun |
| 1.4 ∥ | **`reporun` end to end** — `scripts/test-reporun-github.py`, like the engine suite: against a throwaway repository it opens, checks and merges a PR (rebase and merge commit), creates, updates and closes issues with sub-issues, and deletes the repository at the end. Manual; needs `gh` signed in. | one run passes; CLAUDE.md lists it next to the engine suite | M | — |
| 1.5 ∥ | **Docs site: one description per page** — the static pages (install, flow, contribute, reference) render both the HTML and the Markdown/`llms.txt` from one typed page description, so they can't drift (introspec R9, item 3). Tests cover the LLM output and every page. | `generate-llms.ts` no longer repeats page content; tests fail if a page section is missing from its Markdown | M | — |

## Phase 2 — lean, consistent skills

| # | Item | Done when | Size |
|---|---|---|---|
| 2.1 ∥ | **Slim the largest skills** as `git:workflow` was (23 → 11 KB): `devcontainer:setup` (27 KB), `devcontainer:infra` (20 KB), `devcontainer:workflow` (20 KB), `project:docs` (15 KB), `project:architecture` (12 KB). Rules and decisions stay in the body; worked examples, vendor details and long tables move to `references/`, each with a pointer saying when to read it. Contradictions found on the way are fixed, not moved. | each body at most ~10 KB; their trigger evals still pass | L |
| 2.2 | **Review `shared/pipeline.md`** (299 lines, 11 rules, linked into most plugins): one consistent voice, long passages (Spec Kit, tracker, remote mode) as short rules pointing to their own references, and every stage it names checked to say the same. | each rule fits on one screen; no stage contradicts it | M |
| 2.3 ∥ | **Consistency checks in `check.sh`** — (a) a skill name followed by an `*italic section*` (`git:workflow` → *Landing*) must match a heading in that skill; (b) a list of retired phrasings that must not come back (`3–5 questions`, `one question at a time`, `at most 5 questions`, hardcoded skill or plugin counts in copy); (c) the total description size printed, with a ceiling (today ~4.1k tokens). | `check.sh` fails on each of the three, with a test case | S |

## Phase 3 — CI/CD and operations

| # | Item | Done when | Size |
|---|---|---|---|
| 3.1 ∥ | **No more "cancelled" runs on every release** — Docs CI's push-to-main run is cancelled by the post-release deploy (shared `docs-${ref}` group). Give verify and deploy their own groups, and let a push to main only verify while the post-release `workflow_run` deploys. | a release shows only green runs | S |
| 3.2 | **Branch protection on `dev` and `main`** — required checks `check`, `verify`, `e2e`; no direct pushes except the release bot's commit on `main`; merge methods matching `landing.md` (merge commit and rebase on). Applied through the platform with the user's go-ahead (`devsecops:pipeline`, *Pull requests*). | a red PR can't merge; the release still pushes its commit | S |
| 3.3 ∥ | **Failure issues for ai-gent itself** (`failure-issues.md`): a failure job on `release.yml`, the post-release deploy and the scheduled run of 3.4 opens or updates one `ci-failure` issue per workflow. | a forced failure on a test branch opens one issue, a second updates it | S |
| 3.4 ∥ | **A weekly scheduled run** — the full `check.sh`, the dbrun engine suite (Ubuntu runners have the room), and the docs site's e2e against the live URL. | the schedule runs green; a failure opens an issue (3.3) | M |
| 3.5 ∥ | **Supply chain, dogfooded** (`devsecops:supply-chain`): Dependabot also for the docs site's Bun dependencies (today only GitHub Actions); the docs site SBOM built in CI instead of by hand; secret scanning and push protection on. | Dependabot opens PRs for the site; the SBOM is a CI artifact | S |

## Phase 4 — features (after phases 0–3)

| # | Item | Why | Size |
|---|---|---|---|
| 4.1 | **Forgejo/Gitea in `reporun`** — `tea` and the Gitea API (GitHub-shaped), for self-hosted teams. | the platforms table names Forgejo for CI but `repo:*` can't reach it | M |
| 4.2 | **The installer's agent picker, tested** — a fake `fzf` on `PATH` returns a selection; cases for pre-selection, none selected and the numbered prompt. | the interactive path is the only untested one in `setup.sh` | S |
| 4.3 | **Tracker states beyond open/closed on GitHub** — a GitHub Project (v2) status field (To do / In progress / In review / Done) kept in step by `repo:sync`, where the user wants a board. | GitHub issues only show open/closed; `review` is invisible today | M |
| 4.4 | **Spec Kit in ai-gent** — only when a feature is big enough (decided 2026-10-18); `project:spec` bootstraps it then. | — | — |

## Deliberately not planned

- **Team stores and Windows without pass** — dropped on 2026-10-17; the
  single personal store stays the model.
- **Rewriting TOML profile files in place** — Python's standard library
  can't write TOML; `dbrun` flags plain values instead.
- **Evals in CI** — they cost real tokens per run; 1.2 makes them one
  command, run on demand or monthly.

## Order and tracking

1. **Phase 0** — three parallel tracks, one release (2.10.x).
2. **Phase 1 and Phase 3 together** — 1.3, 1.4, 1.5 and 3.1, 3.3, 3.4, 3.5
   are independent; 1.1 → 1.2 in sequence; 3.2 needs a go-ahead. 1.3's
   dbrun part waits on 3.4. One release per landed group.
3. **Phase 2** — after phase 1, so the behavior evals of 1.1 guard the
   slimming of 2.1 and the review of 2.2.
4. **Phase 4** — as capacity allows.

Each item becomes one tracker issue (`repo:issues`) labelled with its
phase, so progress shows on GitHub next to the feature issues; this file
stays the plan of record and is updated as items land.
