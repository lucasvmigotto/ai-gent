# Visual, Interaction & Content Direction

**Type:** Shared frontend reference  
**Consumers:** `frontend:uiux`, `frontend:spec`, `frontend:build`  
**Canonical path:** `plugins/frontend/references/visual-direction.md`

This reference defines the shared standards for shaping, specifying, and implementing the visual and communicative experience of a product. It is not a standalone workflow and does not replace the responsibilities or gates defined by the consuming skills.

Its purpose is to keep three stages aligned:

- `frontend:uiux` establishes the product's experience, visual direction, voice, terminology, and real interface copy in `docs/product/ux-vision.md`.
- `frontend:spec` translates the accepted vision into implementation-ready design-system and per-feature UI specifications.
- `frontend:build` implements those specifications and verifies the rendered experience.

Each stage owns its artifacts. A downstream stage may identify and report an upstream gap, but must not silently take ownership of or rewrite another stage's decisions. Follow the pipeline contract and the consuming skill's instructions whenever they define a more specific procedure.

---

## 1. Start with the product, not a visual style

A design direction is a response to a product's purpose. Do not begin by selecting a fashionable palette, a typeface pairing, or a familiar page template.

Before making consequential choices, understand:

- **Product:** what it is and what it enables.
- **Audience:** who uses it, their context, expertise, expectations, and constraints.
- **Primary job:** what people must be able to accomplish.
- **Content:** what information, actions, and terminology the experience must carry.
- **Identity:** what should make this product recognizable and credible.
- **Constraints:** business requirements, approved decisions, platform capabilities, accessibility, technology, and delivery limits.

Read the available upstream artifacts, especially `docs/product/brief.md` and `docs/product/ux-vision.md`. Inspect existing screens and conventions when evolving an existing product.

Treat evidence carefully:

- Confirmed requirements are constraints.
- Approved decisions are binding unless the owner reopens them.
- Existing implementation is evidence of current behavior, not proof that the behavior is correct.
- Inferences and proposals must be labelled as such.
- Missing information that could materially change the result must be raised through the owning skill's clarification process.

Do not invent product facts, user needs, brand history, testimonials, metrics, or interface copy and present them as real.

## 2. Establish a design hypothesis

Translate the product understanding into one concise design hypothesis before choosing detailed treatments.

A useful hypothesis states:

1. What the experience should help the user do.
2. What quality or feeling should characterize the experience.
3. Which product-specific idea will give the interface its identity.
4. What should remain visually subordinate.

For example, “a field instrument for exploring the night sky” is a more useful design premise for an astronomy product than “a modern dark dashboard.” It connects the subject to decisions about navigation, information density, typography, motion, and visual hierarchy.

The hypothesis is a decision-making tool, not marketing copy. It should be specific enough to reject design choices that do not belong.

### Explore before committing

When the brief leaves meaningful visual choices open, develop two or three genuinely different composition directions. Vary the underlying structure, not just the accent color.

For each direction, briefly state:

- Its organizing idea.
- How it supports the primary user task.
- Its defining visual characteristic.
- Its main trade-off.

Use short prose and simple wireframes where they clarify the comparison. Choose a direction based on product fit, usability, and feasibility. Do not create alternatives merely to make the process look thorough.

## 3. Build a coherent visual system

The visual system should express the design hypothesis and remain usable across the product. Define roles and relationships before assigning isolated values.

### 3.1 Color

Create a restrained foundational palette, normally four to seven named colors, when the project does not already have an approved system.

Define both:

- **Primitive tokens:** raw palette values such as `violet.700` or `amber.300`.
- **Semantic tokens:** purpose-based roles such as `surface.canvas`, `text.primary`, `border.subtle`, `action.primary`, and `feedback.error`.

Semantic tokens are the interface contract. Components should consume semantic roles rather than hardcoded hex values.

Specify light and dark themes when both are required. Identify foreground/background pairs and verify applicable contrast requirements. Do not assume that a palette is accessible because its individual colors look distinct.

Use color to communicate hierarchy, category, and state. Avoid assigning an accent simply because a design needs “a pop of color.”

### 3.2 Typography

Choose typefaces for the product's character, language coverage, readability, and technical constraints.

Specify:

- Typeface families and their roles.
- Font loading and fallback strategy.
- Type scale, weights, line heights, and letter spacing.
- Heading and body-text behavior.
- Functional typography for labels, data, and code.
- Readable line lengths and responsive scaling.

One family can be sufficient. A second family should have a clear role rather than exist for variety. Reserve monospace for content that benefits from fixed-width character alignment, such as commands, code, or genuinely tabular data.

Treat typography as part of the composition. Avoid arbitrary emphasis, excessive font styles, and typographic decoration that does not improve meaning or hierarchy.

### 3.3 Layout and spacing

Define a layout system that reflects the information architecture and content density.

Specify, as relevant:

- Content and reading widths.
- Grid and column behavior.
- Alignment rules.
- Spacing scale.
- Section rhythm.
- Breakpoints and reflow rules.
- Relationship between navigation and content.
- Overflow behavior for dense or fixed-width material.

Use left alignment for most information-heavy content because it supports scanning. Center alignment is appropriate for focused, short compositions when it serves the content. Justified text requires deliberate control and should not be used by default.

Do not force every screen into the same grid. Reuse layout rules where they help users understand relationships; vary composition where the content or task requires it.

### 3.4 Shape, borders, elevation, and imagery

Define a small, purposeful vocabulary for:

- Corner radii.
- Borders and dividers.
- Shadows and elevation.
- Surface layers.
- Icons and illustration.
- Photography, diagrams, and other media.

Use structural treatments to explain grouping, hierarchy, interaction, or state. A border, card, badge, label, or divider should have a reason to exist.

Do not apply the same radius, shadow, card container, or decorative treatment to every element. Avoid decoration that competes with the product's actual content.

### 3.5 Motion

Motion should explain a transition, show the result of an action, or direct attention to a meaningful change.

Define motion tokens and specify where motion is used. Prefer interaction-triggered motion over ambient animation. Avoid repeating entrance effects across every section or adding hover animation to every element by habit.

Respect `prefers-reduced-motion`. Essential information and functionality must remain available without animation.

---

## 4. Design for tasks, states, and access

A screen is successful when users can understand it and complete its intended task—not merely when its default state looks polished.

For each important screen and component, consider:

- Purpose and primary action.
- Information priority and reading order.
- Entry points and navigation context.
- Default, hover, focus-visible, active, selected, and disabled states where applicable.
- Loading, empty, partial, success, error, offline, and permission states where relevant.
- Validation, confirmation, undo, and recovery behavior.
- Keyboard operation and assistive-technology output.
- Responsive behavior and content overflow.

Do not specify states mechanically when they do not apply. Equally, do not omit a state simply because it is inconvenient to design.

### Accessibility is a design constraint

Include accessibility when choosing patterns, tokens, content, and motion—not as a final cosmetic audit.

At minimum, account for the project's target standard (the frontend pipeline uses WCAG 2.2 AA unless an approved requirement says otherwise), including:

- Semantic structure and meaningful accessible names.
- Keyboard access and visible focus.
- Contrast for text, controls, boundaries, and focus indicators.
- Reflow, zoom, text resizing, and readable content.
- Appropriate target sizes.
- Clear labels, instructions, and error recovery.
- Focus management and status announcements.
- Reduced motion and non-color status cues.

Use native HTML semantics before adding ARIA. Where a composite widget is needed, choose an established interaction pattern and specify its keyboard behavior. Do not invent custom interaction models when a well-understood accessible pattern fits.

### Responsive design is behavioral

Do not treat mobile as a compressed desktop layout. Define what changes as available space and input methods change:

- What reflows, stacks, collapses, or moves.
- Which navigation controls change.
- How dense data remains usable.
- How images crop or resize.
- How touch and pointer interactions differ.
- How content remains reachable when hidden from the initial view.

Choose breakpoints where the composition needs to change, not solely by device labels. Validate narrow and intermediate widths, not only a standard phone and desktop size.

---

## 5. Make the interface speak with one voice

Content is part of the interaction model. It names objects, explains choices, sets expectations, and confirms outcomes.

Before writing, establish:

- Audience and knowledge level.
- Communication objective.
- Brand voice and situational tone.
- Product terminology and glossary.
- Primary message and supporting information.
- Desired user action.
- Localization and formatting requirements.

### Writing standards

Prefer language that is clear, specific, natural, and appropriate to the audience.

- Use active voice and direct verbs.
- Name actions by their outcome: “Save changes,” not “Submit.”
- Keep terminology consistent across navigation, controls, help, and feedback.
- Explain what something does rather than relying on promotional claims.
- Remove filler, vague labels, and unnecessary jargon.
- Keep each message focused on one job.
- Match the tone to the moment without changing the underlying voice.

An action should retain its name throughout the flow. If the user selects “Publish,” the resulting confirmation should say “Published.”

Errors should state what happened and, when possible, how to recover. Empty states should explain what is absent and offer a useful next step. Neither should rely on vague apologies or decorative humor.

### Content hierarchy and persuasion

Order content according to user intent and decision-making needs. Put the information required for the next action where users can find it.

When conversion is an explicit product objective, connect the audience's need to a credible value proposition, relevant evidence, objection handling, and a clear next action. Do not manufacture urgency, guarantees, social proof, or claims.

For real product interfaces, use the approved vision and glossary as the source of truth. If required copy is missing or a term is unresolved, report an upstream gap through the owning skill rather than silently inventing content.

---

## 6. Protect the product's identity from generic defaults

Common patterns are tools, not recipes. A pattern is appropriate when it fits the product, content, audience, and task—not merely because it is familiar.

During concept review, challenge choices that resemble interchangeable generated interfaces, including:

- A hero built from a large headline, decorative gradient, and generic call-to-action without a product-specific reason.
- Repeated rounded cards used as the default container for unrelated content.
- The same shadow, radius, and surface treatment applied to every component.
- Cream-and-serif, near-black-and-neon, or editorial-grid combinations selected without a connection to the brief.
- Repetitive uppercase eyebrows, arbitrary metadata strings, decorative numbering, or labels above every heading.
- Monospace typography used as decoration rather than for code or aligned data.
- Gradients, blobs, floating shapes, and icon clusters that do not explain the product.
- Generic stock imagery that could represent any industry.
- Motion applied uniformly rather than in response to a meaningful event.

This is a diagnostic list, not a ban. Any of these choices may be right for a particular product. The requirement is to explain the fit rather than inherit the pattern by default.

### Differentiation review

Before a visual direction is treated as ready, answer:

1. Could this direction plausibly belong to many unrelated products?
2. Which decisions come directly from this product's subject, audience, or content?
3. Is the defining visual characteristic meaningful and useful?
4. Does the hero or primary screen show the product's actual purpose?
5. Are hierarchy and composition driven by user tasks?
6. Are any patterns present only because they are familiar?
7. What can be removed without reducing clarity or identity?

Revise weak choices and record the material changes and reasons. Spend visual emphasis selectively: one memorable characteristic is usually stronger than many competing effects.

---

## 7. Translate direction without losing intent

The three frontend stages must preserve the same design rationale while working at different levels of detail.

### `frontend:uiux` — establish the experience

Owns the UX and language vision.

- Ground the direction in the brief, audience, and user jobs.
- Establish the design hypothesis and compare meaningful layout options when needed.
- Define product-specific principles, visual direction, draft tokens, interaction patterns, responsive behavior, accessibility targets, voice, terminology, and real key-screen copy.
- Record assumptions, open questions, and decisions.
- Keep the vision's status accurate; a draft is not an approved direction.

### `frontend:spec` — make it implementable

Owns the design-system specification and feature UI specifications.

- Translate draft tokens into a coherent primitive and semantic token system.
- Specify typography, layout, themes, components, variants, states, motion, and accessibility behavior.
- Map screens and interactions to the upstream features and contracts.
- Preserve approved copy keys and terminology.
- Identify missing or contradictory upstream decisions instead of inventing requirements.

### `frontend:build` — implement and verify

Owns implementation of the approved frontend specification.

- Consume the specified tokens and component contracts.
- Do not introduce arbitrary visual values or unapproved patterns.
- Implement all applicable interaction and screen states.
- Preserve copy keys, semantics, responsive behavior, and accessibility requirements.
- Inspect rendered output at representative viewports and themes.
- Compare the result against the vision and specification, not personal taste alone.
- Report genuine upstream gaps rather than silently changing the source specification.

These boundaries complement, and do not replace, the detailed ownership and execution rules in each skill.

---

## 8. Review the rendered result

Specifications and code are hypotheses until the experience is inspected.

When the environment supports visual review:

1. Run the application using the project's prescribed environment.
2. Capture representative screens at narrow and wide viewports, and in each required theme.
3. Inspect the screenshots rather than relying only on generated markup or successful builds.
4. Compare the result with the accepted vision and design-system specification.
5. Check hierarchy, alignment, spacing, typography, content density, media cropping, and responsive transitions.
6. Exercise keyboard navigation, visible focus, interactive states, and reduced-motion behavior.
7. Correct discrepancies and inspect again.

Review the full composition as well as individual components. A set of polished components can still produce an incoherent page.

When screenshots or browser inspection are unavailable, state that visual verification was not performed. Do not claim that an interface has been visually reviewed when it has not.

### CSS and component implementation

Keep styling predictable and aligned with the design system.

- Prefer semantic tokens and explicit component variants.
- Keep selector specificity deliberate.
- Avoid broad element selectors that unexpectedly override component styles.
- Avoid contradictory utility classes and duplicated declarations.
- Keep responsive rules close to the component or layout responsibility they affect.
- Do not use visual hacks to conceal structural problems.

---

## 9. Shared quality gate

Before handing work to the next stage, review the applicable items below. A stage should report items outside its ownership as gaps, not quietly take them over.

### Product fit
- [ ] The direction reflects the actual product, audience, and primary tasks.
- [ ] Important requirements and approved decisions are preserved.
- [ ] Assumptions and unresolved questions are visible.

### Visual coherence
- [ ] The composition has a clear and intentional hierarchy.
- [ ] Palette, typography, layout, shape, and imagery support one design premise.
- [ ] The defining characteristic is product-specific, not decorative.
- [ ] Repeated patterns and visual effects have a reason to exist.

### Interaction and content
- [ ] Primary actions and navigation are understandable.
- [ ] Applicable loading, empty, success, error, and disabled states are addressed.
- [ ] Copy is clear, consistent, audience-appropriate, and aligned with the glossary.
- [ ] Action labels match their confirmation and result messages.

### Accessibility and responsive behavior
- [ ] Keyboard paths and visible focus are accounted for.
- [ ] Contrast, semantics, labels, and status communication meet the project target.
- [ ] Reduced motion is respected.
- [ ] Content reflows and remains usable at supported widths.

### Handoff and evidence
- [ ] The artifact is in the correct location and owned by the correct stage.
- [ ] Decisions, assumptions, and upstream gaps are recorded.
- [ ] The next stage has enough information to proceed without guessing.
- [ ] Verification claims accurately describe what was actually checked.

---

## Operating principle

Create a coherent experience from the product's purpose outward.

Understand the user before arranging the interface. Establish a design rationale before choosing visual treatments. Specify behavior before implementing components. Use language to make the product understandable. Inspect the result before claiming it is complete.

The standard is not novelty for its own sake, nor conformity to a fashionable style. It is a distinctive, usable, accessible, and technically realizable experience whose every important decision has a reason.
