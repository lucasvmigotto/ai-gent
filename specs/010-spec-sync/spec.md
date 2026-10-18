Reconstructed by project:introspec on 2026-10-18 from 384c771

# Feature Specification: Specs-to-tracker planner (specsync)

**Feature Branch**: `docs/introspec-site-runners` (reconstructed; the code already exists on `main`)

**Created**: 2026-10-18

**Status**: Draft

**Input**: Reverse-engineered from `plugins/repo/scripts/specsync.py` (438 lines) and `plugins/repo/scripts/test_specsync.py`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Plan a feature's tracker items (Priority: P1)

`specsync plan <NNN>` reads `specs/NNN-*/spec.md`, `tasks.md`, the feature's row in `specs/README.md` and its `tracker.md`, and prints one item per feature and one per user story — title, body with a hidden marker, tasks as a checklist, target state — and what to do with each (create, update, state, none).

**Why this priority**: what `repo:sync` applies through `reporun batch`.

**Independent Test**: `test_items_and_story_assignment`, `test_states`, `test_all_tasks_checked_is_review`.

**Acceptance Scenarios**:

1. **Given** `### User Story N - Title (Priority: Pn)` headings, **When** planned, **Then** each becomes a story item `[NNN] USn: Title` [OBSERVED: plugins/repo/scripts/specsync.py:37,63-85,334-342].
2. **Given** no such headings but a `## Stories` numbered list (`1. **Title**: …`), **When** planned, **Then** the list's entries are the stories (introspec's shape) [OBSERVED: specsync.py:88-113; test_introspec_story_list].
3. **Given** tasks tagged `[USn]` or under a "User Story n" phase heading, **When** planned, **Then** they go to that story's checklist; others to the feature body [OBSERVED: specsync.py:116-138,309-322].
4. **Given** README statuses, **When** the feature state is computed, **Then** Planned → todo, In progress/Partial → doing, Implemented → review, Verified → done, the least advanced side wins; without statuses, from the tasks [OBSERVED: specsync.py:32-34,166-192; test_status_with_notes_after_it].
5. **Given** a story, **When** its state is computed, **Then** done if the feature is done, else from its tasks (todo / doing / review) [OBSERVED: specsync.py:328].
6. **Given** a tracker item whose title+body hash or state changed, **When** planned, **Then** its action is `update`, `state` or `update+state` [OBSERVED: specsync.py:285-286,344-354].

---

### User Story 2 - Record what the tracker created (Priority: P1)

`specsync record <NNN> --platform P --from batch-output.json` merges a batch's records (key, ref, id, state, hash, url) into the feature's `tracker.md`.

**Independent Test**: `test_tracker_round_trip_and_actions`.

**Acceptance Scenarios**:

1. **Given** records, **When** recorded, **Then** `tracker.md` is rewritten with platform, last-sync date and a `| Key | Item | Id | State | Hash | URL |` table, feature first then stories by number [OBSERVED: specsync.py:212-227,377-394].

---

### User Story 3 - Every feature at a glance (Priority: P2)

`specsync features` lists every `specs/NNN-*` directory with its state, whether it's synced, and how many items are pending.

**Acceptance Scenarios**:

1. **Given** `specs/`, **When** `features` runs, **Then** one row per feature with state (or `retired`), synced/not synced and pending count [OBSERVED: specsync.py:397-410].

---

### User Story 4 - Retired features and orphans (Priority: P3)

A feature whose README statuses are all retired words (retired, superseded, removed, dropped, cancelled) plans no items; items in `tracker.md` that no longer match a story are reported as orphans, never deleted.

**Independent Test**: `test_retired_from_notes`, `test_orphans_reported`.

**Acceptance Scenarios**:

1. **Given** a retired feature, **When** planned, **Then** no items, and the output says so [OBSERVED: specsync.py:163,299-301,355-356,360-361].
2. **Given** an orphan, **When** planned, **Then** "orphan … report it; never delete the item" [OBSERVED: specsync.py:357,371-372].

### Edge Cases

- `specs/` is resolved from the current directory, so specsync must run at the repository root [OBSERVED: specsync.py:46-47].
- Story bodies over 6000 characters are cut at a line with `…` [OBSERVED: specsync.py:329].
- README rows match on the first cell's number with leading zeros stripped [OBSERVED: specsync.py:157].

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: specsync MUST never call the tracker; it only plans and records.
- **FR-002**: Item state MUST follow the repository (tasks, README), never the tracker.
- **FR-003**: Each item body MUST carry the marker `<!-- ai-gent:sync <path> <key> -->` [OBSERVED: specsync.py:35].

### Key Entities

See `data-model.md`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `test_specsync.py` passes [OBSERVED: `scripts/check.sh --quick` 2026-10-18, within "Ran 35 tests" ok].
- **SC-002**: `specsync features` parses the reconstructed `specs/` of this repository [OBSERVED: see `docs/product/introspec.md` § Runs].

## Assumptions

- [ASSUMPTION: `repo:sync` is the only caller.]
