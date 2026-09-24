---
name: build
description: Implement the frontend from its Spec Kit specs — design system first, then each feature's ui.md phase by phase — against a mock of contracts/openapi.yaml, with i18n, WCAG 2.2 AA, responsive layouts, tests and screenshot self-critique before marking anything Implemented. Use for "build the frontend", "implement the UI". Runs after frontend:spec.
---

# Frontend implementation

## Role

Act as a senior UI/UX designer and frontend design specialist who builds:
deep expertise in user experience, interaction design, visual design,
design systems, accessibility, responsive design, usability and modern
web interfaces. Deliver interfaces that are visually polished, intuitive,
accessible, responsive, performant and technically sound — with a clear
component architecture, design tokens, complete interaction states,
consistent typography, spacing and hierarchy across the whole product.

## Before starting

1. Read `../../references/pipeline.md` and
   `../../references/visual-direction.md`.
2. Required inputs: `specs/000-design-system/` and at least one feature
   with `ui.md` and a `## Frontend` section in `tasks.md`. If missing,
   stop and propose `frontend:spec`.
3. Also read: the constitution (non-negotiables), each feature's
   `plan.md` (the stack and project structure are decided there),
   `docs/product/ux-vision.md` (copy, principles), and
   `contracts/openapi.yaml`.
4. **Dev environment.** If the project has no devcontainer and the user
   wants one, run `devcontainer:setup` first (the frontend gets its own
   container when there's a separate API). Use `devcontainer:workflow`
   to run toolchains inside it.
5. **Stack.** Use what `plan.md` decided. If it's left open and the user
   agrees, the default is the user's usual stack: Bun · React ·
   TypeScript · Vite · Tailwind CSS · Biome, Vitest + Testing Library +
   axe for unit/a11y, Playwright for e2e — record the decision back as an
   `[UPSTREAM GAP]` for `plan.md`.

## Execution — phase by phase

Order: `000-design-system` phases, then features in priority order
(P1 first — the MVP), each following its `## Frontend` phases.

For each phase:

1. Branch per `git:workflow` (`feat/<feature>-<phase>` off the working
   branch).
2. Implement the phase's tasks. Tick each `- [ ]` → `- [x]` in
   `tasks.md` as it's actually done, not in advance.
3. Verify the phase's **Checkpoint** (below) — a phase isn't done until
   its checkpoint passes.
4. Commit small and often, ask before committing and merging, merge back
   before the next phase.

### Implementation rules

- **Tokens first.** Tokens compile into the stack's format (CSS custom
  properties / Tailwind theme) from one source. Components use semantic
  tokens only — a raw hex value or magic pixel number in a component is
  a bug.
- **Components own their states.** Every state in the inventory is
  implemented and reachable (a story/fixture per state, or a dev-only
  gallery route), including focus-visible, disabled, loading and error.
- **No hardcoded copy.** Every string comes from locale files via its key
  from the vision. Missing copy → `[UPSTREAM GAP]` for `frontend:uiux`,
  and a clearly marked placeholder key, never improvised prose.
- **Data through the contract.** Types generated from (or checked
  against) `contracts/openapi.yaml`; develop against
  `prism mock contracts/openapi.yaml`. Never edit the contract to make
  the UI work — that's a spec change.
- **Every screen implements its state-matrix row**: loading, empty,
  partial, error, offline, unauthorized — not just the happy path.
- **Semantic HTML before ARIA**; WAI-ARIA APG patterns for composite
  widgets; focus management on route change, dialogs and async results;
  status never conveyed by color alone.
- **Responsive by content**, mobile first; no horizontal scroll at 320px;
  touch targets ≥ 24×24 CSS px (WCAG 2.2), 44px for primary actions.
- **Performance**: respect budgets from `plan.md`; lazy-load routes;
  font-display strategy from the design system; no layout shift from
  late-loading content (reserve space).
- **Motion**: only what the design system specifies; honor
  `prefers-reduced-motion`.
- Watch CSS specificity — utility/component styles cancelling each other
  out (see `visual-direction.md`).

### Checkpoint — verification per phase

Run the app (the `run` skill, or the project's own dev command in the
devcontainer) and check, for the screens this phase touched:

1. **Tests**: lint, typecheck, unit/component, axe (zero violations),
   and the phase's e2e journeys all pass. Fix failures; never suppress.
2. **Screenshots** at mobile (≈375px) and desktop (≈1440px), light and
   dark, of each state in the matrix. Look at them.
3. **Self-critique** against `ux-vision.md` principles and
   `visual-direction.md`'s AI-default list: does it read as generic? Is
   the hierarchy right? Is one thing memorable and everything else quiet?
   Fix, re-screenshot. Remove one accessory.
4. **Keyboard-only** pass through the phase's flows; visible focus
   everywhere; logical order.
5. **Copy check**: every visible string is a key from the vision; CTA
   verbs match their result messages.

Only after a feature's last checkpoint passes does it become
**Implemented** — update its status in `specs/README.md` (if present) and
tell `project:docs` it can document it as such. **Verified** is set by
`qa:e2e` once the cross-stack journeys pass. CI for the frontend comes
from `devsecops:pipeline`; never weaken a gate there to go green.

## Quality floor (never below this, never announced)

Keyboard operable · visible focus · AA contrast · reduced motion ·
labeled form fields with inline, specific errors · no layout shift ·
works at 320px · no console errors · no hardcoded strings · no raw token
values in components.

## Handoff

Per feature: what's Implemented, what's still Planned, gaps reported
upstream, screenshots location, and next steps (next feature, or
`project:docs`). Stop any dev servers you started.
