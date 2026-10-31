# Feature Specification: Guard log

**Feature Branch**: `docs/mod-specs-2` (specification only; built on its own branch)

**Created**: 2026-10-31

**Status**: Implemented

**Input**: A Claude Code mod that remembers the guards' recent decisions and lists them on demand — a `/guards` command and an optional pane showing the last N denies and asks of the git, db, secrets, repo and devcontainer guards, with each one's reason, how often it fired and what command it stopped — so a guard that fires too often for work the person wants done is easy to spot. It reads what 015's presenter already sees at `classic.PreToolUse`; it never changes a decision.

**Depends on**: 015 (guard UX) — the five modules, `decorate` and `shared/guard-ux.ts` this feature extends; 012 (checkpoint mod) — layout, `check.sh` wiring, versioning and install, reused unchanged.

## Context and ground rules

- **What exists.** Each plugin's module hooks `classic.PreToolUse`, calls `next(e)`, and passes the result through `decorate(id, result)` (`shared/guard-ux.ts`, copied into `plugins/{git,db,secrets,repo,devcontainer}/hooks/guard-ux.ts`); `decorate` returns the same decision with more words, and a notice the module toasts (`plugins/git/hooks/register.tsx`). Nothing is kept: spec 015 records "Storage: none". The five guards themselves (`plugins/*/hooks/guard.sh`) write no log either (their `decide` prints one JSON line and exits).
- **What is new** (Planned): a record of each deny or ask the five modules already see, a reader, `/guards`, and a pane. The record is data about decisions, not a second decision path: it is written after `decorate`, inside a `.catch` that replays `next(e)`, so a failure can lose a record and never a decision (015's rule).
- **Guard id and plugin name differ once.** The id `containers` (`shared/guard-ux.ts:16`, `GuardId`) is the guard of the **`devcontainer`** plugin. A row's `guard` field says `containers`; every `dependencies` entry, `$.state.get` ref and `userConfig` says `devcontainer`.
- **What is kept, and what never is.** Per decision: time, guard id, `deny` or `ask`, the table entry it matched (`Entry.match`, or `generic`), the tool name, and the command's **verb only**. The reason shown by `/guards` is the table entry's own fixed text from `shared/guard-ux.ts`, never the guard's raw reason: the guards interpolate values into their reasons (`plugins/git/hooks/guard.sh:83-92` puts `$branch` in, `plugins/db/hooks/guard.sh:94-102` puts `$tool`, `plugins/secrets/hooks/guard.sh:80-83` puts `$sub`), so a raw reason is variable text and is not stored. A `generic` row (no table entry) stores no reason at all, only guard, decision and tool. Never the command line, its arguments, a path, a value or a pasted text: a command may carry a credential (a `psql` URL, a header), and a log must not become a place secrets rest. The verb rule (data-model.md) is tightened so a first word that is itself a token yields no verb, and a test builds every case in `shared/secret-cases.json` (credential shapes assembled from its `vars` fragments at run time; the patterns file has no fixtures) and feeds each one as a command's first word, as an argument and inside a URL, asserting none reaches a record.
- Owner: each of the five guard plugins writes **its own** `guardLog` atom (through the shared `guard-ux.ts`); the `project` plugin only reads them (`$.state.get` of each owner's atom, through `dependencies`) and draws (it owns the pane module, `plugins/project/hooks/register.tsx`, and `.ai-gent/`). Nothing in `project` writes a guard atom.
- The log is for the running session and whatever `$.state` keeps across a reload; a view across sessions is User Story 3, projected.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - `/guards` lists what the guards just did (Priority: P1)

The person types `/guards` and gets the last 20 decisions, newest first, and below them the entries that fired most:

```
guards — 9 decisions this session (2 deny · 7 ask) · newest 3 (/guards 20 shows more)
 14:02  ask   git      git push           git push publishes
 13:58  deny  git      git commit         hooks must not be bypassed
 13:41  ask   db       psql               bypasses dbrun
Most frequent
 7×  git:workflow  git push publishes             ask    (git push ×7)
 2×  db guard      bypasses dbrun                 ask    (psql ×2)
```

(The example is what `/guards 3` prints; the header always counts every row in the ring.)

**Why this priority**: today a guard's decision is one toast and a line in a transcript; nobody can tell whether a guard asks for the same harmless thing ten times a day.

**Independent Test**: a `claude plugin test plugins/project` case stubs each guard plugin's `guardLog` atom with fixture records (the `$.state.get` the module calls) and runs `command.run` for `guards`; it asserts the text, the order and the counts. A case in each guard plugin raises a `classic.PreToolUse` with a hook beneath answering `{ ask: 'git:workflow guard: git push publishes; …' }` and asserts one record exists with the verb `git push`, the entry `match` and no other part of the command (not the branch, not the reason's interpolated text).

**Acceptance Scenarios**:

1. **Given** a git ask, a git deny and a db ask this session, **When** `/guards` runs, **Then** three rows appear, newest first, each with guard, decision, verb and reason; and "Most frequent" lists each entry with its count.
2. **Given** `/guards 5`, **When** it runs, **Then** at most five rows (the argument is clamped to 1–200; the default is `guardLogLimit`).
3. **Given** a reason with no table entry (015's generic card), **When** recorded, **Then** the row stores no reason text and the entry column says `generic`.
4. **Given** no decision yet, **When** `/guards` runs, **Then** it says "no guard decisions this session".
5. **Given** a command carrying `ghp_` + 36 letters in its text, or as its first word, **When** the git guard asks about it, **Then** the record holds the verb only (none, for a first word that looks like a token) and `grep` of the stored state finds no part of the token.
6. **Given** the git guard asking about a commit on `main`, **When** recorded, **Then** the stored row holds the entry `match` and not the text `main` from the reason.

---

### User Story 2 - A pane that stays beside the chat (Priority: P2)

`/guards-pane` docks the same list in a pane that redraws when a decision is recorded; it is never opened by itself (a guard firing is not a reason to move the layout).

**Independent Test**: mount the pane on `terminal` and `desktop` with fixture records; append a record and assert the text changed without re-running the command.

**Acceptance Scenarios**:

1. **Given** the pane open and a new deny recorded, **When** the record lands, **Then** the pane shows the row within one render.
2. **Given** a surface that can't dock a pane, **When** `/guards-pane` runs, **Then** it prints "no pane here — /guards prints the list" and does nothing else.
3. **Given** `guardsPane` off in the `project` plugin, **When** the pane is requested, **Then** it says the pane is off (`/guards` still prints).

---

### User Story 3 - Across sessions (Priority: P3, projected)

Projected for after the first cut. `/guards --history` reads the Claude Code transcripts of this repository (the way `project:recap` does, `plugins/project/scripts/sessions.py`) and counts the guards' denies and asks there, so a guard that fires across weeks is visible. It is a read-only script, `plugins/project/scripts/guard_log.py` (stdlib only, secrets redacted by the shared patterns), run through `$.process.run`.

**Acceptance Scenarios**:

1. **Given** transcripts holding guard reasons from three sessions, **When** `/guards --history` runs, **Then** it prints per-entry counts and the dates of first and last occurrence.
2. **Given** no transcripts, **When** it runs, **Then** it says so and shows nothing else.

### Edge Cases

- **Several guards answered one call** (db asks, secrets denies): each plugin's module records only its own guard's part (015, FR-005); the pane shows both rows.
- **The deny reason is wrapped** (`PreToolUse:Bash hook error: git:workflow guard: …`, verified in 015): the record keeps the text after the guard's marker, as `decorate` finds it.
- **No decision** (the guard didn't run, or timed out): nothing is recorded; the log never fills in a decision.
- **The person's answer to an ask** is not visible at `classic.PreToolUse`; a row says `asked`. Research R2 checks whether a later `tool.call` for the same call can mark it `ran`; if so the row says so, if not it never claims an outcome.
- **Bounded**: the record is a ring of the last 200 rows (`guardLog` atom shape `guard-log-1`); the oldest drop first.
- **A reload** keeps what `$.state` keeps and repeats nothing (no toasts are added by this feature).
- **Headless (`claude -p`)**: records are written; `/guards` answers text; no pane.
- **A guard plugin not installed** (or its `guardLog` off): its `$.state.get` fails or returns nothing; `project` catches it per plugin, lists the others' rows and ends the header with `not reporting: db, repo` (plugin names, from the five in its `dependencies`). `/guards` never fails because one plugin is absent. Whether a missing dependency is an error at load is verified in research R1.
- **Narrow terminal**: rows truncate the reason to the width; the verb and decision are always whole.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Each of the five modules MUST record one row per deny or ask that `decorate` recognizes as its own guard's, after calling `next(e)`, and MUST NOT change the result for the record's sake.
- **FR-002**: A row MUST hold only: time, guard id, `deny` or `ask`, the matched table entry's `match` (or `generic`), the tool name, and the command verb as defined by the verb rule. It MUST NOT hold the guard's raw reason, which interpolates values.
- **FR-003**: No command line, argument, path or value MUST be stored, shown or logged; a test runs every `shared/secret-cases.json` credential shape (built from its fragments) through a command, as a first word, an argument and a URL, and asserts the record is clean.
- **FR-004**: The record MUST be a bounded ring in `$.state` (200 rows) and MUST NOT write a file, `$.store` or anything outside the engine's state.
- **FR-005**: `/guards [N]` MUST print the newest N rows (default `guardLogLimit`, 20) and the entries with the most rows; it MUST NOT run a command or call a model.
- **FR-006**: The summary MUST state counts only; it MUST NOT call a guard "over-strict" or suggest loosening one. What to do with a frequent entry is the maintainer's decision (`shared/guard-ux.ts`, `guard.sh`).
- **FR-007**: A failure in recording MUST NOT change a decision: the `.catch` on the hook replays `next(e)` (015's rule).
- **FR-008**: `guardLog` (boolean, default true) in each guard plugin's `userConfig` turns that plugin's recording off; `guardLogLimit` (number, default 20) and `guardsPane` (boolean, default true) in `project`. `project`'s `plugin.json` lists `git`, `db`, `secrets`, `repo` and `devcontainer` under `dependencies`, and `/guards` must work with any subset present (edge cases).
- **FR-009**: No change to any `guard.sh`, `opencode/guard/`, the shell guard tests or the parity test.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test` passes for the five guard plugins (a record per deny and per ask, none for a decision that isn't theirs, a clean record for every secret fixture) and for `project` (`/guards`, the pane on `terminal` and `desktop`).
- **SC-002**: `scripts/test-guard.sh`, `test-db-guard.sh`, `test-secrets-guard.sh`, `test-repo-guard.sh`, `test-containers-guard.sh` and `opencode/guard/test/rules.test.mjs` pass unchanged.
- **SC-003**: With `guardLog` off everywhere, the `--debug` log shows the same decisions for the parity vectors as with it on.

## Out of scope

- Changing, suppressing or auto-tuning any guard, or any rule it enforces.
- OpenCode's guard (`opencode/guard/index.ts`).
- Keeping the command lines, or any log outside the engine's state (a file under `.ai-gent/`, a database).
- Telemetry or sending the log anywhere.

## Decisions (2026-10-31, with the maintainer)

- The log is `$.state`, not a file: no mod in this repository writes a file today (013: "nothing persisted"; 014: "the mod writes nothing"; 016 FR-006: no files), and a log of blocked commands is the kind of file that ends up committed or pasted. The cost is that it lives for the session and its reloads; across sessions is User Story 3 from transcripts.
- Records carry the matched entry and the verb, never the command or the guard's raw reason (see ground rules).
- `/guards-pane` never opens by itself.
- One owner reads all five: `project` lists `git`, `db`, `secrets`, `repo` and `devcontainer` under `dependencies` in its `plugin.json`; each guard mod writes only its own `guardLog` atom, declared in its own `types/index.d.ts`; `project` reads them with `$.state.get` and serves `/guards` and `/guards-pane`. No shared atom and no per-plugin command (the fallbacks the first draft listed are dropped).
- Behavior is optional (decided 2026-10-31 (the maintainer accepted the default)): the five dependencies are declared, but `/guards` degrades per plugin (edge cases) and says which are not reporting, rather than failing.

## Assumptions

- [ASSUMPTION: `project` can `$.state.get` an atom declared by another plugin once that plugin is under its `dependencies` (the engine lays each dependency's contract into `.claude-plugin/types/<plugin>/index.d.ts`; `claude plugin validate` on 2.1.312 accepted a `dependencies` entry for a plugin that was not installed, with no error or warning, in a probe on 2026-10-31). What `$.state.get` returns for an atom whose plugin is absent at run time is not verified: research R1.]
- [ASSUMPTION: the `classic.PreToolUse` event carries `tool` and `command` for Bash (015, "A module's event carries `tool` and `command`") and, for the db guard's other matcher tools (`Read`, `Grep`, `Glob`, `Edit`, `Write`, `MultiEdit`, `NotebookEdit`), at least `tool`; a missing `command` is recorded as no verb.]
- [ASSUMPTION: the guard reason prefixes and `decorate`'s marker matching stay as they are (015).]
