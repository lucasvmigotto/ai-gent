Reconstructed by project:introspec on 2026-10-18 from 384c771

# Feature Specification: Docs site build, checks and R2 deploy

**Feature Branch**: `docs/introspec-site-runners` (reconstructed; the code already exists on `main`)

**Created**: 2026-10-18

**Status**: Draft

**Input**: Reverse-engineered from `.github/workflows/docs-ci.yml`, `docs/site/tests/e2e/smoke.mjs` and `docs/site/scripts/og-image.ts`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Every docs change is verified (Priority: P1)

On a push to `main` or `dev` or a pull request touching `docs/site/**` or `docs/product/**`, CI installs with the frozen lockfile, lints, typechecks, unit-tests and builds the site under the repository-name prefix, and asserts the build.

**Why this priority**: the only gate between a change and the public site.

**Independent Test**: the `verify` job of a Docs CI run.

**Acceptance Scenarios**:

1. **Given** a change under the trigger paths, **When** Docs CI runs, **Then** `bun install --frozen-lockfile`, `lint`, `typecheck`, `test` and `build` run in order [OBSERVED: .github/workflows/docs-ci.yml:3-66].
2. **Given** a build, **When** it finishes, **Then** CI fails unless `index.html` loads assets from `/<repo>/assets/` [OBSERVED: docs-ci.yml:67-76].
3. **Given** a build, **When** it finishes, **Then** CI fails unless `dist/og-image.png` exists and `og:image` and `twitter:image` are advertised [OBSERVED: docs-ci.yml:78-84].
4. **Given** the workflow file, **When** CI runs, **Then** any `aws s3 sync|cp` line not using `$R2_PREFIX` fails the build [OBSERVED: docs-ci.yml:86-95].

---

### User Story 2 - The built site works in a real browser (Priority: P1)

CI serves the built site and drives it with containerized Chrome (selenium/standalone-chrome, pinned by digest): landing, flow page, a skill page and the not-found route.

**Independent Test**: `bun run build`, `vite preview`, `SELENIUM_URL=… bun run e2e`.

**Acceptance Scenarios**:

1. **Given** the e2e job, **When** it runs, **Then** Chrome is started by digest with `--network host` and the smoke asserts four pages [OBSERVED: docs-ci.yml:105-170; docs/site/tests/e2e/smoke.mjs:19-61].
2. **Given** the last Docs CI runs on `main` and `dev` (2026-10-17), **When** checked, **Then** they succeeded [OBSERVED: `gh run list --workflow docs-ci.yml` → run 39067000179 success].

---

### User Story 3 - Publish after a release (Priority: P1)

After `release.yml` completes successfully on `main`, the deploy job rebuilds `main` (so the footer shows the new tag) and syncs it to Cloudflare R2 under `/<repo>/`, with immutable caching for hashed assets and `no-cache` for entry, LLM and Markdown files.

**Acceptance Scenarios**:

1. **Given** a successful `release` workflow_run, **When** deploy runs, **Then** it builds with `VITE_BASE_PATH=/<repo>/` in environment `ai-gent-docs` [OBSERVED: docs-ci.yml:172-207].
2. **Given** R2 credentials are absent, **When** deploy runs, **Then** it warns and skips every sync step, and the job succeeds [OBSERVED: docs-ci.yml:210-218].
3. **Given** credentials, **When** deploy runs, **Then** it validates the endpoint (`https://`), bucket and prefix shape (`/<slug>/`) before syncing [OBSERVED: docs-ci.yml:220-226].
4. **Given** the sync, **When** it runs, **Then** `assets/` is synced with `--delete` and `max-age=31536000, immutable`, `index.html` and `llms*.txt` with `no-cache`, and `docs/` as `text/markdown` with `--delete`, all under `$R2_PREFIX` [OBSERVED: docs-ci.yml:231-263].

---

### User Story 4 - Regenerate the preview image (Priority: P3)

A maintainer runs `bun run og` to rebuild the committed `public/og-image.png` (1200×630) from `scripts/og-image.svg` with sharp.

**Acceptance Scenarios**:

1. **Given** `scripts/og-image.svg`, **When** `bun run og` runs, **Then** `public/og-image.png` is rewritten [OBSERVED: docs/site/scripts/og-image.ts; docs/site/package.json `og`].

### Edge Cases

- The deploy job does not wait for the `e2e` job; e2e is advisory [OBSERVED: docs-ci.yml:105-108,172-176].
- Hashed assets are deleted before the new `index.html` is uploaded; a reader holding the old `index.html` during the sync can get 404s for old chunks [INFERRED: sync order docs-ci.yml:231-248 → a short broken window per deploy; confirm by loading the site during a deploy, or upload assets without `--delete` first and prune after].
- Third-party actions in this workflow are referenced by tag (`actions/checkout@v4`, `oven-sh/setup-bun@v2`, `actions/upload-artifact@v4`) and Bun by major (`"1"`), while `check.yml` and `release.yml` pin by SHA [OBSERVED: docs-ci.yml:36,40,98,112,114,195,199 vs .github/workflows/check.yml:22].
- A missing-credentials deploy is a green run that published nothing [OBSERVED: docs-ci.yml:216].

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Every R2 write MUST be scoped to `$R2_PREFIX`; a root-scoped sync MUST fail CI.
- **FR-002**: The prefix MUST derive from the repository name, never from configuration.
- **FR-003**: Deploy MUST run only after a successful release on `main`.
- **FR-004**: CI MUST fail when the base path or the Open Graph image drifts.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Docs CI green on `main` and `dev` [OBSERVED: run 39067000179, 2026-10-17].
- **SC-002**: Zero writes outside `/ai-gent/` in the bucket.

## Assumptions

- [ASSUMPTION: the R2 credentials are configured in the `ai-gent-docs` environment; the deploy run's sync output wasn't read.]
