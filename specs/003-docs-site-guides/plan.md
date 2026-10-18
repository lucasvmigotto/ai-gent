Reconstructed by project:introspec on 2026-10-18 from 384c771

# Implementation Plan: Install, flow, contribute, reference and changelog pages

**Branch**: `docs/introspec-site-runners` | **Date**: 2026-10-18 | **Spec**: `spec.md`

## Summary

Static pages composed from `Messages` copy (`docs/site/src/i18n.tsx`) and the generated content (plugins, changelog). The flow page's steps are data in the copy, rendered twice: a one-line "glance" of icons and a numbered list [OBSERVED: docs/site/src/pages.tsx:491-537, 539-601].

## Technical Context

Same stack as feature 001. Icons from `lucide-react`. The installer command strings are duplicated in `pages.tsx:216,222` and `scripts/generate-llms.ts:66,74` [OBSERVED] — two copies to keep in step with `install.sh`.

**Testing**: component and axe tests for the flow page (`tests/pages.test.tsx:46-75`); e2e smoke renders `#/use/flow` (`tests/e2e/smoke.mjs:32-36`). Install, contribute, reference and changelog pages have **no tests**.

## Constitution Check

N/A — no constitution (see feature 001).

## Project Structure

As feature 001.
