# Eval history

One row per plugin per run of `scripts/run-evals.py`: the cases that passed,
the overall score, and what the run cost. Trigger cases check that a request
reaches the right skill; behavior cases check what the skill does once loaded.

| Date | Claude Code | Mode | Plugin | Cases | Score | Cost (USD) |
|---|---|---|---|---|---|---|
| 2026-10-18 | 2.1.303 | behavior | git | 2/2 | 1.00 | 0.30 |
| 2026-10-18 | 2.1.303 | behavior | secrets | 1/1 | 1.00 | 0.08 |
| 2026-10-18 | 2.1.303 | behavior | db | 0/1 | 0.67 | 0.24 |
| 2026-10-19 | 2.1.303 | trigger | security | 4/4 | 1.00 | 0.43 |
| 2026-10-19 | 2.1.303 | trigger | security | 3/4 | 0.75 | 0.23 |
| 2026-10-19 | 2.1.303 | behavior | security | 1/1 | 1.00 | 0.13 |
| 2026-10-19 | 2.1.303 | trigger | project | 13/13 | 1.00 | 1.11 |
| 2026-10-19 | 2.1.303 | trigger | frontend | 5/5 | 1.00 | 0.37 |

Notes: the `db` behavior case (`behavior-raw-client`) is flaky by design of the finding: in about half the runs the agent reads the project's `.env` before using dbrun, which the db skills don't forbid in so many words. A plain "delete rows in production" request loads no db skill at all (only a data-problem framing reaches `db:investigate`); the db guard hook is what stops a raw client then.

`security`'s trigger cases flake occasionally at the repo's usual `runs: 1`: a single-sample miss on `scope-trigger` or `red-trigger`, same as other plugins' routing noise. The first attempt at these two prompts ("penetration test", "red team exercise, find exploitable issues") was stopped outright by Anthropic's own safety classifier before any skill could route — not a plugin bug, but worth knowing: softer professional phrasing ("security assessment", "exploitable security weaknesses") carries the same routing signal without tripping it.
