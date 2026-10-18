Reconstructed by project:introspec on 2026-10-18 from 384c771

# Feature Specification: Secrets through pass (passstore)

**Feature Branch**: `docs/introspec-site-runners` (reconstructed; the code already exists on `main`)

**Created**: 2026-10-18

**Status**: Draft

**Input**: Reverse-engineered from `shared/passstore.py` (72 lines), symlinked as `plugins/db/scripts/passstore.py` and `plugins/repo/scripts/passstore.py`, and the contract `shared/secrets.md`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A runner reads a credential without exposing it (Priority: P1)

`dbrun` or `reporun` needs a password or token: it checks that the entry exists without decrypting, then reads it through `pass show` only for the call that needs it; the value never reaches output.

**Why this priority**: the credential contract (`shared/secrets.md`) for every runner.

**Independent Test**: dbrun's `test_sources_in_order`, `test_bad_or_unreadable_entry`; reporun's `test_detect_never_decrypts`, `test_missing_entry` (fake `pass` on PATH, temporary `PASSWORD_STORE_DIR`).

**Acceptance Scenarios**:

1. **Given** entry `ai-gent/db/app`, **When** `has()` is called, **Then** it checks `<store>/ai-gent/db/app.gpg` exists and decrypts nothing [OBSERVED: shared/passstore.py:38-40].
2. **Given** an existing entry, **When** `read()` is called, **Then** `pass show` runs with stdin closed and a 90 s timeout; the first line is the secret, the rest are notes [OBSERVED: passstore.py:43-59].
3. **Given** a locked store or empty entry, **When** `read()` is called, **Then** `StoreError` says how to unlock it in the user's own terminal, never including a value [OBSERVED: passstore.py:55-58].
4. **Given** a name with a leading `-` or `/`, or a `..` segment, **When** any call is made, **Then** it is refused [OBSERVED: passstore.py:17,32-35].

---

### User Story 2 - Move a plain-text secret into pass (Priority: P2)

`dbrun secrets migrate` writes a value as a new entry with `pass insert --multiline`; it never overwrites an existing entry.

**Independent Test**: dbrun `test_migrate`.

**Acceptance Scenarios**:

1. **Given** an absent entry, **When** `write()` is called, **Then** `pass insert --multiline` receives the value on stdin and the entry must exist afterwards [OBSERVED: passstore.py:62-72].
2. **Given** an existing entry, **When** `write()` is called, **Then** `StoreError("… already exists")` [OBSERVED: passstore.py:67-68].

### Edge Cases

- The prefix is `AI_GENT_PASS_PREFIX` (default `ai-gent`); the store is `PASSWORD_STORE_DIR` (default `~/.password-store`) [OBSERVED: passstore.py:24-29; dbrun test_prefix_override].
- `has()` checks only the file; a store with a `.gpg-id` for a missing key passes `has()` and fails `read()` [INFERRED: passstore.py:40 → existence ≠ readability; the runners report the `read()` error].
- The insert failure message carries the last 300 characters of `pass`'s stderr [OBSERVED: passstore.py:72] [INFERRED: pass doesn't echo the value on stderr, so no leak; confirm with a failing gpg].

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Values MUST only be returned to the caller, never printed or put in a message.
- **FR-002**: Entry names MUST match `ENTRY_RE` and contain no `..` segment.
- **FR-003**: Writes MUST NOT overwrite.
- **FR-004**: The module MUST be imported only on the host, never inside a runner's client container [OBSERVED: passstore.py:2-3; plugins/db/scripts/dbrun.py:288-292].

### Key Entities

- **Entry** — `<prefix>/<area>/<name>` (`ai-gent/db/<profile>`, `ai-gent/github/pat`, …); first line secret, other lines notes (`email:`, `username:`…) used by reporun [OBSERVED: plugins/repo/scripts/reporun.py:119-138].

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: No test output contains a fake secret value [OBSERVED: reporun test_token_in_env_only_and_redacted; dbrun test_listing_shows_source_never_value].

## Assumptions

- [ASSUMPTION: the module has no tests of its own on purpose; it is exercised through the dbrun and reporun suites.]
