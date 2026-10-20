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

At hand-off (`agents:orchestrate` step 5, `agents:parallel` step 4), run
`$TOKREP tasks --project=<the session slug under ~/.claude*/projects/>
--specs-dir specs` — it reads every session's transcripts (Claude Code
carries per-message usage: input, output, cache write, cache read),
dedupes by message id, and links each card's cost to the task ids its
dispatch description names (`cards.md`). It writes `specs/<feature>/
tokens.md` (a record file beside `tasks.md`, never written into it) and
the project-wide roll-up at `docs/product/token-usage.md`; a card naming
no task id lands in the roll-up's Unmapped section instead. Backfill
stage markers from `dev` landings with `$TOKREP backfill-git --repo=.`,
or mark one by hand with `$TOKREP snapshot --label="..."`.

This reads Claude Code's own on-disk transcript format and the Agent
tool's subagent `.meta.json` sidecars — there is no equivalent on another
agent, so the ledger's "tokens used" column stays manual there (whatever
the agent's own UI reports, if anything). An API-price equivalent can
help a manager, but say it is an order of magnitude, and take prices from
the agent's own reference, never from memory.

## Pacing

Before dispatching a new wave — the next parallel group of cards, or the
next track in `agents:parallel` — weigh what's left against what's about
to run:

1. **The remaining-budget figure is ambient, not fetched.** Claude Code
   surfaces it as a `<total_tokens>` system reminder on its own schedule;
   there is no tool call that returns it on demand. Use the most recent
   one already in context.
2. **Estimate the wave's cost from this project's own history**, not a
   guess: `$TOKREP estimate --role builder --rollup docs/product/
   token-usage.md` (or `--label <id>` for one specific card, `--model`
   for a project that overrode `agent-roles.md`'s default mapping).
   `estimate` says plainly when there's no history yet — the project's
   first feature always falls back to the card's S/M/L size, same as
   before this existed.
3. **Decide, out loud to the user:**
   - comfortably covered → dispatch the wave as planned;
   - tight → shrink it (fewer cards this round, or a lower
     `max_parallel`) and say why;
   - not enough for even one more card → stop, name what's queued and
     unestimated, and ask (resume next session, or continue anyway —
     their call, not an automatic one).

This is a heuristic, not a guarantee: past cards sharing a role or label
are a reasonable proxy for a new one's cost, not a promise. Treat a wide
spread in the samples (`estimate` reports the count — weigh a 2-sample
average less than a 20-sample one) as a reason to pad the estimate, not
to distrust the tool.
