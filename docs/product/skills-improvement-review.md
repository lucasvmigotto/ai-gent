# Skills and plugins: recap, analysis and proposed improvements

Status: Draft · 2026-11-14 · read-only review of `plugins/` on branch
`docs/skills-improvement-review`. Nothing was changed; every item below needs
the owner's approval first (AGENTS.md, "Propose first").

Method: sizes from `wc -c`, descriptions from the `description:` lines (counted
after the `description: ` prefix), hooks and scripts from directory listings,
CLI subcommands from `rust/crates/ai-gent/src/cli.rs`. I read in full only
`project:status`, and the "implement contract" and "Converge" sections of
`backend:build` and `frontend:build`; the rest was read by `grep` for rule
words and by section headings. Anything not read is marked "not verified".
Not run: `scripts/check.sh`, the evals, any test.

## 1. Recap

13 plugins, 50 skills, 335,502 bytes of SKILL.md (average 6,710). 18 skills are
over 8 KB (8,192 bytes) and hold 176,446 bytes, 52.6% of the total. 51 files
under `skills/*/references/`, 22 under `plugins/*/references/`. 81 eval cases
(`plugins/*/evals/*`), and every one of the 50 skills has a `fires-*` trigger
grader; behavior cases exist for about 14 skills (21 cases, counted by name).

Hooks listed are files in `plugins/<p>/hooks/`. "Mod" means a Claude Code
interface module (`register.tsx` and the `.ts`/`.tsx` panes, bands and toasts);
"guard" means `guard.sh` backed by `ai-gent guard`.

| Plugin | Skills (SKILL.md bytes) | Hooks | Scripts | Deterministic today / by judgement |
|---|---|---|---|---|
| agents | delegate 8,556 · orchestrate 10,044 · parallel 9,308 | mod only (cards, landing, ledger, tokens, worktrees) | `wt.py`, `token_report.py` | `ai-gent delegate *`, `wt`, `tokens` run the mechanics; what to split, the cards and the review are judgement |
| backend | build 10,761 · domain 6,859 · spec 8,985 | none | none | all judgement |
| compact | code 3,367 | none | none (the tool is the `ai-gent-compact` crate; how the skill finds it: not verified) | the compaction is deterministic and self-verified; choosing what to read is judgement |
| db | connect 3,479 · inspect 4,405 · investigate 4,492 · review 3,934 | guard, mod | `dbrun.py`, `passstore.py` | `dbrun` classifies, masks and audits; findings are judgement |
| devcontainer | infra 9,640 · proxy 4,737 · setup 11,365 · workflow 11,392 | guard, `leftovers.sh`, mod | none | containers guard and leftovers report; everything else judgement |
| devsecops | audit 4,227 · iac 7,696 · migrate 4,766 · pipeline 9,957 · supply-chain 6,183 · watch 8,127 | none | none | none. `watch` is fenced by `allowed-tools` and `model: haiku`, `context: fork` |
| frontend | build 11,126 · spec 8,580 · tui 5,266 · uiux 10,259 | none | none | all judgement |
| git | workflow 9,014 | guard, SessionStart update check, release and hygiene mod | none | guard enforces commit, push and merge rules; `ai-gent repo chain` finds the chain; merge style and message wording are judgement |
| project | architecture 9,981 · checkpoint 4,122 · docs 9,302 · init 10,025 · introspec 8,844 · pitch 3,994 · recap 5,477 · refactor 7,287 · retrofit 7,723 · spec 9,307 · status 7,003 · survey 5,554 | mod only (pipeline pane, next-stage toast, checkpoint, handoff, usage, workers, guards) | `checkpoint.py`, `repo_state.py`, `sessions.py` | scripts feed checkpoint, recap and survey; the pipeline pane computes artifacts and Next in TypeScript; `project:status` redoes this by prose |
| qa | e2e 6,232 · load 5,307 · review 3,704 · strategy 6,167 | none | none | all judgement |
| repo | issues 3,899 · pr 4,165 · sync 5,076 | guard, CI mod | `reporun.py`, `specsync.py`, `passstore.py` | `ai-gent repo`, `ai-gent spec` do the platform calls and the sync plan |
| secrets | pass 3,902 | guard, output warning, secret mask | none in the plugin | the guard keeps values out; the skill guides setup |
| security | blue 3,316 · purple 1,950 · red 3,942 · scope 2,698 | none | none | all judgement; the scope gate is an eval-tested rule, not a hook |

`ai-gent` subcommands today: `version`, `doctor`, `update`, `guard`, `tokens`,
`repo`, `spec` (`plan`, `record`, `features`), `delegate`, `wt`, `autonomy`.
There is no pipeline, tasks or lint command.

Three plugins carry nearly all the deterministic machinery (`agents`, `repo`,
`db`). Eight of 13 plugins (`backend`, `compact`, `devsecops`, `frontend`,
`qa`, `security`, and in practice `devcontainer`, `project`'s stage skills) have
no script behind their skills.

## 2. Analysis

Ranked inside each part by expected gain. IDs are used in the table of part 3.

### 2.1 Script instead of prose

**S1. Pipeline status is computed twice, once in TypeScript and once by prose.**
`project:status` (7,003 bytes) tells the model to check every artifact's
presence and staleness with `git log -1 --format=%cs` (`SKILL.md:23`), compare
each feature claim to evidence (`:40-47`), count `[UPSTREAM GAP]`,
`[NEEDS CLARIFICATION]` and `[ASSUMPTION]` markers with `file:line` (`:83-93`),
and apply the next-stage rules (`:99-118`). The project mod already does this
in `plugins/project/hooks/pipeline-model.ts` (354 lines, header: "follows
project:status §4 rules 1-3 for Next") and `pipeline.ts` (280 lines), but only
inside the Claude Code mod; other agents and a plain session get the prose.
Fix: `ai-gent pipeline status --json` returning
`{artifacts:[{path,state,stale_since}], features:[{id,priority,deps,frontend,backend,qa,claims:[{claimed,evidence,ok}]}], markers:{gaps:[{file,line,text}],clarifications:[],assumptions:[]}, next:{stage,why,alternatives:[]}}`.
The skill then reads the JSON and writes the report; the TS mod can call the
same command or keep a golden-file test against it (`pipeline-golden.ts`
exists). Gain: the 4 numbered steps (about 3 KB) shrink to a few lines, and
the result stops varying between runs. The same command serves the next two.

**S2. "Implemented" and tick-scope checks in the build skills.** `backend:build`
and `frontend:build` ask the model to see that no `## Convergence` phase has
unchecked items (`backend/skills/build/SKILL.md:169`,
`frontend/skills/build/SKILL.md:175`), to tick only its own section
(`backend :58-60`, `frontend :75-77`), and to gate on unchecked
`checklists/` items (both, "Checklist gate"). `project:status` repeats the
Convergence check (`:36-38`). Grep in `rust/crates` finds no Convergence
logic. Fix: `ai-gent spec check <NNN-feature> --side backend|frontend|qa --json`
returning `{checklists:{total,checked,unchecked}, side_tasks:{open,done},
convergence_open:N, other_sides_changed:bool, can_mark:"Implemented"|false}`.
It extends the existing `ai-gent spec` (which already parses `specs/README.md`
and `tasks.md` for `specsync`). One implementation, three skills.

**S3. Merge style is a count rule done by hand.** `git:workflow` says count
commits with `git log <parent>..<branch> --oneline | wc -l`, then 4 or more
means `--no-ff`, fewer means `--ff-only` (`SKILL.md:87-95`); `repo:pr` points
at the same rule (`:65`); `agents:parallel` repeats "commits, their count, the
merge type" (`:115`), as does `agents:orchestrate` (`:116`). No `ff-only` or
`no-ff` appears anywhere under `rust/crates/ai-gent-core/src/guard`, so the
guard does not enforce it either. Fix: `ai-gent repo merge-style <branch>
[--parent dev] --json` returning `{parent, commits, style:"no-ff"|"ff-only",
promotion:bool}`, reusing `repo/chain.rs`. Evals `behavior-merge-few` and
`behavior-merge-many` already exist to prove it.

**S4. Skills still call the Python runners that have Rust ports.** `parallel`
uses `python3 <plugin>/scripts/wt.py` 14 times (`:16` defines `WT`); `repo:sync`
12 times for `specsync`; `project:status:56` and `agents:delegate:23` call
`token_report.py budget`. The binary has the same subcommands (`wt` has plan,
add, list, remove, baseline, resources, finish; `tokens`, `repo`, `spec`
exist). Fix: one sentence per skill, "use `ai-gent wt …` when the binary is
installed, else the script" (a clone install has no binary). Whether the
Python path is kept on purpose as the clone fallback: not verified.

**S5. Smaller candidates (all not verified beyond the cited line).**
`project:survey:62` (commit count, pace, stale branches, drift between main
branches) and `project:recap:78` (compare `files_changed` with `git diff
--stat`) belong in the existing `repo_state.py`; verify it already covers them
before adding anything. `qa:review` and `db:review` ask for ranked findings in
a table with fixed columns (`qa/review:59`, `db/review:59`, `security/red:65`,
`devsecops/audit:67` all define the same Location/Severity/Evidence layout):
a shared findings template in `shared/` (see 2.3) beats a script.

### 2.2 Hook instead of reminder

Allowed events only: a `tool.call` via the guard binary, `prompt.submit`, or a
SessionStart `systemMessage`. No `classic.*`, `prompt.*`, `skill.prompt`,
`session.compact`, `settings.read`, `attribution.text` or doors-attachment
`session.append`. The guards run through `ai-gent guard` (PreToolUse), so none
of these proposals depends on an event that `cc-plugin-sec-default` passes
over. A new rule must land in the Rust rules, `guard.sh` and
`opencode/guard/rules.mjs` together, with fixtures in `shared/guard-fixtures/`
(AGENTS.md, layout rules).

**H1. CI mutations: `gh run rerun`, `gh run cancel`, `gh workflow run`.**
`devsecops:watch:133-137` says never re-run, dispatch or cancel a job that
deploys, publishes or tags without an explicit go-ahead. The skill itself is
fenced by `allowed-tools` (`Bash(gh run list *)`, `view`, `watch`), but the
main model that receives the `Hand back` is not, and the grep of
`guard/` finds no `rerun`, `cancel` or `workflow run`. Fix: a repo-guard rule
that returns `ask` for these three on any platform CLI (`gh`, `glab`, `az
pipelines`), `tool.call` / PreToolUse. Low false-positive cost: they are rare.

**H2. Status columns written without evidence.** `pipeline` rule 6 and the
build skills forbid marking a feature Implemented or Verified without the
evidence (see S2). A PreToolUse rule on `Edit`/`Write` of `specs/README.md`
whose new text adds `Implemented` or `Verified` could run S2's
`ai-gent spec check` and return `ask` with the failing line. The guard runs
shell-free in Rust, so it is feasible there; in the POSIX `guard.sh` path it
would need the binary, which the launcher already requires. Effort is higher
than H1; feasibility of reading `tasks.md` inside the guard: not verified.

**H3. Ticking another layer's tasks.** `backend:build:58-60` and
`frontend:build:75-77` forbid ticking `## Frontend`/`## Backend`/`## QA` tasks
of others. A PreToolUse rule on `Edit` of `specs/*/tasks.md` could ask when a
single edit ticks lines under two layer sections. The hook cannot know which
skill is active, so it can only catch the whole-file pass the skill also
forbids. Partial value; do S2 first.

**H4. SessionStart one-line pipeline hint.** `project:status` claims "at a
session start on a pipeline project" in its description, but nothing at
SessionStart runs. A SessionStart `systemMessage` with `ai-gent pipeline status
--brief` (one line: next stage, open clarifications) replaces the model
deciding to invoke the skill. SessionStart exists already in `git/hooks.json`
(update check), so the pattern is proven. Needs S1.

**H5. Containers: editing through `exec`.** `devcontainer:workflow:89,96,157`
forbids editing files by shelling into the container and says to use `Edit` on
the host; the containers guard (`guard/rules/containers.rs`, 1 line matching
`exec`) is aimed at prune and ownership. Whether it also catches
`exec ... sed -i` or `tee`: not verified. Check first, extend the rule only
if absent.

**Already enforced, leave as prose or skill-level:** secrets values (secrets
guard + output mask), raw database clients (db guard), commits and pushes
(git guard), platform writes (repo guard).

### 2.3 Token cost

**T1. Descriptions are loaded in every session.** Of 50 descriptions, 49 are
at least 300 characters, 41 at least 330, 22 at least 350, 4 at least 380, none
over the 400-character cap (so `scripts/check.sh` limit holds). Only
`frontend:build` is under 300. At about 340 characters each that is roughly
17 KB, about 4,000 tokens, loaded before any work (estimate, not measured). The
four longest are `secrets:pass`, `repo:pr`, `db:investigate`, `project:retrofit`.
Fix: trim the 22 skills at 350+ to about 280-300 by dropping the second or
third example phrase and the "with X, Y, Z" detail lists that the body repeats
(for example `repo:pr` lists "checks, diffs and review threads" in the
description and again in its intro). Gain about 1,000-1,300 tokens per session
(estimate). Risk: trigger drift, so rerun that plugin's trigger evals.

**T2. Build skills duplicate the "implement contract".** `backend:build:36-70`
(1,943 bytes) and `frontend:build:53-86` (1,960 bytes) are the same bullets
(resolve the feature, checklist gate, load order, phases, tests mandatory,
scope, ignore files, halting) with three words changed. The "Converge" section
is also near-identical (`backend :153-177`, `frontend :159-183`). Fix: one
`shared/implement-contract.md`, symlinked into `plugins/backend/references/`
and `plugins/frontend/references/`, plus a two-line per-skill delta (which
section, which ignore files). Gain about 1.7 KB x 2 per skill load, and one place
to fix. `project:status` also restates the Convergence rule, so S2 serves it.

**T3. Eighteen bodies are over 8 KB and have thin `references/`.** Largest, with
their `references/` count: `devcontainer:workflow` 11,392 (6 references),
`devcontainer:setup` 11,365 (4), `frontend:build` 11,126 (0), `backend:build`
10,761 (0), `frontend:uiux` 10,259 (0), `agents:orchestrate` 10,044 (3),
`project:init` 10,025 (2), `project:architecture` 9,981 (1), `devsecops:pipeline`
9,957 (1), `devcontainer:infra` 9,640 (6). Four of the six largest without
a `references/` folder are the best candidates: `frontend:build`,
`backend:build`, `frontend:uiux` (and `frontend:spec` 8,580, `backend:spec` 8,985,
`backend:domain` and `devsecops:iac` have none either). Move the per-phase
execution detail (`frontend:build:87-158`, 72 lines; `backend:build:71-152`,
82 lines) into `references/phases.md` read when starting a phase. Gain about
3-4 KB per build run before the first phase. Not verified: how much of that
block every run truly needs.

**T4. Shared plugin references exist but are underused.** Each of
`security/references/` (5 files), `db/references/` (3), `qa/references/` (2)
has a `principles` or `quality` file; `backend`, `frontend`, `project` have one
quality file each, `devsecops` two. The findings-table layout (T-5 above)
is restated in `security:red`, `devsecops:audit`, `db:review`, `qa:review`.
Fix: `shared/findings-format.md`, symlinked into those four plugins, one line
in each skill. Gain small (about 300 bytes x 4), consistency large.

**T5. The agents trio overlaps.** `orchestrate` (10,044), `parallel` (9,308) and
`delegate` (8,556) each carry a land/review section, an "Other agents" table
(`parallel:153-168`, `orchestrate:186-190`) and a `wt` description. Fix: move
the per-agent session table into `plugins/agents/references/agents-table.md`
(the plugin has a `references/` dir already, content not read: not verified).
Gain about 1 KB.

### 2.4 Trigger quality

**R1. `git:workflow` and `repo:pr` both claim opening and merging a PR.**
`git:workflow` says "Use whenever committing, branching, merging, opening or
merging a PR" with the examples "merge it in", "open the PR"; `repo:pr` claims
"open a PR" and "merge it". The commit-request and merge-request evals pass
for `git:workflow` (`plugins/git/evals/merge-request`), and `repo:pr` has
`pr-open`, but there is no eval with both loaded and an ambiguous "merge it".
Fix: in `git:workflow` say "the rules for landing" and drop the PR verbs, or
in `repo:pr` start with "on a platform". Add one two-skill trigger eval.

**R2. `agents:orchestrate` vs `agents:parallel` vs `agents:delegate`.**
"split this work between agents" (orchestrate), "use worktrees" and "run the
tracks at once" (parallel), "use my other agents" (delegate). Evals exist with
`not-agents-parallel` graders in `orchestrate-feature` and `delegate-to-codex`,
which is the right shape; "build these features with several agents at the same
time" is the likely miss. Not verified by running the evals.

**R3. `devsecops:audit` vs `supply-chain` vs `security:red`.** "is our CI
secure" (audit) and "add security scanning" (supply-chain) share "CI security";
`security:red` also covers "cloud and IaC misconfigurations". The evals have a
`not-devsecops-pipeline` family but no `audit` vs `supply-chain` negative.
Fix: add `not-devsecops-supply-chain` to `audit-workflows`.

**R4. `project:status`, `survey`, `recap`.** Descriptions are explicit and
each has a `not-project-status` style grader (`recap-stalled`, `introspec-legacy`,
`pitch-directors`), which is good. "what's next" in `status` can also match
`recap`'s "catch me up". Low risk.

**R5. Descriptions without the phrases users say.** Most end with 2-5 quoted
phrases. `git:workflow` begins with "This user's git rules", which names no
action until the second sentence; `compact:code` is the shortest meaningful
(3,367 bytes) and its phrases are situations ("before reading large files")
rather than requests. Low priority.

### 2.5 Consistency and gaps

Spot checks (looked at, resolved): `../../references/pipeline-digest.md`,
`spec-kit.md`, `token-tracking.md` from `project:status` exist in
`plugins/project/references/`; `ai-gent delegate` subcommands in
`agents:delegate` (`detect`, `ready`, `brief`, `run`, `mark`, `check`,
`commit`, `review`, `watch`, `stop`, `plan`, `verify-sources`, `ack`) are the
documented ones (the Rust CLI source was not opened to compare); `scripts/update-check.sh`
exists. `AGENTS.md` says `scripts/check.sh` verifies every backticked path and
`plugin:skill` reference; I did not run it, so a full link audit is not verified.

**C1. The delegate skill is the model for the others.** It is 8,556 bytes but
its mechanics are `ai-gent delegate *` calls with `--json`-style outputs. The
mechanical half of `orchestrate` and `parallel` would follow the same shape
(S2, S3, S4). No finding, a direction.

**C2. Eight plugins have no deterministic tooling**, listed in part 1: every
`backend`, `frontend`, `qa`, `security`, `devsecops` rule rests on the model.
Their evals are trigger-level; behavior cases cover about 14 of 50 skills
(counted from case names: delegate, parallel, pipeline, watch, uiux, workflow,
init, introspec, retrofit, load, sync, pass, red, and `db`'s raw client). No
behavior case for `backend:build`, `frontend:build`, `qa:e2e`, `devcontainer:*`
or `project:status`. Fix: with S2 and S1 in place, a behavior case per build
skill ("does not mark Implemented with open Convergence tasks") becomes cheap
to grade because `spec check` is the oracle. Cost per run is real tokens
(`docs/product/evals.md` notes), so add one case each, not a suite.

**C3. Pipeline stages with no mechanical check.** Of the chain in
`shared/pipeline-digest.md`: `project:status` is the only checker, and it is
prose (S1). `docs/product/domain-model.md` is written by `backend:domain`
"after `project:spec`" while the digest lists it among the early artifacts;
the order is stated twice in different places, no check enforces it (not
verified against `pipeline.md`).

**C4. The `project` mod computes Next with the pipeline's rules, and says
`project:status` stays the report of record** (`pipeline-model.ts:5-6`). That
is two sources for one rule; `consistency.py` guards the artifact paths only
(`pipeline-model.ts:11`). S1 closes it.

## 3. Quick wins and larger changes

Effort: S up to half a day, M up to two days, L more. Every row needs the
owner's approval (it changes a skill, plugin, hook, shared reference or the
binary). Token figures are estimates, not measurements.

| ID | Change | Effort | Gain | Risk | Approval |
|---|---|---|---|---|---|
| T2 | Shared `implement-contract.md` for the two build skills | S | about 3.4 KB less across the pair, one place to fix | low; rerun `backend` and `frontend` trigger evals | yes |
| T1 | Trim the 22 descriptions of 350+ characters | S | about 1,000-1,300 tokens every session | trigger drift; rerun 13 plugins' trigger evals | yes |
| S4 | Name the `ai-gent` command next to each Python runner call | S | reliability, fewer runtime deps | clone installs have no binary; keep both | yes |
| R1 | Separate `git:workflow` and `repo:pr` verbs, add one two-skill eval | S | fewer wrong loads | low | yes |
| R3 | Add the `audit` vs `supply-chain` negative grader | S | catches a routing miss | none, an eval only | yes |
| H1 | Repo-guard `ask` for `gh run rerun/cancel`, `gh workflow run` | S-M | enforces a "never" that today is prose | rule must land in Rust, `guard.sh`, `rules.mjs` and fixtures | yes |
| H5 | Check the containers guard for `exec … sed -i/tee` | S | confirm or close a gap | none, a check first | yes if it changes |
| S3 | `ai-gent repo merge-style --json` | S-M | a rule used in 4 skills becomes exact | low; evals exist | yes |
| T4 | `shared/findings-format.md` for red, audit, db:review, qa:review | S | consistency | low | yes |
| T3 | Split `frontend:build`, `backend:build`, `frontend:uiux` phases into `references/` | M | about 3-4 KB per build run | model skips the pointer; add a behavior case | yes |
| T5 | Move the "Other agents" tables of the agents trio to a plugin reference | S | about 1 KB | low | yes |
| S2 | `ai-gent spec check` | M | one correct Implemented gate for 3 skills | parsing `tasks.md` variants | yes |
| S1 | `ai-gent pipeline status --json`, skill and mod read it | L | status identical across agents; about 3 KB less prose | two implementations must match during the move | yes |
| H4 | SessionStart pipeline hint | M (after S1) | the status skill gets used without a prompt | noise in non-pipeline projects; print nothing there | yes |
| H2/H3 | Guard on `specs/README.md` and `tasks.md` edits | L | enforces pipeline rule 6 | false positives; needs S2 | yes |
| C2 | One behavior case per build skill and `project:status` | M | evidence that the rules hold | token cost per run | yes |

Suggested order: T2, R1, R3, H5, H1 (no new commands), then S2 and S3, then S1
with H4, then H2/H3 and C2.

## 4. What not to change

- **`agents:delegate` and its `ai-gent delegate` commands.** The mechanics are
  already a binary with checks, the skill is the thinnest of the three in
  proportion to what it does, and the worker rules are in
  `shared/delegate-worker-rules.md`. Copy this shape; do not reshape it.
- **`devsecops:watch`'s `allowed-tools`, `model: haiku` and `context: fork`.**
  The read-only promise is enforced by the harness, not by wording. Keep
  it; H1 only covers the main-model side.
- **The guards and their parity discipline** (Rust binary, `guard.sh`,
  `rules.mjs`, `shared/guard-fixtures/`, the parity test). It is the repo's
  strongest reliability asset. Any new rule (H1-H3) must go through it, not
  around it.
- **`pipeline-digest.md` beside `pipeline.md`.** The digest keeps stage skills
  lean and says that the full file wins on conflict. Do not merge them.
- **`project:status` being read-only and ending in one recommendation.** Only
  its computation should move (S1), not its contract or its output block.
- **One `fires-*` trigger eval per skill.** All 50 skills have one, with
  `not-*` neighbors where routing is delicate. Keep adding to it, never
  replace it with behavior cases.
- **`secrets` and `db` rules that live in the guards** ("never read a value",
  "only a runner reads"). They are already hooks; do not restate them in more
  skill prose.
- **Description cap and style** (400 characters, no `: ` or ` #`): the cap
  holds today (0 over). T1 is about staying well under it, not changing it.
- **The `references/` pointer pattern** (`devcontainer:workflow` 11,392 bytes
  plus 6 references, `git:workflow` 9,014 plus 8). It works; extend it to the
  skills in T3 rather than invent another.

## 5. Not verified

- Token figures are estimates from byte and character counts, not measured.
- Which eval cases currently pass; no eval or test was run.
- Whether `scripts/check.sh` finds any dangling reference: not run.
- Whether `repo_state.py` already covers S5's survey and recap steps.
- Whether the containers guard catches `exec` writes (H5), and whether the Rust
  guard can read `tasks.md` (H2).
- The content of `plugins/agents/references/` and of skills not named above;
  only headings, sizes, descriptions and rule-word lines were sampled for them.
- That `claude plugin eval` routing matches the overlaps in R1-R4.
