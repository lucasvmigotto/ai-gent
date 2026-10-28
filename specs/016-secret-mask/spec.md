# Feature Specification: Secret mask for what the person sends

**Feature Branch**: `docs/mod-specs` (specification only; built on its own branch)

**Created**: 2026-10-28

**Status**: Draft

**Input**: A Claude Code mod that masks credentials the person pastes — in a prompt, in a slash command's arguments, in a skill's rendered body — before they are recorded in the transcript or reach the model, and tells the person how to store the value in pass instead (`shared/secrets.md`). Uses the masking route the plugin-authoring reference describes: `prompt.submit` for prompt text, `command.run` for arguments, `skill.prompt` for a skill's body.

**Depends on**: 012 (checkpoint mod) — module layout, `check.sh` wiring, versioning and install, reused unchanged.

## Context and ground rules

- `shared/secrets.md` already says a secret is never pasted into the conversation. This mod is a safety net for when it happens anyway; it is not part of that contract, and no skill relies on it.
- **Which patterns.** The `secrets` guard (`plugins/secrets/hooks/guard.sh`) matches *commands* (`pass show`, `gpg` on the store, `pass-run … -- env`), not secret *values*, so it has no patterns to reuse. The value patterns in the repository are `SECRET_PATTERNS` in `plugins/project/scripts/sessions.py` (used by `project:recap` to redact transcripts, tested by `test_redaction` in `plugins/project/scripts/test_scripts.py`): private key blocks, `*PASSWORD|SECRET|TOKEN|API_KEY|ACCESS_KEY|PAT = value`, `Bearer`/`Basic` credentials, `gh[pousr]_…`, `github_pat_…`, `sk-…`, `AKIA…`, `xox[baprs]-…`, and `scheme://user:password@`. `dbrun.py` and `reporun.py` redact the exact values they read, which a mod can't know. This feature reuses the `sessions.py` list, moved to one shared file both read.
- Owner: the `secrets` plugin. Its module `plugins/secrets/hooks/register.tsx` is shared with 015's secrets part.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A pasted token never reaches the transcript (Priority: P1)

The person pastes `here's my token ghp_…` into the prompt. What is recorded and sent is `here's my token <token>`, and a toast says one GitHub token was masked and how to store it.

**Why this priority**: once a value is in the transcript it is on disk under `~/.claude*/projects/` and in the provider's request.

**Independent Test**: a `claude plugin test` case raises `prompt.submit` with a fixture text per pattern and asserts the text `next` received, and that the stored row (`mock.session(on)`) holds no fixture secret.

**Acceptance Scenarios**:

1. **Given** a prompt with `ghp_` followed by 36 letters, **When** submitted, **Then** the recorded prompt holds `<token>` instead, and a toast says "masked 1 token — store it with `pass insert ai-gent/github/pat` in your own terminal (secrets:pass)".
2. **Given** `DB_PASSWORD=hunter22 npm test` in a prompt, **When** submitted, **Then** it is recorded as `DB_PASSWORD=<redacted> npm test` (the same output `sessions.redact` gives).
3. **Given** a pasted `-----BEGIN OPENSSH PRIVATE KEY-----` block, **When** submitted, **Then** the whole block is `<private key>`.
4. **Given** a prompt with no match, **When** submitted, **Then** `next(e)` gets `e` unchanged and nothing is shown.
5. **Given** a hidden prompt that starts a turn of its own (`isMeta`, which the engine doesn't let a hook rewrite), **When** submitted, **Then** the mod passes it on and logs to debug that it could not mask it.

---

### User Story 2 - Slash command arguments and skill bodies (Priority: P1)

`prompt.submit` can't rewrite a slash command's line, so arguments are masked at `command.run` and a skill's rendered body at `skill.prompt`.

**Independent Test**: raise `command.run` with `args` holding a fixture secret and `skill.prompt` with `text` holding one; assert the rewritten values.

**Acceptance Scenarios**:

1. **Given** `/db:connect SHOP_DEV password=abc123secret`, **When** the command runs, **Then** both of the command's rows and its prompt-history line hold `password=<redacted>`.
2. **Given** a skill whose rendered body includes a pasted `Bearer eyJ…`, **When** `skill.prompt` fires, **Then** the body sent is `Bearer <redacted>`.
3. **Given** `prompt.submit` carries `command`, **When** the mod sees it, **Then** it does not attempt a rewrite of `text` (the engine would ignore it); masking happened at `command.run`.

---

### User Story 3 - One list of patterns (Priority: P2)

The patterns live in one file read by `sessions.py` and by the mod, so `project:recap`'s redaction and this mask can't drift.

**Acceptance Scenarios**:

1. **Given** `shared/secret-patterns.json`, **When** `test_scripts.py` and `claude plugin test plugins/secrets` run, **Then** both apply every pattern to the same fixture strings and agree on the output.

### Edge Cases

- **False positives**: `PASSWORD: see the vault` in prose is masked too (`PASSWORD` followed by `:` and a value). Accepted: over-masking a prompt costs a re-phrase; under-masking leaks. A `userConfig` boolean `maskAssignments` (default true) turns off the `NAME=value` rule alone.
- **Code fences**: masked like any other text; a secret in a code block is still a secret.
- **Images and documents**: not scanned (a hook can't change image blocks); out of scope.
- **Very long pastes**: patterns run on the whole text; a pre-filter (cheap substring checks for `BEGIN`, `_`, `=`, `:`, `Bearer`, `Basic`, `://`) skips texts that can't match.
- **A hook that throws** at `prompt.submit` (a gating site): its `.catch` answers `{ drop }` when the pre-filter matched (the text may hold a secret; the person is told it was held back and can resend), and passes the text on when it didn't. See the open question.
- **Hot reload**: patterns are compiled at `register`; nothing else is kept.
- **Headless (`claude -p`)**: masking runs the same; the toast goes to the debug log.
- **Non-terminal surfaces**: only a toast; nothing drawn.
- **Narrow terminal**: the toast is one line; the `pass insert` hint is in the transcript notice if it doesn't fit.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The mod MUST mask at `prompt.submit` (when there is no `command`), `command.run` (`args`) and `skill.prompt` (`text`), before `next`.
- **FR-002**: Patterns MUST come from `shared/secret-patterns.json` (pattern source, flags, replacement), linked into `plugins/secrets/references/` and `plugins/project/references/`; `sessions.py` MUST read the same file and keep its current output (`test_redaction` unchanged).
- **FR-003**: The JSON MUST use only regex features both Python `re` and JavaScript `RegExp` accept, with flags as separate fields (no inline `(?i)`), and replacements as group references both sides translate (`\1` → `$1`).
- **FR-004**: Replacement tokens MUST be the ones `sessions.py` uses (`<redacted>`, `<token>`, `<private key>`).
- **FR-005**: The person MUST see what kind was masked and how many, never any part of the value, plus the `pass insert ai-gent/<area>/<name>` hint from `shared/secrets.md` when the kind maps to an entry (GitHub → `ai-gent/github/pat`, etc.).
- **FR-006**: The mod MUST NOT log, store or toast any matched text, and MUST NOT write `$.store` or files.
- **FR-007**: A `userConfig` boolean `secretMask` (default true) MUST turn the feature off.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/secrets` passes with one fixture per pattern on all three events, plus no-match, `isMeta`, `command`, and throw cases.
- **SC-002**: `python3 -m unittest plugins/project/scripts/test_scripts.py` passes unchanged after `sessions.py` reads the shared file.
- **SC-003**: A cross-check test runs the JSON's patterns in Python and in the mod against the same fixtures and gets identical output.

## Out of scope

- Masking tool results or model output (`session.append`, `process.spawn`); a later feature could reuse the same patterns.
- Detecting arbitrary high-entropy strings.
- Changing `plugins/secrets/hooks/guard.sh`, `shared/secrets.md`'s rules, or the runners' own redaction.

## Decisions (2026-10-28, with the maintainer)

- When masking throws at `prompt.submit`: drop the prompt only if the cheap pre-filter matched, else pass it through.
- `SECRET_PATTERNS` moves out of `plugins/project/scripts/sessions.py` into `shared/secret-patterns.json`, the one source both read, with a test that both give the same output (bumps `project`).
- Found secrets are masked and sent with no question; a toast names the kind and the `pass insert` hint.

## Assumptions

- [ASSUMPTION: `prompt.submit`'s rewrite is written to the row before the transcript stores it, and the queue record follows (2.1.312 reference, "The rows a conversation keeps").]
