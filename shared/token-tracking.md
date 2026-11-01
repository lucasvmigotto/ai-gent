# Token tracking

How a project records what each task costs, projects what the next one will
cost, and keeps a wave inside the user's budget. The tool is
`plugins/agents/scripts/token_report.py` (Python standard library only, Claude
Code only: it reads Claude Code's own transcripts). Skills call it as
`TOKREP="python3 <plugin>/scripts/token_report.py"`.

The loop, per card: **project** before the work (`estimate`), **record** after
it (`tasks`), **check** before the next wave (`budget`). Over time the history
replaces the starting guesses, and the error of each projection is reported.

## What is counted, and why billable

For every assistant message the transcript carries the API's own usage:
uncached input, cache writes, cache reads and output.

- **Billable = uncached input + cache writes + output.** This is the number
  tracked, projected, written on task lines and compared with the budget.
- **Cache reads are excluded but reported** (the `cache_read` field of a
  history record, a column in `tracker.md`). Every step of a long session
  re-reads its context from the cache, so reads dwarf everything else
  (billions in a big run) and cost a small fraction per token; counting them
  would hide what a card actually spent. They matter for one thing: a
  bloated context makes every later call dearer, so keep the thinker's lean.
- **Idle gaps rebuild the cache.** The prompt cache lives about 5 minutes. A
  coordinator that idles longer re-writes its whole context on the next turn
  (in this repo's audit, about 30% of billable). Never idle more than ~4
  minutes in a long session: wait on a background watcher or checkpoint and
  resume fresh (`agents:orchestrate`, its context-discipline reference).
  `report --summary` prints the gaps over 5 minutes that re-wrote 20k or more.
- Messages are **deduplicated by message id** (streamed chunks and account
  switches repeat them) and local `<synthetic>` placeholders, which stand in
  for a limit or an error, are skipped. A message keeps its last, most
  complete usage.

## Where the numbers live

| Where | What | Who writes it |
|---|---|---|
| `tasks.md` task line | `· Tags: extend, backend, protocol, rust, tokio, known-pattern, integration, M · Tokens: projected ~420k \| used 391k`, next to `Executor · Reviewer · Effort` | the planner (Tags), `tasks` (Tokens) |
| `specs/NNN-*/cards.md` | `Tags:` and `Tokens: projected` on every card's route line (a lint requires both; `token-tags.md` lists the tags) | the planner, from `estimate --tags` |
| `specs/NNN-*/tracker.md` | a `Tokens` block: per card projected, used, error, then the subagent total, the coordinator share and the feature total | `tasks` |
| `.ai-gent/orchestrate-status.md` | a `Tokens` column, `~projected / used` per card | `tasks` |
| `docs/product/token-history.jsonl` | one JSON line per finished card, committed | `tasks` |
| `~/.ai-gent/token-history.jsonl` | the cross-project index (every repo's history, plus seeded sessions) | `index`, `seed` |
| `.ai-gent/budget.yaml` | the user's caps and concurrency limits | the user |

A card's figure is the card's: it sits on the **first task line** of the card
(file order), and the card's other tasks say `· Tokens: with T001`, so a sum
over the lines never counts a card twice. `tasks` replaces the tag on every
run; running it again updates the numbers and never adds a second tag, a
second tracker block or a second history line.

A history record: `id`, `source` (`measured` or `seed`), `confidence`,
`project`, `feature`, `card`, `tasks`, `layer`, `effort`, `executor_role`,
`executor_model`, `model_id` (the full model id of the card's lead agent, e.g. `claude-sonnet-5-5`; absent in older records), `files` and `lines` (what the card's agents wrote through
Edit/Write, a size signal rather than a diff), `projected`, `used`,
`cache_read`, `coordinator`, `calls`, `agents` (agent ids, so nothing counts
twice), `desc`, `date`.

## How attribution works

Each subagent is one row (a resumed agent keeps its id, so all its calls,
across every resume and every account's config dir, are summed once). Its
description (`Dispatch description:` in the card) and, if needed, its prompt
decide where it belongs:

1. The description equals a card's dispatch description, or names a card id
   (`004-us1-core`): **exact**, that card.
2. It names task ids (ranges such as `T006-T009` expand) and a feature number
   or slug (`004-daemon`, `012 autostart`), or task ids found in only one
   feature: **exact**, those tasks; the card too when they sit in one card.
3. It names only a feature, in its description, or as the feature its prompt
   mentions most (`specs/004-daemon/...`): **feature**.
4. A description naming several features is read as the first one only when
   it leads with it (`004-daemon build cards + 008 remainder`); a spec-wide
   agent (`Plan+tasks 001, 002, 014`) stays unattributed.
5. Anything else is **agent-only**: it is listed in `tasks`' output as
   unattributed and kept in the history with its model and description, but it
   is never written onto a task line or into a feature total. Not guessed.

Because task ids repeat across features (every feature has a `T001`), put the
card id or the feature number in the dispatch description:
`T001, T002: 004-daemon foundation`.

Attribution needs cards to match against: a repo with no `specs/*/cards.md`
(or whose dispatch descriptions carry no card id, task ids with a feature
number, or the card's dispatch description) reports `cards: []` and every
subagent unattributed. That is a convention gap, not a bug: start every
dispatch description with the card id. Branch names and free titles
(`Build <feature>`) are not matched, since they would be a guess.

## The coordinator split

The main thread (the coordinator) works between landings: planning, reviews,
merges, CI follow-up. Its usage is split by **stage window**. Windows are the
gaps between stage markers, from the repo's first-parent landings on the
integration branch (read from git when no `token-snapshots.json` exists) or
from `snapshot`/`backfill-git`. In each window the coordinator's billable is
shared among the features whose subagents ran in it, **weighted by those
subagents' billable in the window**. A window in which no attributed subagent
ran stays on its own line (`coordinator ... with no attributed subagents`), so
nothing is spread thin over unrelated features. In `tracker.md` the share is
the feature's; in a history record, a card's `coordinator` is the feature's
share by the card's part of the feature's subagent usage. `used` itself is
the card's subagents only.

## Tags

Every card is tagged at planning with one value per facet, from the
controlled list in `token-tags.md` (`shared/token-tags.json`): `kind`,
`layer`, `surface`, `lang`, `novelty`, `verification`, `size`, plus any
number of `stack` values (normalized crate and framework names; aliases such
as `react-dom` fold into `react`). The planner (`agents:orchestrate`, the
Spec Kit tasks step) writes them on the card and its first task line:
`· Tags: extend, backend, protocol, rust, tokio, known-pattern, integration, M`.
`scripts/consistency.py` fails a `cards.md` card without exactly one valid
value per facet (`lang` required, `stack` optional). To add a value, add it
to the JSON and to `token-tags.md`.

## History and estimates

Each repo commits its `docs/product/token-history.jsonl`. `index` rebuilds
`~/.ai-gent/token-history.jsonl` from every repo's history (`--scan ~/codes`),
so estimates draw on experience from every project. `estimate` reads this
repo's history and that index.

```bash
$TOKREP estimate --role builder --effort high \
  --tags "extend, backend, protocol, rust, tokio, known-pattern, integration, M"
```

It prints the **median** (the projection) and the **p75** (the upper bound),
how many similar records it used, at which match level, and a confidence.

**With tags**, it is a weighted nearest-neighbour search over history:

- A record is a neighbour when it shares every facet of the current match
  level. The strictest level is `kind`, `surface`, `novelty`, `lang`, `layer`,
  `verification` and `size`; with fewer than 5 neighbours it **backs off**,
  dropping `size`, then `stack`, then `verification`, then `layer`, then
  `novelty`. The level reached is the confidence: levels 0-1 are `high`, 2-4
  `medium`, deeper `low`.
- `stack` is never a filter. While it is in play, each shared value raises a
  neighbour's weight by its Jaccard overlap with the card's stack (up to
  +50%); a record that lacks the stack is not excluded.
- A neighbour's weight is its confidence (below), halved if its tags were
  inferred rather than planned, plus 25% for each dropped facet it still
  shares.
- When `size` was dropped, a neighbour's cost is scaled by the size ratio
  (S x0.6, M x1, L x1.6) to the card's size.
- Costs are task costs: a record's `used_clean`, without resumes, fix rounds
  and flake hunts (next section).
- **Execution context** is a separate factor, not a tag: how much more or less
  a model and role costs than a Sonnet builder. Neighbours' costs are divided
  by their own context factor, and the median is multiplied by the card's.
  Factors are learned from history (groups that share kind, surface, novelty,
  lang and layer but ran in different contexts; a context needs 5 such
  comparisons), with a prior until then (Opus x1.3, Haiku x0.5).
- **Bias correction.** Each record with a projection and a result gives
  `used / projected` per tag. Once a tag has at least 5 such records, the
  geometric mean of the card's tags' ratios (clamped to 0.5-2.0) corrects the
  projection, and `estimate` prints which tags did. It fixes a planner that is
  systematically off; it is not a second estimate.
- **Hook.** `loglinear_model()` in `token_report.py` is the place for a
  log-linear model (log cost against tag indicators, context and size) once
  history holds a few hundred records (`LOGLINEAR_MIN`). It is not built; it
  returns nothing and the levels above answer.

Without `--tags`, or when even the loosest level has fewer than 5 records,
the older match applies: records of the same model with the same layer,
effort and size, widening until 5 agree. With still fewer than 5, it falls
back to the **starting table**, per model and effort, for a medium card:

| Model | low | medium | high |
|---|---|---|---|
| Sonnet | 55k (p75 80k) | 128k (179k) | 220k (275k) |
| Opus | 250k (400k) | 450k (650k) | 700k (950k) |
| Haiku | 60k (120k) | 120k (200k) | 200k (320k) |

Size scales the table: S x0.6, L x1.6. The Sonnet row is **measured**: 20
Sonnet 5.5 builder cards in this repo's history (Oct 2026) had a median of 128k
and a p75 of 179k per card; low and high keep the previous table's proportions.
The **previous defaults** (Sonnet 150k (p75 250k), 350k (550k), 600k (850k), calibrated
on rstory's first autonomous run) are kept as `STARTING_PREVIOUS` in
`token_report.py`; the Opus and Haiku rows are unchanged from them. The measured
row is the one used when no history exists, for Sonnet-class builders. It is
only a start; history takes over as records accumulate.

Output size: `report` and `tasks` can print thousands of tokens. Use `report
--summary [--top N]` (a few lines) and `tasks --summary [--top N]` (totals and
the N costliest unattributed agents) unless you need the full tables.

Records carry a **confidence**: `exact` (a card or its task ids matched),
`feature` (only the feature matched) and `agent` (nothing did). Estimates
weight them 1.0, 0.6 and 0.3, so a handful of exact cards outweigh many
coarse ones. A record some repo has measured supersedes a seeded record of
the same agent.

`seed` backfills the local index from past sessions of projects that never
tracked tokens, so the first estimates are not blind. It is read-only on the
sessions and on each repo's specs and writes only
`~/.ai-gent/token-history.jsonl` (`source: seed`); the owner decides per repo
whether to run `tasks` there. Seeded records carry **inferred, low-confidence
tags** (`tags_source: inferred`): `kind`, `surface` and `layer` from the
agent's description, `size` from the files and lines it wrote, `lang` from
the files it touched (else the repo's manifests: `Cargo.toml`,
`package.json`, `pyproject.toml`, `go.mod`, `pom.xml`) and `stack` from those
manifests' dependencies that belong to that language. `novelty` is never
inferred.

## Process events, kept out of task cost

Some spend is not the task: an agent **resumed after a limit** (the first call
after the stop re-reads its whole context), the **fix rounds** that follow a
review (an agent whose description is a fix or names findings) and **CI flake
hunts**. `tasks` records them in the history record as
`events: {resumed: 1, fix_rounds: 2, ci_flake: 0, reviews: 1}`, keeps `used`
as the full real cost, and adds `used_clean`, the task without the resumes,
fix rounds and flake hunts. Estimates learn from `used_clean`. The budget
adds the events back as risk: the share of spend they took in history (8%
until history knows), plus 12% for each Opus agent running beyond the first,
because parallel Opus agents are what hit the limits.

## The budget

`.ai-gent/budget.yaml`, flat `key: value` lines (`12M`, `800k` or plain
numbers):

```yaml
session_billable: 12M        # cap for the current session (omit for no cap)
week_billable: 60M           # cap for the last 7 days
session_window_hours: 5      # optional: count the last N hours, all projects, instead of this session
reserve_pct: 10              # left unspent
coordinator_overhead_pct: 35 # the main thread's cost on top of the subagents' (rstory: 38%)
max_opus: 1                  # concurrent Opus subagents
max_agents: 2                # concurrent subagents
rate_window_minutes: 30      # the window the burn rate averages over
```

Before dispatching a wave:

```bash
$TOKREP budget --cards 004-foundation,004-us1-core
```

It sums the cards' projections (their `Tokens: projected` lines, else
`estimate` from the card's tags), adds the coordinator overhead and the event
risk, subtracts what the session (from the transcripts) and the week have
already used from the caps, keeps the reserve, and says **proceed**, **shrink**
(the first N cards fit; dispatch those) or **stop** (not even one fits: name
what is queued and ask). The weekly figure counts every project of the
account. It then packs the cards that fit, in dependency order, into
**batches** of at most `max_agents` subagents and `max_opus` on Opus, and it
suggests moving an expensive Opus card, whose projection is at high or medium
confidence, to a Sonnet builder with the thinker reviewing inline (with the
cost the learned context factor predicts).

`tasks --dry-run --json` prints, read-only, one JSON object instead of the text
table: `cards` (each `card`, `feature`, `model`, `projected`, `used`, `start`,
`end`, `agents`, `calls`) and `unattributed` (a count). `start` and `end` are
UTC with a `Z`. `--json` without `--dry-run` is rejected (exit 2), and nothing is
written. Pass the project as `--project=<slug>` (the cwd with every character
outside `A-Za-z0-9-` replaced by `-`, as `budget` derives it).

`budget --json` prints the same verdict, the caps (`cap`, `used`, `left`,
`eta_minutes`), the cards with their projections, the batches and the `rate` as
one JSON object, and nothing else; the Claude Code token status reads it.

The **burn rate** is the billable tokens of the last `rate_window_minutes`
(default 30) of the week's messages, all projects, scaled to an hour: `rate` is
`{per_hour, window_minutes, messages}`, and the text gains one `rate:` line
after the caps. Each cap's `eta_minutes` is `left / per_hour * 60`, rounded.
Fewer than 3 messages in the window (`MIN_RATE_MESSAGES`) make `rate` null and
every ETA null, because one message after a quiet hour is not a pace; no usage
in the window gives `per_hour` 0 and no ETA. The ETA ignores usage aging out
of a rolling `session_window_hours`, so it is conservative there. The mod only
formats these figures (`~2h10m`, `7.40M/h`) and toasts a cap whose ETA falls
under `tokenEtaWarnMinutes`; the window is this file's key, not a mod option.

The **landing toast** (`cardCostToast`, `cardCostHintPercent`) reads no script
output at all: it takes the ledger's Tokens cell of a card that just turned
`landed` and prints `used 391k vs ~420k projected (-7%)`. The percentage is
computed from the cells' rounded values, half to even like `pct_error`'s
`{:+.0f}`, so it can differ by a point from `tracker.md`'s Error column
(`pct_error` over the exact figures). `hooks/pct-fixtures.ts` is the shared
table that `test_token_report.py` runs `pct_error` against.

Rules that go with it: Sonnet builders carry the confident, expensive cards
and the thinker reviews inline unless a card needs a fresh context; no more
than `max_opus` Opus subagents and `max_agents` subagents at once (rstory hit
the session limit twice running three or four Opus agents together); a
subagent stopped by a limit is **resumed only when its remaining work
justifies re-reading its whole context** (a resume re-reads everything; in
rstory it roughly doubled those agents' cost), otherwise redispatch a
narrower card with only what is left.

## Limits and caveats

- **Account switches.** Transcripts live in one dir per account
  (`~/.claude*`). Reports merge them and dedupe by message id; the budget
  counts only `CLAUDE_CONFIG_DIR`'s account when it is set, since caps are
  per account. A card resumed on another account is still one agent.
- **Synthetic stops.** A limit stop leaves a `<synthetic>` message with no
  real usage; it is skipped. The work done before it is counted.
- **Cache reads** are reported, not counted: a card that re-reads a huge
  context is cheaper in billable than in real load.
- **Main-thread work is not a card.** Only subagents are attributed to cards; work the
  coordinator does itself lands in the coordinator share, so dispatch real work to subagents
  (which is also the cheaper habit).
- **Attribution depends on descriptions.** A vague description is reported as
  unattributed; fix the next dispatch, not the number.
- **Edit sizes** (`files`, `lines`) count Edit/Write calls the agents made,
  not the final diff.
- **Estimates are medians of the past**, not promises; a wide spread (p75 far
  above the median) is a reason to pad. Say so to the user.
- **Not a price.** Billable tokens are a cost proxy; turn them into money only
  with the vendor's own price list, never from memory.

## Recording a phase's cost

On Claude Code with the `agents` plugin, a build or test phase records what it
cost: `python3 <agents plugin>/scripts/token_report.py tasks --project=<session
slug> --specs-dir specs` writes `used` next to each card's projected tokens on
its task lines, in the feature's `tracker.md` and in
`docs/product/token-history.jsonl`. A subagent leaves it to the coordinator
that dispatched it (its own transcript is still open), and work done in the
main thread alone is not attributed to a card.

## Maintaining the tooling

`plugins/agents/scripts/token_report.py` is stdlib only; its tests are in
`test_token_report.py`, run by `scripts/check.sh`. The tests never read real
transcripts or `~/.ai-gent`; they build a fake `HOME`. The tag vocabulary is
`token-tags.json` plus its documentation `token-tags.md`; to add a value,
change both. `scripts/consistency.py` fails if they differ, if a value is in
two facets, or if a `cards.md` card lacks `Tags:` or `Tokens: projected`.
