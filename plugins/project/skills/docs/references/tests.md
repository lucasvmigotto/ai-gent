# Tests (Phase 5)

Part of `project:docs` (`../SKILL.md`): read it when writing the unit, accessibility, e2e, version and LLM-output tests.

- **Unit + a11y (vitest):** Testing Library suites for components,
  navigation, and i18n detection/persistence/fallback in
  `tests/unit/`; automated axe checks (`toHaveNoViolations`) in
  `tests/a11y/`. Scripts: `test` (`vitest run`), `test:watch`.
- **e2e (Playwright or Selenium):** serve the production static build, smoke every
  route including deep links, keyboard-only flows, language switching,
  and a mobile project — through containerized browsers
  (`selenium/standalone-*`; WebKit only via Playwright's container, per
  the qa plugin's references/browsers.md rules, never a host driver install).
  Chromium-first in CI, full matrix locally.
- **Versions:** a unit test of the footer's version resolution, one case
  per row of `version-footer.md`'s table (on the tag, past it,
  no tag, no git), and a render test that the footer shows every entry
  linked to its tree.
- **LLM output:** every page has its `.md` in every locale, every
  `llms.txt` link resolves in `dist/`, `llms-full.txt` contains every
  page, and every non-Implemented item carries its maturity label in the
  Markdown (a test fails on a Planned feature without one).
