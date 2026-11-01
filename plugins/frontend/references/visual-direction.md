# Visual, Interaction & Content Direction

**Type:** Shared frontend reference
**Consumers:** `frontend:uiux`, `frontend:spec`, `frontend:build`
**Canonical path:** `plugins/frontend/references/visual-direction.md`

The shared standard for shaping, specifying and implementing a product's visual and communicative experience. It is not a workflow and does not replace the responsibilities or gates of the consuming skills; where a skill defines a more specific procedure, the skill wins.

Three stages stay aligned through it. Each owns its artifacts: a downstream stage may report an upstream gap but never silently rewrites another stage's decisions (§7).

Load only what your stage needs:

| Stage | Read |
|---|---|
| `frontend:uiux` | this file |
| `frontend:spec` | this file and `design-tokens.md` |
| `frontend:build` | this file and `build-verification.md` (`design-tokens.md` only when a value is unclear) |

---

## 1. Start with the product, not a visual style

A design direction answers a product's purpose. Do not begin with a fashionable palette, typeface pairing or familiar page template.

Before consequential choices, understand:

- **Product:** what it is and what it enables.
- **Audience:** who uses it, their context, expertise and constraints.
- **Primary job:** what people must be able to accomplish.
- **Content:** the information, actions and terminology it carries.
- **Identity:** what makes it recognizable and credible.
- **Constraints:** requirements, approved decisions, platform, accessibility, technology, delivery.

Read the upstream artifacts, especially `docs/product/brief.md` and `docs/product/ux-vision.md`, and inspect existing screens when evolving a product. Treat evidence carefully:

- Confirmed requirements are constraints; approved decisions bind unless the owner reopens them.
- Existing implementation shows current behavior, not correct behavior.
- Label inferences and proposals as such.
- Raise missing information that could materially change the result through the owning skill's clarification process.

Never present invented product facts, user needs, brand history, testimonials, metrics or interface copy as real.

## 2. Establish a design hypothesis

Before detailed treatments, state one concise design hypothesis:

1. What the experience helps the user do.
2. The quality or feeling that characterizes it.
3. The product-specific idea that gives it identity.
4. What stays visually subordinate.

"A field instrument for exploring the night sky" beats "a modern dark dashboard": it drives navigation, density, typography, motion and hierarchy. The hypothesis is a decision tool, not marketing copy; it must be specific enough to reject choices that do not belong.

### Explore before committing

When the brief leaves meaningful visual choices open, develop two or three genuinely different composition directions. Vary the structure, not just the accent color. For each, state its organizing idea, how it supports the primary task, its defining visual characteristic and its main trade-off. Use short prose and simple wireframes. Choose on product fit, usability and feasibility; never invent alternatives to look thorough.

## 3. Build a coherent visual system

The system expresses the design hypothesis and stays usable across the product. Details per area (color, typography, layout, shape, imagery, motion) are in `design-tokens.md`, sections 3.1 to 3.5.

Core rules:

- Semantic tokens are the interface contract; components consume roles, never raw values.
- Verify contrast for every foreground/background pair and every required theme.
- Use color, borders, cards, labels and motion only where they carry meaning. Do not repeat one radius, shadow or container everywhere.

### Type and layout rules of thumb

- Keep body lines under about 80 characters; serif text tolerates slightly longer lines.
- Give serif body text a little more line-height than sans-serif.
- Left-align information-heavy content; center only short compositions.
- Skip all-caps labels and single-word accents (one italic, bold or colored word in a headline) unless they do real work.
- Use numbered markers only for a true sequence, such as steps or a timeline.
- Spend boldness in one place: one memorable element, with everything around it quiet.

### Motion principle

Motion triggered by the user's action (opening, expanding, confirming) is welcome when it shows what changed. Motion that nobody triggered should be one orchestrated moment, such as a single load sequence or reveal, not scattered effects.

---

## 4. Design for tasks, states and access

A screen succeeds when users understand it and finish their task, not when its default state looks polished. For each important screen and component, consider:

- Purpose, primary action, information priority and reading order.
- Entry points and navigation context.
- Default, hover, focus-visible, active, selected and disabled states.
- Loading, empty, partial, success, error, offline and permission states.
- Validation, confirmation, undo and recovery.
- Keyboard operation and assistive-technology output.
- Responsive behavior and content overflow.

Skip states that do not apply, but never omit one because it is inconvenient to design.

### Accessibility is a design constraint

WCAG 2.2 AA is the minimum; an approved requirement may raise it, never lower it. Account for it when choosing patterns, tokens, content and motion, not as a final audit:

- Semantic structure and meaningful accessible names.
- Keyboard access and visible focus.
- Contrast for text, controls, boundaries and focus indicators.
- Reflow, zoom, text resizing and appropriate target sizes.
- Clear labels, instructions and error recovery.
- Focus management and status announcements.
- Reduced motion and non-color status cues.

Use native HTML before ARIA. For a composite widget, pick an established pattern and specify its keyboard behavior rather than inventing a model.

### Responsive design is behavioral

Mobile is not a compressed desktop. Define what changes as space and input change:

- What reflows, stacks, collapses or moves, and how navigation changes.
- How dense data stays usable and images crop.
- How touch and pointer differ.
- How hidden content stays reachable.

Set breakpoints where the composition needs to change, not by device label, and validate narrow and intermediate widths too.

---

## 5. Make the interface speak with one voice

Content is part of the interaction model: it names objects, explains choices, sets expectations and confirms outcomes. Before writing, establish audience and knowledge level, objective, brand voice and situational tone, terminology and glossary, primary message, desired action, and localization needs.

### Writing standards

- Be clear, specific and natural; use active voice and direct verbs.
- Name actions by outcome: "Save changes", not "Submit".
- Keep terminology identical across navigation, controls, help and feedback.
- Explain what something does instead of promoting it.
- Cut filler, vague labels and jargon; give each message one job.
- Match tone to the moment without changing the voice.

An action keeps its name through the flow: "Publish" is confirmed by "Published". Errors say what happened and, when possible, how to recover. Empty states say what is absent and offer a next step. Neither relies on vague apologies or humor.

### Content hierarchy and persuasion

Order content by user intent and put what the next action needs where users will find it. When conversion is an explicit objective, connect the audience's need to a credible value proposition, evidence, objection handling and a clear next action; never manufacture urgency, guarantees, social proof or claims.

For real interfaces the approved vision and glossary are the source of truth. If copy or a term is missing, report an upstream gap through the owning skill.

---

## 6. Protect the product's identity from generic defaults

Common patterns are tools, not recipes; one is right when it fits the product, content, audience and task, not because it is familiar. In concept review, challenge choices that resemble interchangeable generated interfaces:

- A hero of large headline, decorative gradient and generic call to action with no product reason.
- Rounded cards as the default container for unrelated content.
- The same shadow, radius and surface on every component.
- Cream-and-serif, near-black-and-neon or editorial-grid looks unconnected to the brief.
- Uppercase eyebrows, arbitrary metadata strings, decorative numbering or a label above every heading.
- Monospace as decoration rather than for code or aligned data.
- Gradients, blobs, floating shapes and icon clusters that explain nothing.
- Stock imagery that could represent any industry.
- Motion applied uniformly instead of in response to an event.

This is a diagnostic list, not a ban: any item may suit a product. Explain the fit instead of inheriting the pattern.

### Differentiation review

Before a visual direction counts as ready, answer:

1. Could it plausibly belong to many unrelated products?
2. Which decisions come directly from this product's subject, audience or content?
3. Is the defining visual characteristic meaningful and useful?
4. Does the hero or primary screen show the product's actual purpose?
5. Are hierarchy and composition driven by user tasks?
6. Is any pattern present only because it is familiar?
7. What can be removed without losing clarity or identity?

Revise weak choices and record the material changes and reasons.

---

## 7. Stage boundaries

Each stage keeps the same design rationale at a different level of detail. The skills define the ownership and procedure; this file adds only the shared direction.

- `frontend:uiux` owns the UX and language vision; see its skill.
- `frontend:spec` owns the design-system and per-feature UI specifications; see its skill.
- `frontend:build` owns implementation and verification of the approved spec; see its skill.

## 8. Review the rendered result

Inspect the rendered experience before calling it complete, and never claim visual verification that did not happen. `frontend:build` follows the procedure and CSS practice in `build-verification.md`.

## 9. Cross-stage gate

Each skill's own checklist covers its stage's items. Before handing work on, confirm only these cross-stage items, reporting any outside your ownership as a gap rather than taking it over:

- [ ] The design hypothesis traces to the product, audience and primary tasks, and approved decisions are preserved.
- [ ] Assumptions, open questions and upstream gaps are recorded.
- [ ] The artifact is in the right place, owned by the right stage, and the next stage can proceed without guessing.
- [ ] Verification claims state exactly what was checked.

---

## Operating principle

Build the experience outward from the product's purpose: understand the user, set a rationale, specify behavior, use language to make the product understandable, and inspect before claiming completion. The standard is neither novelty nor fashion but a distinctive, usable, accessible, buildable experience in which every important decision has a reason.
