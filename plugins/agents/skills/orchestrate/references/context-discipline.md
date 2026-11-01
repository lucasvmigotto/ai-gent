# Context discipline

Cache reads are about three quarters of a run's weighted cost, so the cost of
a session is its context size times its number of calls. A transcript audit
of this repo's own usage found these levers. Each rule is one line and its
reason.

## The coordinator (you)

- **Never idle more than ~4 minutes with a context over ~100k.** The prompt
  cache lives about 5 minutes; an idle gap rebuilt long contexts and was about
  30% of billable. Wait with a background watcher (Monitor, or one background
  `gh pr checks --watch | tail -5`), or checkpoint (`project:checkpoint`) and
  resume in a fresh context.
- **Keep your context under ~200k: compact or restart at ~150k.** Every later
  call re-reads all of it; Opus coordinators were 42% of weighted cost.
- **Run the coordinator on a mid-size model by default; use the strongest only
  at planning and review gates.** A Sonnet-class coordinator cost about a tenth
  as much per run. This is a recommendation: the role-to-model mapping stays in
  `../../../references/agent-roles.md`.
- **Use the smallest model for pure review or fix of text.** Wording, link and
  lint fixes need no reasoning budget.
- **Read files with Read and offset/limit, not through the shell.** Reading via
  `sed`/`cat` injected 895k tokens, 40% of tool output.

## The cards

- **Prefer a scoped, self-contained card on a general-purpose agent over a
  fork.** A subagent starts at about 40k context, a fork at about 78k, and the
  difference is paid on every call.
- **Keep a card to about 40 tool calls; split bigger ones.** Context grows with
  each call, so two small cards cost less than one long one. The measured
  median for a Sonnet 5.5 builder card is 128k billable (p75 179k).
- **Name the card id first in the dispatch description** (`004-foundation:
  T001, T002 …`). Without it `token_report.py tasks` cannot attribute the cost
  (`../../../references/token-tracking.md`).

## Checks and waiting

- **Run the full `scripts/check.sh` once per card, before the commit; use a
  scoped test or `check.sh --quick --quiet` inside the loop.** The full script
  was run about 6 times per card, each a turn over a 200-370k context.
- **Batch shell steps** into one call (several commands, `| head` / `| tail`)
  so you pay one turn, not four.
- **Replace polling loops** (`sleep`, repeated `gh pr checks`) with one
  background `gh pr checks --watch | tail -5`.
- **Count before you dump:** `grep -c` or `| head -40` before printing matches;
  `report --summary` and `tasks --summary --top 5` for token reports (their full
  output can reach 7k tokens).
