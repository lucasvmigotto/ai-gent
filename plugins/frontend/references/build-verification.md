# Rendered review and CSS practice

**Type:** Shared frontend reference, read with `visual-direction.md` (§8).
**Read it when:** `frontend:build` reaches a phase checkpoint or writes styles. `frontend:uiux` and `frontend:spec` do not need it.

Specifications and code are hypotheses until the experience is inspected.

## Rendered review

When the environment supports it:

1. Run the app in the project's prescribed environment.
2. Capture representative screens at narrow and wide viewports, in every required theme, and look at them; a passing build is not evidence.
3. Compare with the accepted vision and design-system spec: hierarchy, alignment, spacing, typography, density, media cropping, responsive transitions.
4. Exercise keyboard navigation, visible focus, interactive states and reduced motion.
5. Fix discrepancies and inspect again.

Review the whole page as well as components: polished parts can still make an incoherent page. If screenshots or browser inspection are unavailable, say visual verification was not performed, and never claim it was.

## CSS and components

- Prefer semantic tokens and explicit component variants.
- Keep specificity deliberate; avoid broad element selectors that override component styles.
- Avoid contradictory utilities and duplicated declarations.
- Keep responsive rules next to the component or layout they affect.
- Never hide a structural problem with a visual hack.
