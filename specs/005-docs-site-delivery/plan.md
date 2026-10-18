Reconstructed by project:introspec on 2026-10-18 from 384c771

# Implementation Plan: Docs site build, checks and R2 deploy

**Branch**: `docs/introspec-site-runners` | **Date**: 2026-10-18 | **Spec**: `spec.md`

## Summary

One GitHub Actions workflow with three jobs: `verify` (push/PR), `e2e` (after verify), `deploy` (on `workflow_run` of `release`). The site is built three times per release cycle (verify, e2e, deploy) [OBSERVED: .github/workflows/docs-ci.yml].

## Technical Context

**Platform**: GitHub Actions, `ubuntu-24.04`, Bun 1.x, AWS CLI against the R2 S3 endpoint.

**Configuration (names only)**: variables `CLOUDFLARE_R2_ACCOUNT_ID`, `CLOUDFLARE_R2_ENDPOINT_S3_CLIENT`, `CLOUDFLARE_R2_BUCKET_ID`; secret `CLOUDFLARE_R2_ACCOUNT_SECRET`; environment `ai-gent-docs` [OBSERVED: docs-ci.yml:178-193].

**Browser**: `selenium/standalone-chrome:153.0@sha256:7efe71e7…` [OBSERVED: docs-ci.yml:135].

**Testing**: the workflow is its own test; no `act` or local dry run exists.

## Constitution Check

N/A — no constitution. Checked against `CLAUDE.md`: "never sync at the bucket root" ✓ (fitness step), e2e "never a host driver" ✓.

## Project Structure

```text
.github/workflows/docs-ci.yml
docs/site/tests/e2e/smoke.mjs
docs/site/scripts/og-image.ts, og-image.svg → docs/site/public/og-image.png
```
