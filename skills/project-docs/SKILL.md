---
name: project-docs
description: Build a production-quality static documentation website from/for a software project. Use when the user asks to document a project, create a docs site, add a docs/ website, or says "project-docs", "docs site", or similar. Covers phased delivery (discovery, scaffold, components, i18n, tests, deploy), the Bun/React/TS7/Vite/Tailwind/HashRouter/Biome stack, and R2 + Docker shipping.
---

# Project documentation websites

Builds an official, static, accessible, internationalized documentation
website for a software project — a product that evolves with the project,
not a one-off page. Works through `git-workflow` for all versioning
(branch-per-phase, small Conventional Commits, ask-before-commit,
`--no-ff` at 4+ commits, never push without an explicit order).

## Non-negotiable rules

- **Truth first.** Every behavior claim derives from the host repo's code.
  Classify each as Implemented / Partially Implemented / Planned /
  Unavailable / Upstream Limitation / Not Applicable. Never document
  planned functionality as implemented. Keep a running no-invent list
  (commands, flags, env vars, endpoints, error strings) and check every
  page against it.
- **No secrets.** Never real credentials, tokens, or infrastructure values
  in examples, code, build output, or logs. Secrets appear only as
  `Secret`/variable placeholders.
- **Reference, don't clone.** Sibling projects are pattern sources, not
  copy targets: adopt architecture/visual/UX/a11y/deployment patterns,
  fix their weaknesses instead of reproducing them.
- **Accessibility and i18n are day-one architecture**, not later passes.
- **Never weaken CI or security for convenience.** Lint/typecheck/build/
  test failures get fixed, never suppressed.

## Phase 0 — Placement

Default directory is `docs/`. If `docs/` already holds non-site content
(specs, ADRs, runbooks), do not clobber it — fall back to `website/` and
treat the existing docs as content sources. Ask when ambiguous.

## Phase 1 — Discovery (no implementation yet)

Audit the host project: structure, manifest (`Cargo.toml`/`package.json`),
CLI surface, config shape, auth model, API integration, features, tests,
CI/CD, README/LICENSE, version, platforms. Produce a truth table with
`file:line` references plus the no-invent list. Inspect the reference
projects for reusable patterns (visual family, router strategy, test
harness, locale architecture, R2 deploy + cache strategy).

## Phase 2 — Scaffold

Bun · React · TypeScript 7 (pinned, resolved tree verified with
`bunx tsc --version` + `bun pm ls typescript`) · Vite · Tailwind CSS ·
React Router **`HashRouter`** (static hosts provide no SPA fallback, so
hash routing keeps deep links working) · Biome (not ESLint). No runtime
backend, no heavy doc framework.

- Version injected at build time from the host's source of truth
  (e.g. read `Cargo.toml` in `vite.config.ts`), never hand-maintained.
- Page metadata + Open Graph + favicon + `index.html`; distinct project
  branding (dark-first tokens adapted per project, restrained motifs).

## Phase 3 — Components (only what pages need)

`Layout`, `Header`, `Nav`, `MobileNav`, `Footer`, `SkipLink`, `Section`,
`Card`, `FeatureCard`, `Badge`/`StatusBadge`, `Callout`, `CodeBlock` +
`CodeCopyButton`, `Table`, `Accordion`, `Breadcrumbs`, `Search`,
`LanguageSwitcher`, `VersionBadge`.

- Semantic HTML before ARIA; full keyboard operability with visible
  focus; focus restore on mobile-nav/search close; `prefers-reduced-motion`
  honored; status never signaled through color alone.
- Every page carries maturity badges so readers always know what is real.

## Phase 4 — i18n (namespaced locale files)

Separate **UI chrome** from **documentation content** in typed locale
files (e.g. `ui` + `docs` namespaces). A shared content interface makes
a missing translation a type error, never a silent gap. Browser-language
detection → locale persistence → English fallback; `<html lang>` synced.
Technical identifiers (commands, flags, env vars, code, URLs) stay
untranslated. New locales must not require component rewrites.

## Phase 5 — Tests

- **Unit + a11y (vitest):** Testing Library suites for components,
  navigation, and i18n detection/persistence/fallback in
  `tests/unit/`; automated axe checks (`toHaveNoViolations`) in
  `tests/a11y/`. Scripts: `test` (`vitest run`), `test:watch`.
- **e2e (Playwright):** serve the production static build, smoke every
  route including deep links, keyboard-only flows, language switching,
  and a mobile project. Chromium-first in CI, full matrix locally.

## Phase 6 — Validation

`bun install --frozen-lockfile && bun run lint && bun run typecheck
&& bun run test && bun run build`, then inspect `dist/`. Follow with a
manual keyboard-only / screen-reader pass and a genuine second
engineering pass (dedup, links, mobile behavior, deps, versions, cache,
error states) before calling anything done.

## Phase 7 — Ship

- **R2 workflow:** checkout → setup Bun (pinned) → install → lint →
  typecheck → test → build → artifact → S3-sync to Cloudflare R2.
  Immutable long-lived caching for hashed assets, `no-cache` for entry
  HTML. Concurrency cancel-in-progress, least-privilege permissions,
  endpoint validation. Credentials via Variables/Secrets placeholders.
- **Docker (optional):** Bun builder → nginx runtime, non-root user,
  security headers, gzip. Repeat security headers inside every
  `location` block — location-level `add_header` disables server-level
  inheritance. Hash routing needs no SPA fallback (`try_files
  $uri $uri/ =404`).
- **Final report:** architecture, reference deltas (reused / adapted /
  intentionally changed + why), accessibility mechanisms, i18n design,
  deployment, validation results, remaining work, future work.
