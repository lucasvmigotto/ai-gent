Reconstructed by project:introspec on 2026-10-18 from 384c771

# Implementation Plan: Skill finder and skill reference

**Branch**: `docs/introspec-site-runners` | **Date**: 2026-10-18 | **Spec**: `spec.md`

## Summary

Pages read the bundled content (`../001-docs-site-shell/data-model.md`) synchronously. Search is an in-memory filter over 44 entries, re-run on every keystroke; there is no search index to load, so the UX vision's loading skeleton and "index fetch failed" state don't apply [OBSERVED: docs/site/src/components.tsx:292].

## Technical Context

Same stack as feature 001. Code: `docs/site/src/pages.tsx:50-204, 616-628`, `docs/site/src/components.tsx:265-369`, `docs/site/src/content.ts:45-90`. Markdown bodies render through a lazy `react-markdown` chunk with a `…` fallback [OBSERVED: components.tsx:24-32].

**Testing**: unit (`tests/content.test.ts`), component and axe (`tests/pages.test.tsx`), e2e smoke for the skill page and not-found (`tests/e2e/smoke.mjs:38-56`), run in CI (`.github/workflows/docs-ci.yml` job `e2e`).

## Constitution Check

N/A — no constitution (see feature 001).

## Project Structure

As feature 001; no files of its own beyond the page components.
