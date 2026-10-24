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
- Messages are **deduplicated by message id** (streamed chunks and account
  switches repeat them) and local `<synthetic>` placeholders, which stand in
  for a limit or an error, are skipped. A message keeps its last, most
  complete usage.

## Where the numbers live

| Where | What | Who writes it |
|---|---|---|
| `tasks.md` task line | `· Tokens: projected ~420k \| used 391k`, next to `Executor · Reviewer · Effort` | `tasks` |
| `specs/NNN-*/cards.md` | `Tokens: projected` on every card's route line (a lint requires it) | the planner, from `estimate` |
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
`executor_model`, `files` and `lines` (what the card's agents wrote through
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

## History and estimates

Each repo commits its `docs/product/token-history.jsonl`. `index` rebuilds
`~/.ai-gent/token-history.jsonl` from every repo's history (`--scan ~/codes`),
so estimates draw on experience from every project. `estimate` reads this
repo's history and that index.

```bash
$TOKREP estimate --role builder --effort high --layer backend --tasks 5
```

It prints the **median** (the projection) and the **p75** (the upper bound),
and how many similar records it used. Records match on layer, effort, model
family and size (S up to 3 tasks, M up to 6, L above); the match widens
(layer, then size, then effort) until at least 5 records agree. With fewer
than 5 even at the widest, it falls back to the **starting table**, per model
and effort, for a medium card:

| Model | low | medium | high |
|---|---|---|---|
| Sonnet | 150k (p75 250k) | 350k (550k) | 600k (850k) |
| Opus | 250k (400k) | 450k (650k) | 700k (950k) |
| Haiku | 60k (120k) | 120k (200k) | 200k (320k) |

Size scales the table: S x0.6, L x1.6. It was calibrated on rstory's first
autonomous run (Sonnet feature builders 0.3-0.8M each, Opus agents 0.5-0.9M
each) and is only a start; history takes over as records accumulate.

Records carry a **confidence**: `exact` (a card or its task ids matched),
`feature` (only the feature matched) and `agent` (nothing did). `estimate`
weights them 1.0, 0.6 and 0.3 when taking the median and p75, so a handful of
exact cards outweigh many coarse ones. A record some repo has measured
supersedes a seeded record of the same agent.

`seed` backfills the local index from past sessions of projects that never
tracked tokens, so the first estimates are not blind. It is read-only on the
sessions and on each repo's specs and writes only `~/.ai-gent/token-history.jsonl`
(`source: seed`); the owner decides per repo whether to run `tasks` there.

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
```

Before dispatching a wave:

```bash
$TOKREP budget --cards 004-foundation,004-us1-core
```

It sums the cards' projections (from `cards.md`, else `estimate`), adds the
coordinator overhead, subtracts what the session (from the transcripts) and
the week have already used from the caps, keeps the reserve, and says
**proceed**, **shrink** (the first N cards fit; dispatch those) or **stop**
(not even one fits: name what is queued and ask). It also states the
concurrency limits. The weekly figure counts every project of the account.

Rules that go with it: no more than `max_opus` Opus subagents and `max_agents`
subagents at once (rstory hit the session limit twice running three or four
Opus agents together); Sonnet builders, with the thinker's review done
inline unless the card needs a fresh context; a subagent stopped by a limit is
**resumed only when its remaining work justifies re-reading its whole context**
(a resume re-reads everything; in rstory it roughly doubled those agents'
cost), otherwise redispatch a narrower card with only what is left.

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
