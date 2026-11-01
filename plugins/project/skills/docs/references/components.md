# Components and the footer (Phase 3)

Part of `project:docs` (`../SKILL.md`): read it when building the component set, the accessibility floor, or the footer and header.

`Layout`, `Header`, `Nav`, `MobileNav`, `Footer`, `SkipLink`, `Section`,
`Card`, `FeatureCard`, `Badge`/`StatusBadge`, `Callout`, `CodeBlock` +
`CodeCopyButton`, `Table`, `Accordion`, `Breadcrumbs`, `Search`,
`LanguageSwitcher`, `VersionBadge`.

- Semantic HTML before ARIA; full keyboard operability with visible
  focus; focus restore on mobile-nav/search close; `prefers-reduced-motion`
  honored; status never signaled through color alone.
- Every page carries maturity badges so readers always know what is real.
- **The footer carries the site's metadata; the header carries
  navigation.** Footer, one centered row that wraps on narrow screens: the
  versions (`version-footer.md`), the license as its short name
  with a mark (`GPL v3+`, `MIT`) linked to `LICENSE` at the build's ref
  with the full name as its title — or, when no `LICENSE` file exists,
  plain text reading "All rights reserved," unlinked; never assume an
  open license the project doesn't have — and the theme switch, which is
  icon buttons (sun, moon) in a `fieldset` with a hidden `legend`, each
  named (`aria-label` + `title`) with `aria-pressed` on the current one
  and the choice persisted. No "Source on GitHub" link or counts: the
  version links already open the repository.
