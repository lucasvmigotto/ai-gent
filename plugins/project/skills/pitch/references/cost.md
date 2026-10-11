# AI effort and cost section

Optional, `large` by default, collapsed behind `<details>`.

## Tokens

From the local agent logs of the sessions that built the project:

- Claude Code: `~/.claude*/projects/<project-dir>/**/*.jsonl` (subagent
  transcripts included). Each assistant message carries `usage`:
  `input_tokens`, `output_tokens`, `cache_creation_input_tokens`,
  `cache_read_input_tokens`. The same message appears several times while
  streaming: **deduplicate by message id + request id and keep the largest
  output count.** Several profiles may share one projects directory; count
  each message once.
- Other agents: their session export, when they keep per-call usage; if
  not, say the section can't be estimated.

Group by **role** (planning/review/orchestration on the strong model,
independent reviews, implementation on the fast model, evaluation runs,
searches) and by model. Show uncached input, cache writes, cache reads,
output and number of calls.

Say what is left out (work before the logs, tools that don't log, real
users' usage) and that logged output may be under the real figure.

## API-price equivalent

Price the same tokens with the provider's public price list, read from the
agent's own reference (never from memory), with the cache-write rate the
agent actually uses (Claude Code: the 1-hour rate). Give a total, a range
(5-minute vs 1-hour cache writes) and the share by model. State that the
work ran on subscriptions if so, and that this is an order of magnitude,
not a bill. Don't compare it with human hours unless the user gives the
rate.
