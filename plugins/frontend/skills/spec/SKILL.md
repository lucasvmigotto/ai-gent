---
name: spec
description: Turn the UX vision (docs/product/ux-vision.md) into a phased, implementation-ready frontend specification in GitHub Spec Kit format — a 000-design-system feature (final tokens, typography, component inventory with states and variants, motion, breakpoints, accessibility rules) plus a ui.md layer on every existing feature that has UI (routes, screens, wireframes per breakpoint, components, data needs mapped to contract operations, state matrix, validation and microcopy keys, accessibility and test plan), and Frontend tasks appended to each tasks.md. Updates existing specs rather than duplicating them. Use when the user asks to "spec the frontend", "plan the UI implementation", "write the frontend speckit specs", or runs /frontend:spec. Second stage of the frontend chain, after frontend:uiux and project:spec.
---

# Frontend specification (Spec Kit)

## Role

Act as a senior UI/UX and product designer specializing in user-centered
design, interaction design, information architecture, visual design,
design systems, accessibility, responsive interfaces and usability —
writing specifications an engineer can implement without guessing.
Prioritize clarity, consistency, accessibility, visual hierarchy,
intuitive interactions, responsive behavior and production readiness,
balancing user, business and technical requirements.

## Before starting

1. Read `../../references/pipeline.md` — ownership rules matter here:
   this skill **adds a UI layer** to features `project:spec` owns.
2. Read `../../references/visual-direction.md` for token/component
   decisions.
3. Required inputs:
   - `docs/product/ux-vision.md` — if missing, stop and propose running
     `frontend:uiux` first.
   - `specs/` with Spec Kit features and `.specify/` — if missing,
     propose `project:spec` first. **Exception:** a frontend-only project
     (no backend in scope, confirmed with the user) — then bootstrap
     Spec Kit per `pipeline.md` and create the UI features yourself with
     `/speckit-specify`, owning them.
4. Also read: `docs/product/brief.md` (glossary), the constitution,
   every feature's `spec.md`/`plan.md`/`tasks.md`, and
   `contracts/openapi.yaml` if present.

## Process

### 1. Coverage matrix

Map the vision onto the features before writing anything:

| Screen / flow (ux-vision) | Feature (`specs/NNN-*`) | User story | Contract operations |
|---|---|---|---|

Every screen lands in exactly one feature. A screen with no feature, or a
feature user story with no screen, is an `[UPSTREAM GAP]` — report it and
propose the fix to `project:spec` or `frontend:uiux`; don't invent a
feature.

### 2. `specs/000-design-system/`

The foundation every UI feature depends on. Create it with Spec Kit's
templates (`spec.md`, `plan.md`, `tasks.md`) plus:

- **Tokens** — final values from the vision's draft, as a table and as the
  implementation format the stack will use (CSS custom properties, a
  Tailwind theme, a tokens JSON). Semantic layer (`color.text.muted`) over
  primitive layer (`gray.600`); light and dark; contrast pairs listed with
  their ratio.
- **Typography** — families, loading strategy, scale with line-heights,
  usage per role.
- **Layout** — grid, container widths, breakpoints, spacing rules.
- **Component inventory** — every component the screens need, grouped
  (primitives, composites, patterns). Per component: purpose, anatomy,
  variants, sizes, **states** (default, hover, focus-visible, active,
  disabled, loading, error, selected/checked, empty), keyboard
  interaction, ARIA pattern (name the WAI-ARIA APG pattern when one
  applies), content rules (label length, truncation), and do/don't.
- **Motion** — durations/easings as tokens, which interactions animate,
  reduced-motion behavior.
- **Accessibility rules** — the WCAG 2.2 AA targets from the vision as
  testable rules (focus ring spec, minimum target size, contrast, live
  region usage).
- **i18n** — string key convention (from the vision's copy keys), locale
  files layout, formatting utilities.

### 3. `ui.md` for every feature with UI

In `specs/NNN-<feature>/ui.md`, per the feature's user stories:

- **Routes** — path, params, auth requirement, title, breadcrumbs.
- **Screens** — for each: purpose, content priority, ASCII wireframe at
  mobile and desktop (more breakpoints only if the layout genuinely
  changes), components used (from the inventory — a new one goes back
  into `000-design-system`).
- **Data** — per screen, the contract operations it calls (by
  `operationId`), when (on load, on action, polling), caching/revalidation,
  optimistic updates, and what the UI does with each error status.
- **State matrix** — rows = screens/regions, columns = loading · empty ·
  partial · success · error · offline · unauthorized/forbidden · stale;
  each cell says what is shown and which copy key.
- **Interactions** — validation rules and timing, submit/disable
  behavior, destructive-action confirmation, undo, keyboard shortcuts.
- **Copy** — every string by key, taken from the vision; missing copy is
  an `[UPSTREAM GAP]` for `frontend:uiux`, not something to improvise.
- **Accessibility** — landmarks, heading outline, focus order and focus
  management on route change/dialog/async result, announcements.
- **Responsive** — what changes per breakpoint.
- **Acceptance criteria** — Given/When/Then at UI level, linked to the
  spec's user stories and success criteria.
- **Test plan** — component tests, a11y (axe) checks, e2e journeys,
  visual checks per breakpoint and theme.

### 4. Frontend tasks

Append a `## Frontend` section to each feature's `tasks.md` (and fill
`000-design-system/tasks.md`) in Spec Kit's task format, phased like the
template: Setup → Foundational → one phase per user story in priority
order → Polish. Each task names its file path, marks `[P]` only when it
truly touches different files with no dependency, and ends each phase
with a **Checkpoint** line stating how to verify it. Include a task to
run the contract mock (`prism mock contracts/openapi.yaml`) in Setup so
UI work never waits on the backend.

Technical frontend decisions not covered by the feature `plan.md`
(state management, data-fetching library, form library, testing tools)
go in `000-design-system/plan.md` with rationale — the stack itself comes
from `plan.md`/the constitution; don't override it.

### 5. Consistency pass

Run `/speckit-analyze` (or follow its skill file) across the updated
features and fix what it finds in *your* layer; report the rest.

## Coverage checklist

- [ ] every screen in the vision appears in exactly one feature's `ui.md`
- [ ] every component referenced exists in the inventory with all its states
- [ ] every screen has a filled state-matrix row
- [ ] every data need maps to a contract `operationId` (or an upstream gap)
- [ ] every string is a copy key present in the vision
- [ ] tokens have light/dark values and listed contrast ratios
- [ ] every phase in `## Frontend` has a checkpoint
- [ ] `/speckit-analyze` run; findings resolved or reported

## Handoff

List the features updated, the gaps reported upstream, and the next
stage: `frontend:build`, starting with `000-design-system`. Commits
follow `git-workflow`.
