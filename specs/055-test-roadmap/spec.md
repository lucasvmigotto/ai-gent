# Feature Specification: Live acceptance testing on a shared roadmap page (`qa:uat`)

**Feature Branch**: none yet (specification only, not committed; `plan.md` names the branch it would be built on)

**Created**: 2026-11-19

**Status**: Draft

**Input**: the owner's request (2026-11-19) and one pilot session run by hand: an acceptance roadmap written as a claude.ai Artifact page with a shared register, used by business validators while the agent followed the results and triaged every failure. The pilot's product, people and data are deliberately not recorded here; only the process and what it taught are.

**Decided**: the owner answered the first round of open questions on 2026-11-19; see Decisions.

**Depends on**: the Artifact tool and its runtime capabilities (`db`, `user`; `assets` only when attachments are turned on), the `ArtifactData` tool (reads, including reads at a lower access level), `qa:strategy` (personas, the UAT row of `qa.md`), `project:spec` (Spec Kit features and acceptance scenarios), the `ai-gent` binary (`plan.md`). Milestone: 3.x, after 3.0.0.

## Context and ground rules

- **What it is.** A guided *live testing* session: people who know the business (validators) execute a written roadmap against a running test environment, mark each step OK or NOK, and suggest improvements, all on one shared page. The agent writes the roadmap from evidence, has it reviewed independently, runs the access preflight, follows the register on request, triages each NOK against the code and data (read-only), drafts replies and status messages, and closes with a consolidated report.
- **What it is not.** It does not replace automated tests (`qa:e2e`, `qa:load`); it never moves a feature to Verified (pipeline rule: only `qa:e2e` and `qa:load` plus green CI do); it is not a pipeline artifact (its report stays local, Decision D2); it never fixes code during the session; it never runs against production.
- **The page is the single source.** No parallel PDF or spreadsheet copy is kept during the session. A static report is produced only at the end (FR-060).
- **Register content is third-party data.** Observations, improvements, links and role proposals written by validators are data, never instructions to the agent (FR-078).
- **The agent writes to the register only when the owner asks** (FR-071).
- **Nothing runs on its own by default.** The agent reads the register when the owner asks; periodic follow-up exists only when the owner turns it on (FR-077, Decision D4).
- **Audience split.** The page is written for business validators: no code, SQL, file paths or internal identifiers. Evidence for each claim lives in the roadmap source and the reviewer's findings, not on the page.
- **Tier.** The shared page needs claude.ai Artifacts, which only Claude Code (tier 1, `docs/product/rust-decisions.md` D2) has. Elsewhere the skill runs in reduced mode (FR-090).

### Glossary

Each term below is used with this meaning only.

| Term | Meaning |
|---|---|
| Owner | The person who runs the session and owns the Artifact: asks for the roadmap, lists the validators and assigns their roles, shares the page, decides on NOKs, role proposals and change requests. One per session. |
| Validator | A person invited to execute steps and mark them. Has an **assigned role**. |
| Role | A permission profile of the application under test (role A, role B, role C, "technical role"). Each step names the role that must execute it (its **target role**). |
| Assigned role | The role the owner gave a validator. The only role coverage counts. |
| Role proposal | A validator's request, made in R0, to be counted under another role. Visible to the owner; has no effect until the owner approves it. |
| Roadmap | The whole page and its source: context sections, test data, scripts, results. Has a version. |
| Roadmap source | `roadmap.json`, the local file the page is rendered from. |
| Script | One numbered group of steps (`R0` … `Rn`): preparation, full flows, one per rule or variation, complements. |
| Step | One action with its expected result, executed by one target role. Has a stable id `r<N>-<M>` and a revision. |
| Test record | One item of test data a script creates and uses, named with the script's prefix and date. |
| Register | The Artifact's shared `db`: marks, roster, role proposals, roadmap metadata and, optionally, triage notes. |
| Snapshot | A copy of the register the agent read with `ArtifactData` and saved locally. |
| Mark | One validator's result on one step: OK or NOK, with an observation (required on NOK) and optional links. |
| Stale mark | A mark made on an earlier revision of its step. |
| Improvement | A suggestion on a step, independent of the mark (valid with OK). |
| Evidence | What backs a NOK: structured text (required), links, and attachments only when turned on. |
| Preflight | The access check: a guided checklist the owner confirms before inviting, plus a test read of the register after publishing. |
| Follow-up | The agent reading a snapshot, reporting what changed and triaging new NOKs. On request by default; periodic only when turned on. |
| Last real check | The time of the latest snapshot the agent actually read. |
| Triage | The agent's classification of a NOK, its probable cause and a suggested reply. |
| Decision list | Change requests and open questions only the owner or the business can settle. |
| Reduced mode | The skill without the Artifact tool: local page, per-validator checklist, same triage and reports (FR-090). |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Describe the scope and the people (Priority: P1)

The owner says what to test and who will test it. The scope is the whole application (mode A) or one or more features (mode B, by Spec Kit feature number). The owner lists the validators: a label to show on the page, the **assigned role** of each, the account or e-mail to share the page with (kept local), and the scripts each is expected to execute. The owner names the test environment (never production).

**Independent Test**: with a fixture project that has two Spec Kit features, mode B on one feature yields a roadmap whose steps trace only to that feature's acceptance scenarios; a scope naming a production environment is refused with the reason; a validator listed without a role is asked about before any step is written for them.

**Acceptance Scenarios**:

1. **Given** no scope, **When** the skill starts, **Then** it asks for scope, validators with their assigned roles, and environment in one round (`shared/asking.md`), offering mode A or B (FR-070).
2. **Given** mode B and `specs/007-feature-y/spec.md`, **When** inputs are collected, **Then** each story and acceptance scenario of that feature is listed as a candidate for at least one step, read through the existing Spec Kit reader (FR-008).
3. **Given** an environment the project classifies as production, or one the owner cannot confirm is not, **When** inputs are collected, **Then** the skill stops and asks for a test environment (FR-080).
4. **Given** a validator without an assigned role, **When** inputs are collected, **Then** the skill asks the owner for it; it never infers a role from a name or an e-mail (FR-091).
5. **Given** a role in the list that no step targets, **When** the roadmap is drafted, **Then** the skill says so and asks whether that validator is needed (FR-020).

### User Story 2 - Get a roadmap that is true and executable (Priority: P1)

The agent derives scripts and steps from evidence (spec, contracts, authorization matrix, code, tests, data), labels each claim, writes each expected result so the target role can observe it on screen, names the target role and preconditions, and adds R0 (access and write check, role confirmation, test data, configuration to register). A separate agent then reviews every claim read-only before anything is published.

**Independent Test**: a fixture roadmap with a false claim, an unobservable expected result (an internal state the screen does not show), a result only another role can see, and a missing precondition is rejected by the reviewer and by `ai-gent uat lint` with one finding per defect.

**Acceptance Scenarios**:

1. **Given** a draft roadmap, **When** it is written, **Then** every claim in the source has an evidence label (`[OBSERVED: …]`, `[INFERRED: … → confirm with …]`, `[ASSUMPTION: …]`) and every step has `who`, `action`, `expected`, and `pre` when one applies (FR-005, FR-007).
2. **Given** a draft, **When** the independent review runs, **Then** it is done by a different agent with read-only tools, it reads the code and data, not the author's reasoning, and it returns findings that the author fixes before the owner sees the page (FR-009).
3. **Given** a step whose expected result is visible only to another role (a notice another role receives), **When** linted, **Then** it is split into two steps with two target roles, or rewritten so the target role can observe it (FR-006).
4. **Given** the page text, **When** rendered, **Then** it contains no file path, SQL, class name, code identifier, e-mail or account (FR-010).
5. **Given** an authorization rule in scope, **When** the roadmap is linted, **Then** at least one step has a target role that must be refused (FR-003, FR-004).
6. **Given** a step that depends on shared state or on a delay, **When** linted, **Then** it declares exclusivity or a wait (FR-015, FR-016).
7. **Given** a draft, **When** linted, **Then** the page has every section of FR-001 in order and R0 has the write-check and role-confirmation steps (FR-001, FR-002).

### User Story 3 - Check access before and after publishing (Priority: P1)

Before the invitation goes out, the agent walks the owner through a checklist: for each validator, the sharing level that lets them write marks, the caveats of the sharing mode (outside-organization people, link sharing) and exactly what to set in the sharing menu; the owner confirms each line. After publishing, the agent reads the register at lower access levels (`view`, `interact`, `admin`) to prove that the **register's rules** give each level what they should and no more. Neither step can see the sharing level each person actually has: that is proven only when each validator's own write in R0 shows up in the register.

**Independent Test**: a fixture validator list with one person to be shared as Viewer, one outside the owner's organization and one at the right level yields a checklist with two named fixes and one pass; after a publish, `ArtifactData` reads at `view`, `interact` and `admin` return the collections the rules allow at each level and nothing the rules hide; the preflight record states what was and was not proven.

**Acceptance Scenarios**:

1. **Given** a validator to be shared as Viewer or Commenter, **When** the checklist runs, **Then** it names the person and the level that allows writing, and waits for the owner to confirm (FR-092).
2. **Given** an outside-organization validator, **When** the checklist runs, **Then** it warns that switching the page to link sharing can remove their write access and that they may not be able to upload attachments (FR-092, FR-094).
3. **Given** a published page, **When** the test read runs, **Then** the agent reads each collection at `view`, `interact` and `admin`, compares what each level sees with `data-model.md`, and reports a mismatch as a blocker (FR-093).
4. **Given** the test read passed, **When** the preflight record is written, **Then** it says that the register's rules are proven for reading, that the write side and each person's sharing level are not, and it lists the validators whose R0 write check has not yet appeared (FR-093, `contracts/reports.md`).
5. **Given** a write refused at runtime, **When** a validator marks a step, **Then** the page reverts the mark and shows a fixed, visible notice saying what access to ask the owner for (FR-030).
6. **Given** the owner changes sharing later, **When** the next report is written, **Then** it reminds the owner to re-check sharing for validators who stopped marking; the agent cannot see refused writes, so this is a prompt, never a claim (FR-075).
7. **Given** validators had to be shared as Editors, **When** the checklist runs, **Then** it warns that Editors can republish the page and change its rules (FR-097).

### User Story 4 - Mark, observe and suggest on one shared page (Priority: P1)

Each validator opens the page, sees the context, the scripts and the steps, marks OK or NOK per step, writes what happened on NOK, optionally adds links, and suggests improvements on any step. Everyone sees everyone's marks live, with names and photos resolved at display time.

**Independent Test**: two signed-in test accounts marking the same step at the same time both keep their marks; a Viewer account sees all marks and no controls; a NOK without its observation text shows as "observation missing".

**Acceptance Scenarios**:

1. **Given** a validator with write access, **When** they press NOK, **Then** an observation field opens with prompts for what happened, when, the message and the screen, and the NOK shows as "observation missing" until it is filled (FR-014, FR-050).
2. **Given** a NOK, **When** the validator adds a link, **Then** it is saved with the mark and shown as a link that opens in a new tab (FR-050).
3. **Given** attachments are off (the default), **When** the page renders, **Then** no upload control appears and the observation label tells the validator to describe the screen instead (FR-050, FR-094).
4. **Given** attachments were turned on for the session, **When** a validator attaches a file, **Then** the page shows the limits and the remaining budget, stores only the asset id, and refuses a file over the budget (FR-051, FR-094).
5. **Given** a step marked OK, **When** the validator writes an improvement, **Then** it is saved and shown beside the step and in the improvements list, and the mark stays OK (FR-017).
6. **Given** two validators marking at once, **When** both save, **Then** neither loses data (one document per person, FR-031, FR-041).
7. **Given** a step whose text was rewritten after a validator marked it, **When** the page renders, **Then** that mark shows a stale badge and is not counted (FR-022).
8. **Given** a phone-width screen or dark theme, **When** the page renders, **Then** the tables scroll in their own container, every control works by keyboard and contrast holds (FR-012, FR-023, FR-024).

### User Story 5 - See progress, gaps and coverage by role (Priority: P1)

The page shows a panel per script (steps, with OK, with NOK, unmarked, stale, who marked), the list of NOKs (script, step, who, observation, time), the list of improvements, and coverage by role: for each step, its target role against the assigned roles of the people who marked it, flagging steps marked only by other roles. The owner also sees the roster panel.

**Independent Test**: a fixture register where a high-privilege role marked every step yields a panel that flags every step whose target role is a lower-privilege role as "not covered by its role"; a pending role proposal does not change any count.

**Acceptance Scenarios**:

1. **Given** marks only from role A on a step whose target role is C, **When** the panel renders, **Then** the step is flagged and counted as not covered (FR-020).
2. **Given** a NOK and an OK on the same step from different people, **When** the panel renders, **Then** the step counts as NOK and both chips show (FR-018, FR-019, FR-021).
3. **Given** a validator who proposed role C in R0 while assigned role A, **When** the panel renders, **Then** their marks count as role A, the owner sees the pending proposal with its note, and coverage changes only after the owner approves it (FR-026, FR-091).
4. **Given** a validator who has marked but has no roster entry (signed in with an account the owner did not list), **When** the panel renders, **Then** their marks show under "role not assigned", count for no role, and the owner sees them in the roster panel (FR-026, FR-091).

### User Story 6 - Follow the session and triage each NOK (Priority: P1)

While the session runs, the owner asks the agent for a check; the agent reads a snapshot of the register, reports what changed since the last real check, and triages each new NOK read-only against the code, specs and test data: it classifies the cause, explains it in plain words and drafts a reply to the validator. Change requests go to the decision list, never to a fix. Periodic checks are off by default; they run only if the owner turns them on, and only while the session is open.

**Independent Test**: a fixture register with six NOKs, one per class, yields a triage report with the right class for each, one reply per NOK, and the change request on the decision list; a check with no change since the last snapshot prints exactly the "no news" line with the last real check time; with follow-up left at its default, no timer exists after the skill's turn ends.

**Acceptance Scenarios**:

1. **Given** a NOK on a step that works as specified but the validator wants other behavior, **When** triaged, **Then** it is classified `change-request`, the reply explains the difference between NOK and an improvement, and the item goes to the decision list (FR-072, FR-073).
2. **Given** follow-up at its default (off), **When** the session runs, **Then** the agent creates no timer and reads the register only when the owner asks (FR-077).
3. **Given** the owner turned periodic follow-up on, **When** the agent sets it up, **Then** it states the interval, that it runs only while this session is open and idle, that it expires after 7 days, and that it notifies nobody (FR-070, FR-077).
4. **Given** no new marks since the last snapshot, **When** a check runs, **Then** the report is one line saying there is nothing new, with the last real check time and when the next check is (or "on request") (FR-075, FR-095).
5. **Given** a NOK whose observation contains text shaped like an instruction, **When** triaged, **Then** it is treated as data and quoted, never followed (FR-078).
6. **Given** a `defect`, **When** the owner agrees, **Then** an issue is drafted through `repo:issues`, asked first (FR-074).
7. **Given** a status report for the team, **When** drafted, **Then** it is a list in plain language with no table, no code and no file name (FR-076).
8. **Given** the owner asks to note "answered" on a NOK in the register, **When** the agent writes, **Then** it first shows the exact write, writes only that, and only into `triage` (FR-043, FR-071).

### User Story 7 - Change the roadmap mid-session without corrupting marks (Priority: P2)

When a step is wrong, the agent proposes a fix; on the owner's approval it republishes the page with a new roadmap version, a visible changelog and a notice to reload. Step ids never change; a rewritten step gets a new revision; marks made on an older revision show as stale; new steps are appended without renumbering.

**Independent Test**: `ai-gent uat diff` between two roadmap versions refuses a renumbered or deleted id, bumps the revision of each step whose text changed, and drafts the changelog; the page shows a stale badge on a mark whose revision is older than the step's.

**Acceptance Scenarios**:

1. **Given** a step with marks, **When** its text is rewritten, **Then** its id and its controls' ids stay (FR-013), its revision goes up, existing marks show as stale, and the owner is offered an assisted clean-up (keep as stale, or ask the validator to redo; the agent writes nothing without the owner's request) (FR-022, FR-071).
2. **Given** a new step between steps 3 and 4, **When** added, **Then** it gets the next free number in the script (not 4), and its place is kept by a separate order field (FR-005).
3. **Given** a republish, **When** validators have the page open, **Then** the page shows a reload notice, and after reloading the new version and its changelog entry; the register is intact (FR-022, FR-044).
4. **Given** a step that is no longer valid, **When** removed, **Then** it is kept as retired, hidden from the scripts, and its marks stay in the register and the report (FR-005).
5. **Given** the published page differs from the last page the agent rendered (someone else republished it), **When** the agent is about to republish, **Then** it stops and shows the difference to the owner (FR-097).
6. **Given** a rewritten step whose claims changed, **When** republishing, **Then** the independent review runs for the changed steps first (FR-009).

### User Story 8 - Close the session (Priority: P2)

At the end, the agent writes the consolidated report at `.ai-gent/uat/<slug>/report.md`: coverage per script and per role, NOKs by class with their status, improvements, the decision list, known limitations, the preflight record, and the session's timeline. It drafts the closing message for the team.

**Independent Test**: a fixture register and roadmap yield a consolidated Markdown report with every NOK classified and every step accounted for (marked, unmarked, stale, retired, not covered by its role, only marked by someone who left).

**Acceptance Scenarios**:

1. **Given** unclassified NOKs or pending role proposals, **When** closing, **Then** the skill lists them and asks before closing anyway (FR-070).
2. **Given** closing, **When** the report is written, **Then** it is written only under `.ai-gent/uat/<slug>/`, never into `specs/`, `docs/` or git (FR-079).
3. **Given** periodic follow-up is on, **When** the session closes, **Then** the timer is removed and the closing message says so (FR-077).
4. **Given** the owner wants a file to send, **When** closing, **Then** the report is that file; the page itself is never printed (FR-060).

### Edge Cases

- **Empty register** (new session, or a viewer signed out): the page renders all steps with no marks and a status line, never an error; `digest` reports zero progress (FR-033).
- **Outside claude.ai** (no `window.claude`): the page renders read-only with a line saying where to open it (FR-033).
- **Validator without an assigned role**: at input time the skill asks (US1 AS4); at runtime, an unlisted account's marks show under "role not assigned" (US5 AS4).
- **Validator leaves mid-session** (access removed or left the organization): their marks stay; chips show a neutral label when the profile cannot be resolved; the report lists steps whose only current marks came from people who left, so the owner may reassign them (FR-099).
- **Roadmap republished with a rewritten step**: stale marks, never deleted (US7 AS1).
- **Step depends on shared state** another validator can change: exclusivity note and a test record per script (FR-015).
- **Behavior with a delay** (polling, a scheduled job): the step declares the wait and what to look at, so a too-early look is not a NOK (FR-016).
- **Two scripts use the same test record**: refused by lint (FR-010).
- **Large register** (many validators, long observations): text fields are capped, `lint` warns when steps times caps could exceed the per-document budget, and the agent reads snapshots page by page (FR-098).
- **Register quota reached**: the page shows the quota notice; the next report tells the owner (FR-030).
- **Two owners**: a session has one owner, the Artifact's owner. A second person who needs to decide is told that the register's moderation rules name only the owner, so their decisions go through the owner. If validators had to be shared as Editors, the checklist warns that Editors can republish the page and change its rules (FR-097).
- **Validator marks before R0** (no write check yet): allowed; the panel shows R0 incomplete for that person.
- **Reduced mode** (no Artifact tool): no shared page; local HTML and per-validator checklists; triage and reports from the collected files; preflight is skipped and the report says so (FR-090).
- **Same person, two accounts**: two roster entries; the owner assigns one and leaves the other unassigned.
- **Session left open for days with periodic follow-up on**: the timer expires after 7 days; the next report gives the last real check and says the timer has expired (FR-095).

## Requirements *(mandatory)*

### Roadmap content

- **FR-001**: The roadmap MUST have, in order: how to use it (OK versus NOK versus improvement, what to write on NOK, exclusivity and waits), who is involved (roles, what each role may do, which validator holds which assigned role), the rules being tested in plain words, the test data, the scripts, the results, known limitations, and the changelog.
- **FR-002**: R0 MUST be a preparation script: each validator confirms access and their assigned role (and may propose another, FR-091), marks a write-check step to prove they can write (FR-093), and the owner (or a named role) creates the test data and any configuration the scripts need.
- **FR-003**: Scripts MUST be grouped as preparation, full flows (end to end), one per rule or variation (including the negative: a role that must be refused), and complements.
- **FR-004**: Every authorization rule in scope MUST have at least one step whose target role must be refused, mirroring the QA quality floor (`plugins/qa/references/qa-quality.md`).
- **FR-005**: Every step MUST carry: id, order, revision, script, `who` (target role, optionally a named validator), action, expected result, and when they apply: precondition, exclusivity, wait, expected evidence, traces (Spec Kit story or scenario). New steps take the next free number; ids are never renumbered or reused; removed steps are retired, not deleted.
- **FR-006**: The expected result MUST be observable on screen by the target role. A result only another role can see becomes a separate step for that role.
- **FR-007**: Every claim in the roadmap source MUST carry an evidence label; `[INFERRED]` names how to confirm, `[ASSUMPTION]` is listed for the owner before publishing. Labels never appear on the page.
- **FR-008**: In mode B, every acceptance scenario of the chosen features MUST map to at least one step or be listed as not testable by hand with the reason; each step lists the scenarios it traces to. Features are read with the existing Spec Kit reader.
- **FR-009**: Before the first publish, an independent agent (a different subagent, read-only tools) MUST check every claim against code and data and return findings; the author fixes them and records the review's date and result in the roadmap source. Republishes that change claims get the same review for the changed steps.
- **FR-010**: Page text MUST be free of code, SQL, file paths, internal ids, e-mails, accounts and personal data. Validators appear by the label the owner chose. Test data is synthetic, named with a prefix per script and date, and no test record is shared by two scripts.

### Page structure and behavior

- **FR-011**: A header with title, roadmap version, environment name and a status line (loading, saved, read-only, error); an index of scripts at the top linking to each.
- **FR-012**: Each script is a table with columns `#`, Who, Action and expected result, Result. Tables scroll inside their own container on narrow screens; the page itself never scrolls horizontally.
- **FR-013**: Each step row carries `data-step="r<N>-<M>"`, `data-rev="<n>"` and `data-role="<role key>"`; every control has a stable id derived from the step id.
- **FR-014**: OK and NOK are toggle buttons with `aria-pressed`; pressing the active one clears the mark. NOK opens an observation field (prompts for what, when, message, screen; a length cap) and shows "observation missing" while it is empty.
- **FR-015**: A step with exclusivity shows a note ("nobody else touches this item until you have checked") and the script's test record name.
- **FR-016**: A step with a wait shows how long and what to look at.
- **FR-017**: Every step has a "Suggest improvement" control, independent of the mark; improvements show beside the step with author and time.
- **FR-018**: Each step shows who marked what as chips (photo, name, result color; observation, links and time as the accessible description). Names and photos are resolved at render from stored ids, never stored.
- **FR-019**: A results panel per script: steps, with OK, with NOK, unmarked, stale, who marked. A step with any current NOK counts as NOK.
- **FR-020**: A coverage-by-role view: per step, target role and the assigned roles of those who marked; steps marked only by other roles are flagged; a count per role of steps covered. Only assigned roles count (FR-091).
- **FR-021**: A NOK list (script, step, who, observation, time, newest first, linked to the step) and an improvements list in the same shape.
- **FR-022**: Roadmap version and changelog shown near the header; a stale badge on marks whose revision is older than the step's; a reload notice when the register's `meta` shows a newer roadmap version than the page's.
- **FR-023**: Light and dark themes through color tokens (`prefers-color-scheme` and an explicit `data-theme`), mobile layout at phone width with a 16 px gutter, reduced motion respected.
- **FR-024**: Accessibility: WCAG 2.2 AA contrast in both themes, keyboard operation of every control, visible focus, labels on every field, status and error lines in live regions (`role="status"`, `role="alert"`).
- **FR-025**: The page MUST NOT store names, e-mails, photos, tokens, cookies or any secret; it stores ids, marks, observations, links, improvements, role proposals, attachment ids (when on) and timestamps only. The observation, improvement and link fields remind people not to paste credentials or personal data.
- **FR-026**: The owner sees a roster panel: each account that opened or marked, its assigned role, any pending role proposal with the validator's note, and controls to assign a role or approve or reject a proposal. Other viewers see assigned roles and that a proposal is pending, not its note.

### Writes and errors

- **FR-030**: A refused write (`invalid_argument` or `not_granted` on the register) MUST revert the local state to the last saved one, hide the controls, and show a fixed notice at the bottom of the screen saying the access is view-only and what to ask the owner for. Quota and other errors have their own messages.
- **FR-031**: Writes are queued: one write in flight per document, coalescing changes made meanwhile, only on change, never on load.
- **FR-032**: The page subscribes to each register collection once, with a 10 s loading timer cleared on the first server snapshot or error; it never reloads itself.
- **FR-033**: The page renders fully with no register (signed out, outside claude.ai, capability refused) and says why it is read-only.

### Register data model (detail in `data-model.md`)

- **FR-040**: The register is the Artifact's `db`. Collection `marks`: one document per validator id, holding a `steps` map (`{r, obs, at, rev, links[], ev[]}` per step id) and an `improvements` map (`{txt, at, rev}` per step id).
- **FR-041**: One document per person, not one per step or a shared document: rules can then say "each writes only their own", and two people saving at once cannot overwrite each other (the register is last-writer-wins per document).
- **FR-042**: Rules: `marks` read `view`, write `owner` (the owner can moderate and clean); `marks/{self}` write `interact`. `roster` (assigned roles) read `view`, write `owner`, with no self rule. `proposals` (role proposals) read `view`, write `owner`; `proposals/{self}` write `interact`. `meta` (roadmap version) read `view`, write `owner`.
- **FR-043**: Optional collection `triage` (one document per step and validator) written only by the agent on the owner's request: class, short reply, status. Read `view`, write `owner`.
- **FR-044**: Capabilities declared on first publish: `db` with the rules above and `user` with scope `profile`. Republishes omit `capabilities` so the declaration is kept; turning attachments on restates the whole set with `assets` added.

### Evidence

- **FR-050**: Evidence on a NOK is structured text (required: what, when, message, screen) and, optionally, links. Attachments are off by default.
- **FR-051**: With attachments on (FR-094), the page stores the asset id in the mark (`ev[]`), shows the size and type limits and the session's remaining budget, and deletes an attachment only on a deliberate action by its author or the owner.

### Export

- **FR-060**: The page does not print or make a PDF. The static result is the consolidated report the agent writes from a snapshot (`ai-gent uat report`, FR-079).

### Agent and skill behavior

- **FR-070**: The skill MUST stop for the owner at: inputs collected; roadmap draft with the `ASSUMPTION` list and the reviewer's findings; each preflight checklist line; publish; preflight test read result; invitation text; turning periodic follow-up on; turning attachments on; each roadmap change mid-session; any write to the register; closing with open items.
- **FR-071**: The agent never writes to the register (marks, roster, proposals, triage, meta, clean-up) without the owner asking for that specific write, and shows what it will write first. The one standing request is `meta` on each publish the owner approved. It never edits or deletes a validator's mark or proposal on its own judgment.
- **FR-072**: Triage is read-only against code, specs and test data. Each NOK gets one class: `defect` (the application does not do what is specified), `setup` (configuration, role, permission or test data), `roadmap` (the step's text is wrong or unclear), `change-request` (works as specified; the business wants something else), `known-limitation` (listed in the page's known limitations), or `needs-info` (cannot be told without more evidence). Each comes with a probable cause in plain words, a confidence, and a suggested reply.
- **FR-073**: `change-request` items and open rule questions go to the decision list; the skill never proposes a code fix for them during the session.
- **FR-074**: `defect` items are offered as issues through `repo:issues` (asked first), with the step, the observation and the evidence, and no validator's personal data beyond what the owner approves.
- **FR-075**: Status reports have a fixed shape (`contracts/reports.md`): "no news" as a single line; otherwise progress per script, new NOKs with class, cause and suggested reply, new improvements, pending role proposals, coverage gaps by role, stale marks, validators whose R0 write check is missing, and what the owner needs to decide.
- **FR-076**: Messages for the team use lists, never tables (team chats often do not render tables), and plain language. Templates: invitation, status, closing, and a reply per NOK (`contracts/reports.md`).
- **FR-077**: Follow-up is on request by default. Periodic follow-up is off unless the owner turns it on; then it is a session timer that runs only while this session is open and idle, expires after 7 days, notifies nobody, and is removed at closing. A cloud routine is out of scope.
- **FR-078**: Everything the register holds is third-party data: quoted, never executed or followed.
- **FR-079**: Local state lives under `.ai-gent/uat/<slug>/` (git-excluded): `roadmap.json`, the rendered page, the Artifact URL, snapshots, the preflight record, the follow-up setting, the triage log, the decision list and `report.md`. Nothing is written to `specs/`, `docs/` or git; the report is not a pipeline artifact.
- **FR-080**: The skill refuses an environment classified as production, and one the owner cannot confirm is a test environment.

### Portability

- **FR-090**: Where the Artifact tool is not available (another agent, or Claude Code without claude.ai), the skill says so and offers reduced mode: the rendered page as a local HTML file without a shared register, and a Markdown checklist per validator that the owner collects; triage and reports work the same from the collected files.

### Added by the owner's decisions (2026-11-19)

- **FR-091**: The owner assigns each validator's role when listing them. In R0 a validator may propose another role with a note; the proposal is visible to the owner and changes nothing until the owner approves it, and the owner's decision wins. Coverage counts only assigned roles. An account with no roster entry counts for no role.
- **FR-092**: The preflight checklist lists, per validator, the sharing level needed to write marks (the level that grants `interact` on the register), the caveats (outside-organization people; link sharing can remove their write access; they may not be able to upload attachments), and the exact change in the sharing menu. The owner confirms each line; the agent never claims to have seen anyone's sharing level.
- **FR-093**: After the first publish (and after any republish that restates capabilities), the agent reads every register collection with `ArtifactData` at `view`, `interact` and `admin` and compares what each level sees with `data-model.md`; a mismatch blocks the invitation. The preflight record states what this proves (the register's rules, as enforced for reading) and what it does not (the write side of the rules, which reads cannot exercise, and each person's sharing level, which the agent cannot read); per-person write access is proven only when that validator's R0 write-check mark appears in a snapshot.
- **FR-094**: Attachments are turned on per session by the owner only. Turning them on sets a size budget for the session, adds the warning not to upload cookies, tokens, passwords or personal data, and says that outside-organization validators may not be able to upload; validators keep text and links as the fallback.
- **FR-095**: Every report states the last real check (the time of the snapshot it was built from) and, when periodic follow-up is on, the next scheduled one or that the timer expired; the agent never reports a check it did not run.
- **FR-096**: Every requirement is traced to at least one acceptance scenario, success criterion or task (Traceability, below), and `tasks.md` names the requirements each task covers.
- **FR-097**: Before each republish the agent reads the published page and compares it with the last page it rendered; a difference (someone else republished) stops the republish and is shown to the owner. When validators had to be shared as Editors, the preflight warns that Editors can republish the page and change its rules.
- **FR-098**: Text fields have caps (observation, improvement, note, link count); `ai-gent uat lint` warns when the number of steps times the caps could exceed a per-document budget; the agent reads snapshots page by page and `digest` accepts several snapshot files.
- **FR-099**: Marks of a validator who left stay in the register and the report; the report lists steps whose only current marks come from people who left, so the owner can reassign them.

## Success Criteria *(mandatory)*

- **SC-001**: In a pilot, zero roadmap claims found false after publishing (the independent review catches them before).
- **SC-002**: Every NOK raised in a pilot is classified within one follow-up, and every `change-request` is on the decision list, not in a fix.
- **SC-003**: No validator loses a mark to a concurrent save, and a validator without write access sees the refusal notice within one action.
- **SC-004**: Coverage by role shows, at closing, every step either covered by its target role or explicitly listed as not covered.
- **SC-005**: A republish mid-session leaves every existing mark readable and flags stale ones; no step id changes.
- **SC-006**: The page passes axe with no serious or critical violations in both themes at 375 px and 1280 px.
- **SC-007**: In a pilot, the preflight test read matches `data-model.md` at all three levels, and every validator's R0 write check appears before their first script step.
- **SC-008**: With default settings no timer exists, no attachment control appears, and every report carries its last real check time.

## Traceability

| Requirements | Covered by |
|---|---|
| FR-001 to FR-010 | US1, US2; SC-001; T010, T030, T051 |
| FR-011 to FR-025 | US4, US5, US7; SC-003, SC-004, SC-006; T012, T020, T021 |
| FR-026, FR-091 | US1 AS4, US5 AS3 and AS4; T016 |
| FR-030 to FR-033 | US3 AS5, US4, edge cases; SC-003; T020 |
| FR-040 to FR-044 | US4 AS6, US6 AS8, US7 AS3; SC-007; T010, T020 |
| FR-050, FR-051, FR-094 | US3 AS2, US4 AS1 to AS4; SC-008; T017 |
| FR-060, FR-079 | US8; T013 |
| FR-070 to FR-078, FR-095 | US6, US8; SC-002, SC-008; T013, T018, T030 |
| FR-080 | US1 AS3; T030 |
| FR-090 | edge cases; T019 |
| FR-092, FR-093, FR-097 | US3, US7 AS5; SC-007; T014 |
| FR-098, FR-099 | edge cases; T013 |
| FR-096 | this table; T052 |

## Decisions (owner, 2026-11-19)

| # | Decision | Why |
|---|---|---|
| D1 | The skill is `qa:uat`; the spec directory stays `test-roadmap`. (`qa:roadmap` and `qa:live` were considered and dropped.) | "UAT" is the term `qa:strategy` already uses for this layer. |
| D2 | The consolidated report is local, `.ai-gent/uat/<slug>/report.md` (git-excluded), and is not a pipeline artifact; `shared/pipeline.md` does not change. | It holds validators' words and test-session details that do not belong in the repository; the pipeline needs no new owner or status. |
| D3 | Preflight is a guided checklist the owner confirms plus a test read of the register at `view`, `interact` and `admin` after publishing (FR-092, FR-093). | The agent cannot read the sharing menu; it can prove the register's rules for reading, and each validator's R0 write proves their own access. |
| D4 | Periodic follow-up is off by default; on only at the owner's request, session-bound, with its limits stated, and every report giving the last real check (FR-077, FR-095). A cloud routine is a future extension, assumed not to work. | A timer that silently stops misleads; reading on request is honest by default. |
| D5 | The owner assigns roles when listing validators; a validator may propose another in R0; the owner decides and wins (FR-091, FR-026). | Coverage by role is only meaningful if roles are not self-declared. |
| D6 | Attachments are off by default; evidence is structured text and links; the owner may turn attachments on per session with a size budget and the warnings (FR-050, FR-094). | Text was enough in the pilot; uploads carry privacy risk and do not work for every outside-organization guest. |
| D7 | `ai-gent uat` is a subcommand of the main binary, like `delegate`, not its own crate. | Small, no heavy dependency; the size reason that made `compact` its own crate (D29) does not apply. |
| D8 | Milestone 3.x, after 3.0.0 (like spec 041); the page is tested by a behavior eval and a manual pilot with no new CI job; mode B reuses the existing Spec Kit reader (`spec::parse`). | A new subcommand, not a port; a browser job for one page is not worth its CI cost. |
| D9 | Validator labels on the page are the label the owner chooses (a first name, initials or "Validator 2"), never an e-mail or account. | Enough to tell people apart without exposing accounts. |
| D10 | When the sharing menu has no level between Commenter and Editor, a session may use Editors; FR-097 detects a republish before the agent's next republish. | The owner accepts the risk for a session; an organization-only rule stays a choice the owner can make. |

## Open questions

None open. Answered by the owner, 2026-11-19, and now Decisions D9 and D10.

## Possible extensions (out of scope)

- A cloud routine for follow-up, once it is shown to read an Artifact's register (D4).
- Assisted clean-up of stale marks with per-step choices (keep as stale, ask the validator to redo).
- Coverage by role as a closing gate: closing refuses while a step is not covered, unless the owner overrides.
- A CSV of marks next to `report.md` for spreadsheets.
- Mode B traceability in the report: a scenario-to-step table and `[UPSTREAM GAP]` notes for scenarios that turned out untestable or wrong (reported to the owner, never edited into the feature's spec).
- Message templates in the owner's language, chosen per session.
- Roadmap quality metrics: claims per evidence label, steps rewritten after publish, NOKs classified `roadmap`, steps covered by role, time from NOK to reply.
- Triage notes visible on the page (FR-043): "under analysis", "fixed in the test environment", "decision pending".
- A retest list: after a defect is fixed in the test environment, the steps to retest and their validators.
- Live exclusivity (who is executing an exclusive step now) with the `room` capability, for presence only, never as authority.
