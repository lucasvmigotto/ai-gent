Reconstructed by project:introspec on 2026-10-18 from 384c771

# Implementation Plan: Docs site shell and generated content

**Branch**: `docs/introspec-site-runners` | **Date**: 2026-10-18 | **Spec**: `spec.md`

**Input**: As-is code in `docs/site/` (no intent reconstructed).

## Summary

A static React single-page app, hash-routed, built by Vite under a path prefix. A Node script reads the repository at build time into one JSON file that every page imports; there is no runtime data fetch.

## Technical Context

**Language/Version**: TypeScript 7.0.2, React 19.3.0, Node-compatible scripts run by Bun 1.x [OBSERVED: docs/site/package.json; .github/workflows/docs-ci.yml:40-42]

**Primary Dependencies**: react-router 8.4.0 (HashRouter), react-markdown 10.1.0 + remark-gfm 4.0.1 (lazy-loaded), lucide-react 1.50.0, Tailwind CSS 4.3.3 via `@tailwindcss/vite`, Vite 8.3.2 [OBSERVED: docs/site/package.json]

**Storage**: none at runtime; `localStorage` keys `ai-gent-docs-theme`, `ai-gent-docs-locale` [OBSERVED: docs/site/src/main.tsx:6; docs/site/src/i18n.tsx:403]

**Testing**: Vitest 5 + Testing Library + happy-dom + jest-axe (unit, component, a11y); Selenium smoke in containerized Chrome (feature 005) [OBSERVED: docs/site/package.json; docs/site/tests/]

**Target Platform**: modern browsers; static hosting on Cloudflare R2 under `/ai-gent/`

**Project Type**: static web app (documentation site)

**Performance Goals**: [ASSUMPTION: the brief's "landing under 200 KB transferred" target was never measured; no bundle-size check exists]

**Constraints**: no backend; content fixed at build time; English only

**Scale/Scope**: 12 plugins, 44 plugin skills at 384c771 [OBSERVED: `ls plugins/*/skills/*/SKILL.md | wc -l`]

## Constitution Check

N/A — no `.specify/memory/constitution.md` exists in this repository (see `docs/product/introspec.md`, open items). Checked instead against `CLAUDE.md` "Docs site" rules: generated content not hand-edited ✓, `LOCALES` single source with a completeness test ✓, deploy prefix-scoped ✓ (feature 005).

## Project Structure

```text
docs/site/
├── index.html                 metadata, %SITE_URL% / %VITE_BASE_PATH% placeholders
├── vite.config.ts             base path, html env plugin
├── scripts/
│   ├── generate-content.mjs   repository → src/content/generated.json
│   ├── version.mjs            gitInfo(), footerVersions(), label()
│   ├── generate-llms.ts       feature 004
│   └── og-image.ts/.svg       feature 005
├── src/
│   ├── main.tsx               theme bootstrap, root render
│   ├── App.tsx                routes
│   ├── components.tsx         Layout, TopBar, ThemeSwitch, CodeBlock, SearchBox, SkillIndex
│   ├── content.ts             typed access to generated.json, search, triggers
│   ├── i18n.tsx               LOCALES, Messages, provider
│   ├── Markdown.tsx           lazy react-markdown
│   └── pages.tsx              features 002 and 003
└── tests/                     vitest suites + e2e/smoke.mjs
```

**Structure Decision**: as found; single static app.

## Complexity Tracking

None recorded.
