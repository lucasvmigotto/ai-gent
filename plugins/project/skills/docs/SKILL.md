---
name: docs
description: Build a comprehensive, production-quality static documentation website for a software project in docs/site/ — truth-first from the code, enriched by the product pipeline's brief, UX vision, domain model and Spec Kit specs (always labelled Planned until built), branded from the UX vision or via frontend:uiux. Use when the user asks to document a project, create a docs site, add a documentation website, or runs /project:docs. Covers phased delivery (discovery, scaffold, components, i18n, tests, deploy), the Bun/React/TS7/Vite/Tailwind/HashRouter/Biome stack, and R2 + Docker shipping.
---

# Project documentation websites

Builds an official, static, accessible, internationalized documentation
website for a software project — a product that evolves with the project,
not a one-off page. Works through `git-workflow` for all versioning
(branch-per-phase, small Conventional Commits, ask-before-commit,
`--no-ff` at 4+ commits, never push without an explicit order).

Part of the product pipeline — read `../../references/pipeline.md` first.
Its artifacts are content sources, not substitutes for reading the code:

| Source | Used for | Maturity label |
|---|---|---|
| the code, tests, CI, manifests | every behavior claim | as verified |
| `docs/product/brief.md` | overview, audiences, concepts, **glossary** (the site's terminology) | Planned unless the code implements it |
| `docs/product/domain-model.md` | concept and lifecycle pages | Planned unless implemented |
| `docs/product/ux-vision.md` | the site's voice, and the product's visual identity | — |
| `docs/product/architecture.md`, `docs/product/adr/` | architecture overview and decision pages | Planned until the deployed system matches it |
| `specs/README.md`, `specs/NNN-*/` | roadmap and "what's coming" pages | **Planned**, always, until a build stage marks it Implemented *and* the code confirms it; show **Verified** when QA has passed |
| `contracts/openapi.yaml` | API reference | Implemented only for operations the backend actually serves (check routes/contract tests) |

## Non-negotiable rules

- **Truth first.** Every behavior claim derives from the host repo's code.
  Classify each as Implemented / Partially Implemented / Planned /
  Unavailable / Upstream Limitation / Not Applicable. Never document
  planned functionality as implemented — a Spec Kit spec, a brief or a
  UX vision describes intent, not behavior. Keep a running no-invent list
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

The site always lives in `docs/site/` — its own `package.json`, lockfile
and build. `docs/` is the umbrella for all documentation: `docs/product/`
(pipeline artifacts) and any existing ADRs/runbooks stay where they are
and become content sources. If a site already exists elsewhere (`docs/`
root, `website/`), ask before moving it.

## Phase 1 — Discovery (no implementation yet)

Audit the host project: structure, manifest (`Cargo.toml`/`package.json`),
CLI surface, config shape, auth model, API integration, features, tests,
CI/CD, README/LICENSE, version, platforms. Produce a truth table with
`file:line` references plus the no-invent list. Inspect the reference
projects for reusable patterns (visual family, router strategy, test
harness, locale architecture, R2 deploy + cache strategy).

Then read the pipeline artifacts (table above). For each spec feature,
check the code: the truth table gets one row per feature and user story
with its real status. Planned material goes on clearly marked roadmap
pages, never mixed into how-to or reference pages as if it worked today.
Use the brief's glossary for every term on the site.

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
- **Visual identity.** If `docs/product/ux-vision.md` exists, derive the
  site's tokens from it (and from `specs/000-design-system/` if built),
  so the docs look like the product's family. If not, invoke
  `frontend:uiux` scoped to the docs site (audience: the site's readers)
  for a lightweight direction — its visual-direction method avoids the
  templated docs look. Hold the site to `frontend:build`'s quality floor.

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

- **Paths:** every CI step, cache key and the Docker build context use
  `docs/site/` (`working-directory: docs/site`, `paths: [docs/site/**]`
  triggers). Content read from `docs/product/` or `specs/` must also
  trigger a rebuild.
- **Pipeline:** the site's workflow is designed by `devsecops:pipeline`
  (GitHub Actions unless the project uses another platform) with its
  supply-chain baseline; the steps below are what it must contain.
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
