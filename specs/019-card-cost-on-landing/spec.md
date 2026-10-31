# Feature Specification: Card cost on landing

**Feature Branch**: `docs/mod-specs-2` (specification only; built on its own branch)

**Created**: 2026-10-31

**Status**: Accepted

**Input**: When an `agents:orchestrate` card lands, a toast says what it cost against what was projected (`004-us1-core landed: used 391k vs ~420k projected (-7%)`). A large miss, either way, adds a pointer to what to fix so the next projection is better. The figures are the ledger's Tokens cell; the mod writes nothing, and the hint is documented, never applied.

**Depends on**: 014 (token status) — the ledger reader (`parseLedger`, `LedgerCard`, `inFlight` in `plugins/agents/hooks/token-model.ts`), the refresh in `tokens.ts` and its `$.state` atoms. 014's overrun toast (US3) fires while a card is *still running* and over its projection; this feature fires once, *at landing*, for every card, over or under.

## Context and ground rules

- The ledger's State column has the fixed words `queued`, `running`, `review`, `landed` (`plugins/agents/skills/orchestrate/SKILL.md`, "Ledger"); a card is done at `landed`. The Tokens column (`~projected / used`) is written only by `token_report.py tasks` (`write_ledger`, `ledger_cell`), which the coordinator runs after a landing (the skill's token step) — so the used figure usually appears *after* the state flips.
- The percentage is the ratio of the two figures the ledger shows; the mod computes no other token figure (014 FR-001). Its rounding is checked against the script's `pct_error` (`token_report.py:959`).
- `token_report.py tasks` already records projected and used per card in `docs/product/token-history.jsonl` and the history already feeds `estimate` (`estimate_by_tags`, `bias_factors`). Nothing new is recorded; this feature makes the miss visible at the moment it can be acted on.
- Owner: the `agents` plugin, the same module and `userConfig` as 014. Read-only; Claude Code-only.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Know what a card cost when it lands (Priority: P1)

When a card's state turns `landed` and its Tokens cell holds both figures, one toast: `004-us1-core landed: used 391k vs ~420k projected (-7%)`.

**Why this priority**: the coordinator reports the estimation error per card in the final handoff, but today nobody sees it while the next wave is still being planned.

**Independent Test**: a `claude plugin test` case with a fixture ledger read twice (a card `running`, then `landed` with `~420k / 391k`) and a mocked clock asserts one toast with that text.

**Acceptance Scenarios**:

1. **Given** the ledger had `004-us1-core` as `review`, **When** the next read finds it `landed` with `~420k / 391k`, **Then** one toast says `004-us1-core landed: used 391k vs ~420k projected (-7%)`.
2. **Given** a card lands with the Tokens cell `~420k / —` (`tasks` not run yet), **When** read, **Then** no toast; when a later read shows `~420k / 391k`, **Then** the toast fires then.
3. **Given** a card lands with no projection (`— / 391k`), **When** read, **Then** the toast reads `004-us1-core landed: used 391k (no projection)`.
4. **Given** the same card on the next reads, **When** nothing changed, **Then** no second toast.
5. **Given** `cardCostToast` is false, **When** a card lands, **Then** nothing is shown.

---

### User Story 2 - A big miss says what to fix (Priority: P2)

When the error is at least `cardCostHintPercent` (default 25%) either way, the toast adds a short pointer: `… (+31%) — wrong tags or effort? see token-economy` for an overrun, `… (-42%) — projection too high; budget shrank for nothing` for an underrun.

**Why this priority**: an unexplained number gets ignored; the cause is almost always the card's `Tags:` or effort, which `estimate` reads.

**Independent Test**: fixture cells at +10%, +31% and -42%; assert the hint on the last two only.

**Acceptance Scenarios**:

1. **Given** `~420k / 550k` (+31%), **When** it lands, **Then** the toast carries the overrun hint.
2. **Given** `~420k / 240k` (-43%), **When** it lands, **Then** the toast carries the underrun hint.
3. **Given** an error under the threshold, **When** it lands, **Then** no hint.
4. **Given** the hint, **When** the user follows `plugins/agents/skills/orchestrate/references/token-economy.md`, **Then** a new section ("Reading the landing toast") says what to check (the card's `Tags:` in `cards.md`, its effort and task count) and to re-run `token_report.py tasks` so the history learns; the mod changes none of these files.

---

### User Story 3 - See the wave's outcome in one line (Priority: P3)

When the last in-flight card lands, one toast counts how the wave went: `wave landed: 3 cards, 1 over, 2 under projection · /tokens for the budget`.

**Independent Test**: three fixture cards landing across reads; one wave toast after the last.

**Acceptance Scenarios**:

1. **Given** cards `queued`, `running` or `review` remain, **When** one lands, **Then** no wave toast.
2. **Given** the last in-flight card lands and at least one card landed during this session, **When** read, **Then** one wave toast counts the cards, those over and those under (the counts, never a token sum).
3. **Given** a session that starts with every card already `landed`, **When** read, **Then** no toast (nothing landed while it was watching).

### Edge Cases

- **First look** (session start, resume, hot reload): cards already `landed` are recorded as seen and never toasted; only a transition seen by the mod, or a `landed` card whose used figure appears later, toasts. A resumed run does not replay its landings.
- **A landing the mod never saw as in flight** (the coordinator went `queued` → `landed` between two reads): it still toasts once, by the rule that a `landed` card not yet in the toasted list and not in the first-look set is news.
- **Fix round after landing**: a card that goes back to `running` or `review` leaves the toasted list; its next landing toasts again.
- **Card removed from the ledger** before its used figure appears: no toast.
- **Several cards land in one read**: at most three toasts, then `+N more landed — /tokens`.
- **Latency**: the figure appears when `tasks` runs; the module reads the ledger at once after a Bash call whose command contains `token_report.py tasks` finishes (FR-004), instead of waiting for 014's throttled refresh.
- **Zero projection** (`~0k`): no percentage; read as no projection.
- **Malformed or missing ledger**: nothing happens (014's reader skips it).
- **Headless**: nothing.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Every figure shown MUST be a cell of the ledger's Tokens column as written (`projectedText`, `usedText`); the only derived value is the percentage, `(used / projected − 1)` rounded to a whole percent with a sign. It is computed from the cells' parsed (already rounded, `391k`) values, rounded half to even like Python's `{:+.0f}`, and equals `pct_error` fed the same parsed values on a shared fixture table that includes exact `.5` cases. It can differ by a point from `tracker.md`'s Error column, which `pct_error` computes from exact figures; the toast states the figures it was computed from.
- **FR-002**: A card is *landed news* when its state is `landed`, its used figure is present, and it is not in `$.state`'s `landingToasted` list nor in `landingFirstLook` (both atoms, `data-model.md`, so a hot reload keeps them); its state leaving `landed` MUST clear it from the toasted list.
- **FR-003**: The mod MUST read the ledger through 014's `parseLedger` and `readCards` (one reader), and MUST NOT write the ledger, `cards.md`, `tasks.md` or the history; the hint MUST NOT be applied by the mod.
- **FR-004**: After a Bash tool call whose command contains `token_report.py tasks` completes, the module MUST read the ledger immediately, bypassing 014's refresh throttle (no spawn of the script: it only reads the file the script wrote).
- **FR-005**: `plugins/agents/skills/orchestrate/references/token-economy.md` MUST gain a section on reading the landing toast and acting on a miss (tags, effort, task count, re-run `tasks`); `SKILL.md` links it in one sentence. This is the "feed back" half: documented, manual.
- **FR-006**: `userConfig`: `cardCostToast` (boolean, default true) and `cardCostHintPercent` (number, default 25; 0 turns the hint off). `cardCostToast` is checked where the module composes the landing code in `register.tsx`; `tokenStatus` false (014's switch) returns first and so also turns this feature off, because the landing code rides 014's ledger reader.
- **FR-007**: At most three card toasts per read; the rest collapse into one `+N more` line.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/agents` passes stories 1–3 on `terminal` and `desktop`, including the first-look, delayed-figure and fix-round cases.
- **SC-002**: The percentage parity test passes: for a table of projected/used pairs (rounded cell values, including `.5` boundaries) the TypeScript result equals `pct_error`'s output for the same values (run from `test_token_report.py` against a fixture table in `plugins/agents/hooks/`); a second table row set pins that a ledger-cell pair and its exact-figure pair may differ by one point, so the difference is documented, not discovered.
- **SC-003**: The module spawns nothing for this feature (it reads the ledger file), and a session with no `landed` change reads no more than 014 already does.

## Out of scope

- Writing the percentage, the hint or any figure back to the ledger, `cards.md`, `tasks.md` or the history (`tasks` keeps writing them).
- Applying the hint: no automatic edit of `Tags:`, effort or the projection.
- Summing token figures across the wave (a sum is `token_report.py`'s job; see Decisions).
- Live usage of a running card (014's out of scope, and 014 US5).
- The coordinator's share and the run's total error (the final handoff's job, `SKILL.md` "Land").

## Assumptions

- [ASSUMPTION: The coordinator runs `token_report.py tasks` after each landing, as `SKILL.md` says, so the used figure follows the state change within the same stretch of work.]
- [ASSUMPTION: The ledger keeps `landed` rows for the rest of the run (the skill defines `landed` as a state, not a reason to delete the row).]

## Decisions (2026-10-31, with the maintainer)

- The wave toast counts cards over and under projection and shows no token total (a total would need a script summary; not built). Toast only, no native notification.
- Hint threshold 25% (`cardCostHintPercent`). decided 2026-10-31 (the maintainer accepted the default): `token-economy.md` states no tolerance figure (checked), so there was nothing to align with; the new "Reading the landing toast" section records 25% as the default and says to tune it.
- The percentage comes from the ledger's rounded cells, half to even (FR-001); the one-point difference from `tracker.md` is accepted and documented.
- The landing toast and 014's overrun toast stay separate (different moment, different text); they are not merged.
- `tokenStatus` off turns this feature off too (FR-006).
