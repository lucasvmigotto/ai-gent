# Design tokens and visual system detail

**Type:** Shared frontend reference, read with `visual-direction.md` (§3).
**Read it when:** `frontend:spec` writes the design-system feature, or `frontend:uiux` drafts tokens. `frontend:build` reads it only when a token or value is unclear.

Define roles and relationships before isolated values.

## 3.1 Color

When no approved system exists, create a restrained palette, normally four to seven named colors.

- **Primitive tokens** are raw values (`violet.700`, `amber.300`).
- **Semantic tokens** are purpose-based roles (`surface.canvas`, `text.primary`, `border.subtle`, `action.primary`, `feedback.error`). They are the interface contract: components consume roles, never hex values.

Specify each required theme (light, dark). List foreground/background pairs with their contrast ratios; distinct-looking colors are not proof of accessibility. Use color for hierarchy, category and state, not for "a pop of color".

## 3.2 Typography

Choose typefaces for character, language coverage, readability and technical limits. Specify:

- families and their roles, plus loading and fallback strategy;
- scale, weights, line heights and letter spacing;
- heading, body and functional (label, data, code) behavior;
- line lengths and responsive scaling.

One family can be enough; a second needs a role, not variety. Reserve monospace for commands, code and genuinely tabular data. Avoid emphasis and styles that add no meaning or hierarchy.

## 3.3 Layout and spacing

Reflect the information architecture and density. Specify, as relevant: content and reading widths, grid and column behavior, alignment, spacing scale, section rhythm, breakpoints and reflow, navigation-to-content relationship, and overflow for dense or fixed-width material.

Left-align information-heavy content for scanning. Center only short, focused compositions. Justified text needs deliberate control. Reuse layout rules where they clarify relationships, and vary the composition where content or task requires it.

## 3.4 Shape, borders, elevation and imagery

Define a small vocabulary: corner radii, borders and dividers, shadows and elevation, surface layers, icons and illustration, photography and diagrams.

Each border, card, badge, label or divider must explain grouping, hierarchy, interaction or state. Do not give every element the same radius, shadow or container, and keep decoration from competing with content.

## 3.5 Motion

Motion explains a transition, shows an action's result, or directs attention to a meaningful change. Define motion tokens and where they apply. Prefer interaction-triggered motion to ambient animation; the one-orchestrated-moment rule is in `visual-direction.md` §3. Honor `prefers-reduced-motion`: essential information and function must work without animation.
