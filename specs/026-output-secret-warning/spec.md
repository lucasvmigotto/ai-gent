# Feature Specification: Secret warning and redaction for tool output

**Feature Branch**: `docs/mod-specs-2` (specification only; built on its own branch)

**Created**: 2026-10-31

**Status**: Accepted

**Input**: A Claude Code mod that tells the person when a tool's result looks like it holds a credential — a `cat .env`, an `env` dump, a log line, a fetched page, a `Read` of a credentials file — using the same value patterns the prompt mask uses, says what to do (rotate it, and keep the new one in pass under `ai-gent/<area>/<name>`), and, for the results of `Bash` and `WebFetch`, **replaces the matched value with a placeholder in the stored row**, so neither the model nor the transcript file reads it. It is a last line of defense for what the secrets guard (which keeps `pass` values out) and the prompt mask (which covers what the person types) can't see. For other tools (`Read`, `Grep`, MCP) it warns and leaves the result intact.

**Depends on**: 016 (secret mask) — `shared/secret-patterns.json`, `plugins/secrets/hooks/secret-mask.ts`, the module and its generated `secret-data.ts`; 012 (checkpoint mod) — layout, `check.sh` wiring, versioning and install.

## Context and ground rules

- **The gap.** `shared/secrets.md` says a secret is never "pasted into the conversation" or put "in the agent's shell environment", and 016 masks what the person sends; it lists tool results as out of scope ("Masking tool results or model output (`session.append`, `process.spawn`); a later feature could reuse the same patterns"). The `secrets` guard (`plugins/secrets/hooks/guard.sh`) denies commands that would show a value from pass, but a value can still arrive in a result: a repository's `.env`, `~/.aws/credentials`, `printenv`, a test log, a page.
- **The route (verified in the kit's types on 2026-10-31).** A hook on `session.append` with the matcher `{ door: 'tool-result' }` sees each tool-result row before it is stored. `next({ ...e, message })` rewrites the row's `content` — a `tool_result`'s `content` and `is_error`, text blocks — and "the model and the transcript file never read that form"; the screen, an SDK stream or Remote Control may show the original for an instant first. The row's `e.origin` is `{ kind: 'tool', tool }`, the tool's **name** (`unknown` when no call of that id is found); it carries no input, so the `Read` path or the command is **not** available to this hook. The same probe of `session.append{door=tool-result}`, `session.compact` and `$.session.usage` passed `claude plugin validate` on the CI pin, Claude Code 2.1.312 (2026-10-31); `claude plugin test` cases confirm the behavior at build time (R1).
- **What redaction can and can't do.** It stops the value reaching the model's next request and the transcript. It cannot recall what already happened: the tool ran, the screen may have drawn the original, the process may have written it elsewhere (a log, a file), and a value the person or a tool echoed in another row (a `Read` result) is another row. So the toast still says **the value should be treated as exposed** and to rotate it, and never says it was erased.
- **Why not every tool.** A model that reads a file and sees a placeholder where a value was may write the placeholder back into that file, corrupting it; `Read` and `Grep` results are file content the model acts on. The first cut therefore redacts `Bash` and `WebFetch` only (command output and fetched pages, where a value is least expected and nothing is edited from it) and, for every other tool (`Read`, `Grep`, `Glob`, MCP tools), warns and leaves the row intact. Which tools redact is a `userConfig` list (FR-006), so a person can opt a tool out or in.
- **Never echo.** The toast, the debug log, `$.state` and every test failure message hold the **kind**, a **count** and the **tool's name**, never any part of the value: not a prefix, not the last four characters, not a hash. The placeholder itself carries only the kind: `<github-token>`.
- **Which patterns.** The 016 list, but only its **high-confidence** kinds: private keys, GitHub tokens, `sk-` keys, AWS access keys, Slack tokens and URL passwords. The `NAME=value` and `Bearer|Basic` kinds are left out of a result, as 016 leaves them out of a skill's body (`skipIn`), because results are full of prose, examples and code (`Basic authentication`, `DB_PASSWORD=<value>`). A planned `skipIn: ["result"]` on those two patterns expresses it.
- **What the pre-filter must not break.** `scripts`, tests and docs in this repository hold token-shaped fixtures built from fragments (`shared/secret-cases.json`); a `Bash` result that prints such a fixture is redacted like any other, which is acceptable (the fragments themselves don't match).
- Owner: the `secrets` plugin; its module `plugins/secrets/hooks/register.tsx` (shared with 015's and 016's parts).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A leaked credential in a Bash or WebFetch result is flagged and redacted (Priority: P1)

The agent runs `cat .env` and the output holds `AKIA` + 16 characters. The row the model reads shows `<aws-key>` where the value was, and a toast says: "output-secret: the result of Bash held 1 AWS key — redacted in what Claude reads; treat it as exposed anyway: rotate it with AWS, then keep the new one in pass (`secrets:pass`)."

**Why this priority**: a credential in a result reaches the model and the transcript with nothing in the way, and the person often isn't watching the output scroll by.

**Independent Test**: a `claude plugin test plugins/secrets` case raises `session.append` with `door: 'tool-result'`, `origin: { kind: 'tool', tool: 'Bash' }` and a `tool_result` block whose text holds a fixture per kind (assembled from fragments at run time, 016's rule), and asserts that the row passed to `next` holds the placeholder and none of the fixture, that one toast names kind, count and tool, and that no output holds a fragment of the fixture.

**Acceptance Scenarios**:

1. **Given** a `Bash` result holding `ghp_` followed by 36 letters, **When** the row is appended, **Then** the stored row holds `<github-token>` in its place, and one toast says 1 GitHub token, that it was redacted in what Claude reads, to treat it as exposed, to rotate it and to run `pass insert ai-gent/github/pat` — the entry in `shared/secret-patterns.json`.
2. **Given** a kind with no entry in the patterns file (AWS, `sk-`, Slack, URL password, private key), **When** flagged, **Then** the toast says to rotate it at the service that issued it and points at `secrets:pass` and `shared/secrets.md`'s section "Entry names", naming no invented entry.
3. **Given** a result holding two tokens and a URL password, **When** flagged, **Then** all three are replaced, and one toast lists all kinds with counts (never one toast per match).
4. **Given** a result with `DB_PASSWORD=<value>` or `Bearer authentication`, **When** appended, **Then** the row is unchanged and nothing is shown (the low-confidence kinds don't apply to results).
5. **Given** a result that matches nothing, **When** appended, **Then** the row is passed on byte for byte unchanged and nothing is shown.
6. **Given** `secretOutputWarning` off, **When** a leaked token appears, **Then** the row is unchanged and nothing is shown.
7. **Given** a `WebFetch` result holding a Slack token, **When** appended, **Then** it is redacted like a `Bash` result.

---

### User Story 2 - Other tools get the warning, never a rewrite (Priority: P1)

A `Read` of `~/.aws/credentials` or a `Grep` that hits a key returns a result holding a credential. The row is left exactly as the tool returned it (so a later `Edit` of that file stays correct), and the toast says: "output-secret: the result of Read held 1 AWS key — it was not redacted (file content the agent may edit); treat it as exposed: rotate it…".

**Independent Test**: the same case with `origin.tool` `Read`, `Grep` and an MCP tool name asserts the row is unchanged and the toast text says "not redacted" and holds no fragment of the value.

**Acceptance Scenarios**:

1. **Given** a `Read` result with a token, **When** appended, **Then** the stored row equals the received row and one toast says it was not redacted.
2. **Given** `secretOutputTools` listing `Read`, **When** the same result arrives, **Then** it is redacted like a `Bash` result (the person's choice, documented as risking a placeholder written back into a file).
3. **Given** `origin.tool` is `unknown`, **When** a token is found, **Then** the row is left unchanged and the toast names the tool as `unknown` (the mod never redacts a row whose tool it can't name).

---

### User Story 3 - Quiet when it repeats (Priority: P2)

Reading the same `.env` five times, or a log with fifty tokens, must not bury the person in toasts. (Redaction still happens every time; only the toasts are rate-limited.)

**Independent Test**: raise six appended rows with matching results inside a minute; assert every row is redacted (or left, per tool), at most three toasts and one final summary toast, never a toast per row.

**Acceptance Scenarios**:

1. **Given** the same kind flagged in the same turn by two rows, **When** the second is appended, **Then** it joins the first toast's count instead of raising another.
2. **Given** more than three warnings in a minute, **When** another matches, **Then** it is counted, and one summary toast ("N more results held credentials this session — treat them as exposed too") is shown once, with no value in it.

### Edge Cases

- **Images, PDFs and binary results**: not scanned and never rewritten (a hook may drop or move media blocks but not change them, per the types); text blocks and a `tool_result`'s text `content` only.
- **Very large results**: the scan stops after the first 256 KiB of a result's text; the rest is passed on unscanned and the toast says "scanned the first 256 KiB" if it did. A cheap substring pre-filter (`BEGIN`, `gh`, `github_pat_`, `sk-`, `AKIA`, `xox`, `://`) skips results that can't match. The pre-filter is a new `LOOKS.result` entry: `secret-mask.ts` declares `LOOKS: Record<Scope, RegExp>` (line 78), so adding `'result'` to `Scope` fails to compile until the table has its entry.
- **Redaction itself fails** (the compiled patterns throw, or the rewritten row is refused): decided 2026-10-31 (the maintainer accepted the default): **fail open** — the row is stored as received through the hook's `.catch` replaying `next(e)`, and a loud toast says "output-secret: could not check or redact this result — treat any credential in it as exposed". A failed scan loses a row's protection, never the row.
- **Results of the secrets plugin's own commands**: `pass ls` and the like show names only; nothing special is needed.
- **Fixtures and docs in a repository**: a `Read` of `shared/secret-cases.json` shows fragments, which don't match; a token-shaped example in a `Read` result is flagged and left intact (a warning costs a glance); in a `Bash` result it is redacted, and `secretOutputWarning` or the tool list can turn that off.
- **The person pasted the token and the agent echoed it back**: 016 already masked the input, so an echo carries the mask, not the value.
- **Headless (`claude -p`)**: the toast goes to the debug log with kind and count only; redaction still applies.
- **Narrow terminal**: the toast is one line; the rotate/pass hint is also in the transcript notice if it doesn't fit.
- **Hot reload**: patterns are compiled once (016's `loadRules`); the toast-rate counters live in `$.state` and reset with the session.
- **Subagents' results**: a row's `agentId` is carried unchanged; a subagent's tool-result rows are handled the same way.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The mod MUST hook `session.append` with `{ door: 'tool-result' }`, scan the row's text content, and for a row whose `origin.tool` is in the redact list replace each match of a high-confidence kind with `<kind>` through `next({ ...e, message })`; for every other row it MUST pass the row on unchanged. A row with no match MUST be passed on unchanged and untouched.
- **FR-002**: Patterns MUST come from `shared/secret-patterns.json` through 016's `compile`, using only the kinds not marked `skipIn: ["result"]`; no pattern is copied into the module. `secret-mask.ts`'s `Scope` gains `'result'` and its `LOOKS` table gains the matching entry (decided in the plan: `LOOKS.result` holds the pre-filter, so `looksSecret` works for results too).
- **FR-003**: The toast, the debug log, `$.state` and test output MUST hold only kinds, counts and the tool's name (from `e.origin.tool`), never any part of a match and never a `Read` path or a command (the row does not carry them); tests assert it on every kind.
- **FR-004**: The toast MUST say the value is to be treated as exposed, to rotate it, and where its replacement belongs (the entry from the patterns file when there is one, otherwise `secrets:pass` and `shared/secrets.md`); for a redacted row it says "redacted in what Claude reads", for an unredacted one "not redacted", and it MUST NOT say or imply the value was erased or that nothing was exposed.
- **FR-005**: At most one toast per appended row, at most three per minute, then a single summary; the rate limit never skips a redaction.
- **FR-006**: `userConfig`: `secretOutputWarning` (boolean, default true) turns the whole feature off; `secretOutputRedact` (boolean, default true) turns redaction off (warn-only); `secretOutputTools` (a comma-separated list of tool names, default `Bash,WebFetch`) names the tools whose results are redacted — the per-tool opt-out and opt-in. A tool not listed is warn-only. [If `userConfig` takes no string type on the pin, two booleans `secretOutputRedactBash` and `secretOutputRedactWebFetch` replace the list; R4 verifies.]
- **FR-007**: The mod MUST NOT write `$.store` or files, and MUST NOT call a model or the network.
- **FR-008**: `shared/secret-patterns.json` gains the optional `skipIn` value `"result"` on `assignment` and `auth-header`, with `sessions.py`'s output unchanged and `secret-data.ts` regenerated (`python3 scripts/consistency.py --sync-secret-data`).
- **FR-009**: On any failure of the scan or the rewrite the hook MUST fail open (the row stored as received) with the loud toast of the edge case; it MUST NOT drop or delay a row.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/secrets` passes with: one redaction fixture per high-confidence kind for `Bash` and `WebFetch`; the same fixtures left intact (with a toast) for `Read`, `Grep` and an MCP tool name; no-match; the two low-confidence kinds (silent); `unknown` tool (left intact); large-result; repeat-limit; a throwing scan (row stored as received, loud toast); `secretOutputRedact` off; the tool list changed; and a no-value-in-any-output check.
- **SC-002**: `python3 -m unittest plugins/project/scripts/test_scripts.py` (the redaction cases) and `scripts/test-secrets-guard.sh` pass. Adding `'result'` to the data does not change `sessions.py`'s output; if `plugins/project/scripts/test_scripts.py`'s scope handling (it knows only `skill` today, near line 190) needs a case for the new `skipIn` value, that is a small additive test change recorded in the plan, not a change to existing cases.
- **SC-003**: A check in `scripts/consistency.py` fails if `secret-data.ts` differs from the patterns and cases (the existing whole-file equality check, `check_secret_data`, already covers `skipIn` because the twin is generated from the JSON); the plan adds a case in `scripts/test_consistency.py` that a `skipIn: ["result"]` entry survives the regeneration.

## Out of scope

- Redacting `Read`, `Grep`, `Glob` or MCP results by default (listed in `secretOutputTools` only by the person's choice), erasing a transcript row already written, or rotating a credential: rotation is the person's, at the service.
- Detecting arbitrary high-entropy strings or names like `AWS_SECRET_ACCESS_KEY` in prose.
- Stopping the tool before it runs (that's the guard's job, and it can't know what a file holds).
- Changing `plugins/secrets/hooks/guard.sh`, `shared/secrets.md`'s rules, or 016's masking.

## Decisions (2026-10-31, with the maintainer)

- The mod warns and redacts: a `session.append` hook on `door: 'tool-result'` replaces the matched value with a `<kind>` placeholder in the stored row, which neither the model nor the transcript file reads in its original form. The screen may show the original for an instant (types of Claude Code 2.1.316; the events passed `claude plugin validate` on the 2.1.312 pin). The first draft's warn-only `tool.call` route is dropped; hiding the result from the model is in the first cut, no longer projected.
- **First cut redacts `Bash` and `WebFetch` only.** `Read`, `Grep` and every other tool get the toast and are left intact so file edits stay correct (a placeholder read from a file could be written back into it). The list is a `userConfig` option (FR-006).
- High-confidence kinds only, via a new `skipIn` value `result`, so the one shared list stays the single source.
- Never a hash or fingerprint of a match: repeats are limited by count and rate, not by remembering values.
- The toast still says to treat the value as exposed and to rotate it, and never shows any part of a match.
- **decided 2026-10-31 (the maintainer accepted the default): if redaction itself fails, fail open** with a loud toast, rather than hold the row back (holding a row back could break a tool run).

## Assumptions

- [ASSUMPTION: a `session.append` row for a tool result carries the tool's name in `e.origin.tool` (the 2.1.316 types say so, `unknown` when the call isn't found) and its text in the `tool_result` block's `content`; on 2.1.312 only the hook's registration is validated, so a `claude plugin test` case checks the origin and content shape first (plan, task 1; R1).]
- [ASSUMPTION: the hook sees `WebFetch` and MCP results as well as `Bash`, `Read` and `Grep`; if a tool's rows don't arrive by this door, the spec says which tools are covered.]
- [ASSUMPTION: `userConfig` accepts a string option; otherwise two booleans (FR-006; R4).]
- Fallback if `e.origin.tool` is absent on the pin: a `tool.call` hook (after `next`) records `tool_use_id` to tool name in an atom, and the `session.append` hook looks the tool up by the `tool_use_id` its `tool_result` block carries; the Bash/WebFetch split then does not depend on `origin`. Found 2026-10-31: the 2.1.312 pin already runs `session.append` hooks (the installed `git` and `devcontainer` mods register them), and `claude plugin validate` accepts `userConfig` options of `type: string` there. A headless `claude -p --plugin-dir` run does not load a hooks module, so the `origin` shape needs an interactive session or a `claude plugin test` case (plan task 1).
