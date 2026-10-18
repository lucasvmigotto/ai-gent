Reconstructed by project:introspec on 2026-10-18 from 384c771

# Implementation Plan: LLM-readable output

**Branch**: `docs/introspec-site-runners` | **Date**: 2026-10-18 | **Spec**: `spec.md`

## Summary

A Bun script run after `vite build` imports `generated.json` and the `Messages` copy, builds a list of pages (static pages hand-assembled from copy keys, plus one per skill) and writes them under `dist/` [OBSERVED: docs/site/scripts/generate-llms.ts:1-234; docs/site/package.json `build`].

## Technical Context

**Language**: TypeScript run by Bun. **Inputs**: `src/content/generated.json`, `src/i18n.tsx` `messages`. **Outputs**:

```text
dist/llms.txt, dist/llms-full.txt                  en-US
dist/llms-<locale>.txt, dist/llms-full-<locale>.txt
dist/docs/<locale>/{install,flow,contribute,reference,changelog}.md
dist/docs/<locale>/skills/<plugin>-<skill>.md
```

**Testing**: none (see spec Edge Cases).

**Design note** [INFERRED: the static pages' Markdown is a second hand-written rendering of the same copy, `generate-llms.ts:51-164` → it drifts whenever a page gains a section; confirm by diffing sections, or render both from one page description].

## Constitution Check

N/A — no constitution (see feature 001).
