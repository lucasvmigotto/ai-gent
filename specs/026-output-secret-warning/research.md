# Research — secret warning and redaction for tool output

Items marked *to verify* need a run on Claude Code 2.1.312 (the CI pin) before building; the decision says what happens either way. The mod API types read on 2026-10-31 were 2.1.316's; a probe module hooking `session.append{door=tool-result}`, `session.compact{trigger=precompute}` and calling `$.session.usage` passed `claude plugin validate` on 2.1.312 the same day (registration only, not behavior).

## R1 — What does a `session.append` tool-result row carry, and does a rewrite take? (decided route; one run to verify)

**Checked** (2.1.316 types): `session.append` fires once per row before it is stored; `next({ ...e, message })` rewrites `content` (text blocks, a `tool_result`'s `content` and `is_error`), "the model and the transcript file never read that form", and the screen may show the original just before. `e.origin` for a tool's result is `{ kind: 'tool', tool }`, the tool's name, `unknown` when no call of that id is found; it carries no input, so neither a command nor a `Read` path reaches this hook. `tool_use_id` of a `tool_result` is "put back" (not rewritable). The repository's modules use `tool.call` after `next` and read no result content (`plugins/git/hooks/register.tsx`), and 017 reads a stored `hook-context` row through `session.append` (`plugins/devcontainer/hooks/hook-lines.ts`'s `rowText`), which is this route's precedent here.

**Decision (the maintainer, 2026-10-31)**: `session.append` on `door: 'tool-result'` is the route (the first draft's `tool.call` route is dropped). *To verify* with a `claude plugin test` case and one headless run on the pin: the row's `origin.tool` and `content` shape for `Bash`, `WebFetch`, `Read`, `Grep` and an MCP tool; that a placeholder written by the hook is what the next request carries (a probe asks the model to repeat the result); which tools' results arrive by this door. If `origin.tool` is missing on the pin, the tool is recovered from `tool_use_id` against a `tool.call` hook's record.

## R2 — Can a result be rewritten before the model reads it? (resolved)

**Resolved from the types (2026-10-31)**: yes, by the route in R1; the original may flash on screen and may already be wherever the tool wrote it, so the toast says "treat as exposed" and never "erased". Confirmed on the pin by R1's probe at build time.

## R3 — Which kinds are safe to flag, and where does the pre-filter live?

**Checked**: `shared/secret-patterns.json` has eight kinds; `assignment` and `auth-header` already carry `skipIn: ["skill"]` because they match prose (016's "Found while building"). Results are more prose-and-example than a skill's body. `secret-mask.ts` declares `Scope = 'prompt' | 'skill'` and `LOOKS: Record<Scope, RegExp>` (line 78, read by `looksSecret`), so a new scope value needs a table entry.

**Decision**: the six high-confidence kinds (`private-key`, `github-token`, `api-key`, `aws-key`, `slack-token`, `url-credentials`) via `skipIn: ["result"]` on the other two; `Scope` gains `'result'` and `LOOKS.result` holds the pre-filter. A kind that proves noisy is switched off by a `skipIn` entry, not by a code change in the module.

## R4 — Does `userConfig` take a list or a string?

**To verify** on the pin with `claude plugin validate`: whether a `userConfig` field of type string (a comma-separated tool list) is accepted for `secretOutputTools`. If not, two booleans (`secretOutputRedactBash`, `secretOutputRedactWebFetch`) replace it and a person adding `Read` edits the module's list in a later change.
