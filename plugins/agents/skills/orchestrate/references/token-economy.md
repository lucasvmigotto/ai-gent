# Token economy

Where orchestration saves or wastes tokens, from a real feature-by-feature
run (a knowledge base and two tools built over ten days: about 7,400 model
calls, 2.5 M output tokens, 44 M written to cache and 1.35 B read from it).

## Where the tokens go

- **Cache reads dominate.** Every step re-reads the conversation; almost all
  of it comes from the cache. New input was close to zero. So the cost of a
  long-running thinker session is its context size times the number of
  steps — keep the thinker's context lean.
- **The thinker is the expensive line.** In that run about 70% of the
  estimated cost was the strong model, planning, reviewing and orchestrating
  with the project's full context. Move every bulk read and mechanical step
  out of it.
- **Output is small.** A few percent of the cost. Long reports still hurt,
  because the thinker re-reads them on every later step.

## What to do

Context size, idle gaps, the coordinator's model, tool-call budgets and check
frequency have their own page: `context-discipline.md`.

| Practice | Why it saves |
|---|---|
| Builder reads the files, thinker reads the diff | Bulk reading happens once, in the cheaper model |
| Self-contained cards | The builder doesn't re-explore; the thinker doesn't re-explain |
| Short structured reports | They stay in the thinker's context for the rest of the session |
| Batch small tasks | One card's preamble and setup instead of five |
| Follow-ups to the same builder | Its context is already loaded and cached |
| Resume, don't restart, after a limit | The work and the reading already done are kept |
| Cap parallel agents (about 4) | Eleven at once hit account limits twice; the restarts cost more than the parallelism saved |
| Stable role preamble, varying part last | Reuses the prompt cache across cards |
| `Effort: low` for mechanical cards | Less reasoning spent where none is needed |
| Small model for searches | "Where is X" doesn't need the strong model |

## Measuring

```bash
TOKREP="python3 <plugin>/scripts/token_report.py"
```

The method, the numbers and their limits are in
`../../../references/token-tracking.md`. In short, per card: project it at
planning (`$TOKREP estimate --role builder --effort high --layer backend
--tasks 5`, written as `Tokens: projected` on the card), record it after each
landing, and report the error at the end.

After each landing and at hand-off (`agents:orchestrate` step 5,
`agents:parallel` step 4), run `$TOKREP tasks --project=<the session slug
under ~/.claude*/projects/> --specs-dir specs`. It reads every session's
transcripts (Claude Code carries per-message usage), dedupes by message id and
attributes each subagent to its card and task ids from the dispatch
description (`cards.md`). It writes `used` onto the task lines (`· Tokens:
projected ~420k | used 391k`), a Tokens block into `specs/<feature>/tracker.md`
(with the coordinator's share), the ledger's Tokens column and the repo's
`docs/product/token-history.jsonl`; usage it can't attribute is listed, not
guessed. Stage markers come from `dev` landings (read from git, or
`$TOKREP backfill-git --repo=.`), or by hand with `$TOKREP snapshot
--label="..."`. `$TOKREP index --scan ~/codes` refreshes the cross-project
history that `estimate` draws on.

This reads Claude Code's own on-disk transcript format and the Agent
tool's subagent `.meta.json` sidecars — there is no equivalent on another
agent, so the ledger's "tokens used" column stays manual there (whatever
the agent's own UI reports, if anything). An API-price equivalent can
help a manager, but say it is an order of magnitude, and take prices from
the agent's own reference, never from memory.

## Pacing

Before dispatching a new wave — the next parallel group of cards, or the
next track in `agents:parallel` — weigh the wave's projections against the
budget the user set:

1. **The budget is a file, the usage is measured.** `.ai-gent/budget.yaml`
   holds the per-session and per-week billable caps and the concurrency
   limits; if it doesn't exist, ask the user whether to create one (a
   template is in `../../../references/token-tracking.md`). The session's
   usage comes from the transcripts, not from the ambient `<total_tokens>`
   reminder, which is only a rough cross-check.
2. **Run `$TOKREP budget --cards <ids of the wave>`.** It sums the cards'
   projections (their `Tokens: projected` lines, else `estimate`), adds the
   coordinator's overhead, subtracts what the session and the week have
   used and keeps a reserve.
3. **Decide, out loud to the user:**
   - `proceed` → dispatch the wave as planned;
   - `shrink` → dispatch only the cards that fit, say why, and queue the rest;
   - `stop` → name what's queued and its projection, and ask (resume
     next session, raise the cap, or continue anyway — their call).
4. **Cap the concurrency** at the budget file's `max_opus` (default 1) and
   `max_agents` (default 2), not at `max_parallel`: three or four Opus
   agents together hit the session limit twice in the run these defaults
   come from. Prefer Sonnet builders and inline thinker reviews.
5. **After a limit**, resume a stopped agent only when its remaining work
   justifies re-reading its context (a resume re-reads everything and roughly
   doubled those agents' cost); otherwise redispatch a narrower card with
   only what is left.

This is a heuristic, not a guarantee: a projection is the median of similar
past cards and `estimate` says how many it used and its p75 — pad for a wide
spread, and weigh a handful of records less than twenty.

## Reading the landing toast

On Claude Code, the token status (`tokenStatus`) toasts a card when it lands
with both figures of its ledger Tokens cell: `004-us1-core landed: used 391k
vs ~420k projected (-7%)`. It appears when `token_report.py tasks` has
written the used figure, which is why the skill runs it after each landing.
The percentage is computed from the cells as written (`391k`, `~420k`),
rounded half to even, so it can differ by a point from `tracker.md`'s Error
column, which comes from the exact figures. When the last card in flight
lands, one more toast counts the wave: cards landed, how many over and how
many under their projection (a count, never a token sum).

A miss of 25% or more, either way, adds a hint. 25% is the default of
`cardCostHintPercent` (0 turns the hint off); this file states no tolerance
of its own, so tune the option to the spread your cards show. What to check:

- **Over (`wrong tags or effort?`).** Open the card in `cards.md`: its `Tags:`
  (kind, layer, stack, risk, size), its effort and its task count. A missing
  `integration` or `known-pattern` tag, a size `M` for a card that was `L`,
  or a count that grew in review each move the estimate. Fix the tags, not
  the number.
- **Under (`projection too high`).** The wave budget shrank, or cards were
  queued, for tokens the card never used. Check for an over-wide size or
  risk tag, or a model priced above the one that built it.
- **Then re-run `$TOKREP tasks`** (as in "Measuring") so
  `docs/product/token-history.jsonl` records the corrected card; `estimate`
  learns from the history, not from the toast.

The mod changes none of these files: the hint is a pointer, and applying it
is the coordinator's job.
