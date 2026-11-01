---
name: docs
description: Build a static, accessible, internationalized documentation site in docs/site/ — truth-first from the code, enriched by the brief, domain model and specs (Planned until built) — plus llms.txt and Markdown pages for LLMs, with tests and R2 or Docker shipping. Use for "document this project", "create a docs site", "add a documentation website".
---

# Project documentation websites

Builds an official, static, accessible, internationalized documentation
website for a software project — a product that evolves with the project,
not a one-off page. All versioning follows `git:workflow`, one branch per
phase.

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

In a project repo the site lives in `docs/site/` — its own `package.json`,
lockfile and build. `docs/` is the umbrella for all documentation:
`docs/product/` (pipeline artifacts) and any existing ADRs/runbooks stay
where they are and become content sources.

**When the repository *is* the site** — a documentation hub, landing or
portfolio whose whole purpose is the site, with no host application to
document — put it at the root instead, in `site/`. There is no
"documentation of something else" to sit beside, so nesting it under
`docs/site/` would only be `docs/`-of-`docs/`. Every other repo stays on
`docs/site/`.

If a site already exists elsewhere (`docs/` root, `website/`), ask before
moving it.

**An existing site is audited, not only extended.** Check it against the
Phase 2 and Phase 7 rules (the version footer, source links at the build's
tag instead of `main`, the deploy after the release) and bring what falls
short up to them as listed changes, even when the request was only new
content.

## Phase 1 — Discovery (no implementation yet)

Audit the host project: structure, manifest (`Cargo.toml`/`package.json`),
CLI surface, config shape, auth model, API integration, features, tests,
CI/CD, README/LICENSE, version, platforms (the README itself follows
`../../references/readme.md`; add the docs site to its *Links*). Produce a truth table with
`file:line` references plus the no-invent list. Inspect the reference
projects for reusable patterns (visual family, router strategy, test
harness, locale architecture, R2 deploy + cache strategy).

Then read the pipeline artifacts (table above). For each spec feature,
check the code: the truth table gets one row per feature and user story
with its real status. Planned material goes on clearly marked roadmap
pages, never mixed into how-to or reference pages as if it worked today.
Use the brief's glossary for every term on the site.

If the project ships a CLI or TUI, the site gets a command reference
generated from the real `--help` output (never hand-written) and demos
recorded from `vhs` tapes, re-rendered in CI so they can't go stale.

## Phase 2 — Scaffold

Bun · React · TypeScript (current stable major, pinned) · Vite · Tailwind CSS ·
React Router **`HashRouter`** (static hosts have no SPA fallback) · Biome (not
ESLint). No runtime backend, no heavy doc framework. Versions are injected at
build time from the host's sources of truth; the footer reads
`<project>@v<version> · docs@v<docs version>`. Branding derives from
`docs/product/ux-vision.md` when it exists. Read `references/scaffold.md` before
scaffolding: footer rules, metadata, visual identity.

## Phase 3 — Components (only what pages need)

The set (`Layout`, `Header`, `Nav`, `Footer`, `Card`, `Callout`, `CodeBlock`,
`Search`, `LanguageSwitcher`, `VersionBadge`, …) and the rules are in
`references/components.md`; read it when building them. The floor:
semantic HTML before ARIA, full keyboard operability with visible focus,
`prefers-reduced-motion` honored, status never by color alone, a maturity
badge on every page. **The footer carries the site's metadata; the header
carries navigation** — versions, the license linked to `LICENSE` at the
build's ref, and an icon theme switch; no "Source on GitHub" link.

## Phase 4 — i18n (namespaced locale files)

The site's **primary** locale is en-US, unless the product's own
interface is confirmed primarily non-English — then the primary locale
matches it instead, content written entirely in that language
(`../../references/language-policy.md`). Other locales are still additive on
top of whichever one is primary.

Separate **UI chrome** from **documentation content** in typed locale
files (e.g. `ui` + `docs` namespaces). A shared content interface makes
a missing translation a type error, never a silent gap. Browser-language
detection → locale persistence → English fallback; `<html lang>` synced.
Technical identifiers (commands, flags, env vars, code, URLs) stay
untranslated. New locales must not require component rewrites.
## Phase 4b — LLM-readable output → `references/llm-output.md`

A single-page app is close to invisible to LLMs: crawlers and agent fetch
tools don't run JavaScript. The build also emits plain Markdown from the
**same typed content** the pages render, never a second copy: `llms.txt`
(llmstxt.org), `llms-full.txt`, and one `.md` per page and locale,
advertised with `<link rel="alternate" type="text/markdown">`. Maturity
labels appear in the text itself (`**Planned — not implemented yet.**`).
The file has the structure, the code-block and table rules, and the
`AGENTS.md` offer.

## Phase 5 — Tests → `references/tests.md`

Unit and axe a11y suites (vitest), e2e smoke through containerized
browsers (never a host driver), a unit test per row of the footer's
version table, and LLM-output tests (every page has its `.md` in every
locale, every `llms.txt` link resolves, no Planned item without its
label). The file has the details.

## Phase 6 — Validation

`bun install --frozen-lockfile && bun run lint && bun run typecheck
&& bun run test && bun run build`, then inspect `dist/`: the footer
shows every version (`<project>@v… · docs@v…`), and no source link
points at `blob/main`. Follow with a
manual keyboard-only / screen-reader pass and a genuine second
engineering pass (dedup, links, mobile behavior, deps, versions, cache,
error states) before calling anything done.

## Phase 7 — Ship

Every CI step and the Docker context use the site directory. Unless the project
uses another platform, write `.github/workflows/<site>.yml` (`verify` then
`deploy`, publishing on the default branch only, with a declared `environment:`)
and deploy to R2 with `devsecops:pipeline`'s secret names; Docker is optional.
Read `references/ship.md` before writing the workflow or Dockerfile. End with
the final report: architecture, reference deltas, accessibility, i18n,
deployment, validation results, remaining work.
