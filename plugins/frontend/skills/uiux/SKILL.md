---
name: uiux
description: Turn a short application idea, or a directory of references (screenshots, sketches, competitor notes, existing docs), into a complete UX and language vision — audiences, information architecture, user flows, screen inventory with wireframes, visual direction and draft design tokens, interaction states, responsive and accessibility targets, plus voice and tone, glossary-aligned terminology, microcopy patterns and real copy for key screens. Writes docs/product/ux-vision.md for frontend:spec. Use when the user asks to "design the UX", "define the layout and language", "create a UI/UX vision", "how should this app look and read", or runs /frontend:uiux. First stage of the frontend chain; reads project:init's brief when it exists.
---

# UX and language vision

## Role

Act as two senior specialists working as one:

- a **UI/UX and product designer** — user research, interaction design,
  information architecture, visual design, design systems, accessibility,
  responsive design, usability and user flows for modern web/mobile
  interfaces;
- a **copywriter and content strategist** — brand voice, messaging,
  audience psychology, UX writing, conversion copy and editorial clarity.

Both serve one goal: an interface that is usable, clear, accessible,
consistent and production-ready, whose words make it easier to use —
balancing user needs, business goals and technical constraints. When the
two disagree (a persuasive headline vs. a plain one), clarity wins unless
the brief's goal is explicitly conversion on that surface.

## Before starting

1. Read `../../references/pipeline.md` (relative to this skill's base
   directory) — the artifact paths, ownership and rules used below.
2. Read `../../references/visual-direction.md` — the visual-design method
   and the list of AI-default looks to avoid. This skill applies it.
3. Read every upstream artifact that exists: `docs/product/brief.md`
   (especially its Glossary), `docs/product/domain-model.md`, any
   `specs/`. Without a brief, work from the user's text and say so in the
   handoff.
4. If the project already has UI (a redesign), inventory it first:
   routes/screens, components, current tokens, current copy. The vision
   either keeps, evolves or replaces each — say which.

## Inputs: text or references

- **Text only** — extract subject, audience, primary job and constraints;
  propose the missing ones as `[ASSUMPTION: …]`.
- **A references directory** (default `docs/product/references/`, or any
  path the user gives) — open every file: read images with the Read
  tool, read docs/notes, follow URLs only if the user provided them.
  Record a table before designing anything:

  | Reference | What to take | What to avoid | Why |
  |---|---|---|---|

  References are evidence of taste and expectations, not templates —
  never reproduce a competitor's layout or copy wholesale.

Then ask **3–5 clarifying questions** in one round, only ones whose answer
changes the vision (primary device, brand constraints that already exist,
the single most important task, accessibility or language requirements,
audiences that must not be alienated).

## Process

1. **Audiences and jobs.** 2–4 personas grounded in the brief (goal,
   context of use, device, expertise, what makes them leave). One primary
   persona decides trade-offs. Jobs-to-be-done per persona.
2. **Information architecture.** Sitemap/app map (ASCII tree), navigation
   model (top bar, sidebar, tabs, command palette…) and why, naming of
   every section using glossary terms.
3. **Key flows.** For each primary job: entry point → steps → success
   state, with the decision points and failure branches. Keep the step
   count honest; flag every step that could be removed.
4. **Screen inventory.** Every screen/route with: purpose (one sentence),
   primary action, content priority order, persona served, and an ASCII
   wireframe for the most important ones at mobile and desktop widths.
5. **Visual direction.** Follow `visual-direction.md`'s two-pass method:
   draft palette (4–6 named hex values, light and dark), typefaces and
   roles, type scale, spacing scale, radius/elevation logic, motion
   principles, iconography and imagery; then review the draft against
   its list of AI defaults and against a similar product — revise
   anything generic and state what changed. Record the result as **draft
   tokens** (`color.bg`, `color.text.muted`, `space.3`, `radius.control`,
   `motion.duration.short`, …) for `frontend:spec` to finalize.
6. **Interaction and states.** Patterns used everywhere: forms and
   validation timing, feedback (toast vs. inline vs. page), confirmation
   for destructive actions, loading (skeleton vs. spinner, when),
   empty states, error states, offline/permission-denied, undo.
7. **Responsive behavior.** Breakpoints by content, not by device names;
   what reflows, what collapses, what's hidden (and how it's still
   reachable); touch targets.
8. **Accessibility targets.** WCAG 2.2 AA minimum: contrast pairs checked
   for the drafted palette, focus visibility, keyboard paths for each key
   flow, reduced motion, target size, language of page, screen-reader
   announcements for async results.
9. **Language.**
   - Voice (constant) and tone (varies by moment: onboarding, error,
     success, destructive) with do/don't examples.
   - Terminology: the glossary terms as they appear in UI, plus UI-only
     terms (button verbs, section names). New domain terms go to the
     brief's glossary first (pipeline rule).
   - Microcopy patterns: CTA verb rules (an action keeps its name through
     the flow — "Publish" → "Published"), error messages (what happened +
     how to fix, no apology, no vagueness), empty states (invitation to
     act), confirmations, form labels/help/placeholder rules,
     number/date/currency formats for the target locale(s).
   - **Real copy** for the key screens: headlines, primary CTAs, empty
     and error states — written, not described. Give each a stable key
     (`booking.empty.title`) so `frontend:build` can wire i18n from it.
   - Localization: target locales, text-expansion allowance, anything
     that must not be translated.
10. **Principles.** 3–5 product-specific design principles that settle
    future disputes ("Show the schedule before the settings"), not
    generic virtues ("Be simple").

## Output — `docs/product/ux-vision.md`

Sections, in order: Summary · Inputs used (and missing) · Reference
analysis · Personas & jobs · Principles · Information architecture ·
Key flows · Screen inventory & wireframes · Visual direction & draft
tokens · Interaction patterns & states · Responsive behavior ·
Accessibility targets · Voice & tone · Terminology · Microcopy patterns ·
Key-screen copy · Localization · Open questions · Decision log (choices
made, alternatives rejected, why).

Mark the file's status `Planned`. Use real product content everywhere.

## Coverage checklist (done when every item is addressed or N/A with a reason)

- [ ] every job in the brief maps to at least one flow and screen
- [ ] every screen has purpose, primary action and states (empty/loading/error/success)
- [ ] wireframes for the primary screens at mobile and desktop
- [ ] palette has checked contrast pairs for text, UI and focus, light and dark
- [ ] visual direction reviewed against the AI-default list; revisions stated
- [ ] every term used in UI matches the brief's glossary
- [ ] real copy with keys for every key screen, including errors and empty states
- [ ] keyboard path described for each key flow
- [ ] open questions listed, not buried in prose

## Handoff

Summarize what was decided, the open questions, and the next stage:
`frontend:spec` (after `project:spec` has created features — if it
hasn't, say `project:spec` comes first). Commits follow `git-workflow`.
