# Product brief — ai-gent docs site

Status: Draft

## Summary

A public, bilingual documentation site for **ai-gent** (9 plugins, 36 skills
for Claude Code and opencode) at `https://docs.lucasvmigotto.me/ai-gent/`,
serving both toolkit users and contributors, kept true to the code it
documents and readable by humans and LLM agents alike.

ai-gent is a single-source repository of agent skills and plugins kept in one
place and installed by symlink (`setup.sh`, `install.sh`). It encodes a whole
software-delivery methodology: a product pipeline (brief → architecture →
specs → frontend, backend, QA → docs), container-first development, CI/CD
and supply-chain security, safe database access, and git rules enforced by
hooks. Its audience today reads a 372-line `README.md`, per-skill `SKILL.md`
files, and `AGENTS.md`. The docs site turns that material into a navigable,
searchable, versioned product that grows with the toolkit instead of a
single long page.

## Problem & context

ai-gent's documentation lives where its code lives: `README.md`, 36 skill
files, shared contracts (`shared/pipeline.md`, `shared/containers.md`,
`shared/reading-code.md`, `shared/spec-kit.md`), `CHANGELOG.md`, and
`AGENTS.md`. That works for contributors who already know the layout, but
three groups hurt:

- **New users** face one long README to install the toolkit and find the
  right skill among 36; there is no per-skill reference page to link to.
- **Contributors** must piece together the editing conventions (`AGENTS.md`),
  `scripts/check.sh`, the release flow, and the layout rules from several
  files before a first change.
- **Agents** (Claude Code, opencode, and anything fetching over HTTP) get no
  machine-readable map of the toolkit: crawlers and fetch tools don't run
  JavaScript, so a client-rendered page reference would be invisible to them.

Alternatives in use today: reading the repo directly on GitHub, and asking
the maintainer. Both scale with the maintainer's time, which does not scale
with the toolkit's growth (9 plugins and counting, per-plugin versioning,
automated releases).

Why now: the toolkit just reached 1.7.0 with a stable install story
(OpenCode V2 support, guard plugin), a `compact` plugin, and session tooling
(`project:recap`, `project:survey`). The surface is big enough to need real
docs and stable enough to document truthfully.

## Goals and non-goals

Goals (measurable; see Success metrics):

- Every one of the 36 skills has its own reference page, accurate to the
  code, in English at launch, with the i18n architecture in place so a
  second locale is a content task, not a rewrite.
- A new user can go from landing to installed toolkit and first invoked
  skill without reading the raw repository.
- A contributor can go from landing to a passing `scripts/check.sh` run
  without asking the maintainer.
- An agent fetching over HTTP can map the whole toolkit from `llms.txt`
  without running JavaScript.

Non-goals (things a reasonable person might assume are in scope):

- Not a tutorial course or video content — reference and how-to only.
- Not a changelog browser beyond what `CHANGELOG.md` already is; releases
  stay automated from Conventional Commits.
- Not user accounts, comments, or analytics dashboards.
- Not documentation for downstream products built *with* ai-gent skills;
  the site documents the toolkit itself.

## Audiences

- **Toolkit users** (primary): developers using Claude Code or opencode who
  install ai-gent and invoke its skills. Goals: install once, find the
  right skill fast, trust what it does. Context: their own terminal and
  editor; frequent use while working. Expertise: working developers, new to
  this toolkit.
- **Contributors** (primary): developers changing skills, plugins, scripts,
  or shared contracts. Goals: make a correct first change (metadata,
  references, checks, release flow) with `scripts/check.sh` green. Context:
  the ai-gent repo itself; occasional but deep sessions. Expertise: the
  repo's conventions, which they are here to learn.
- **Agents over HTTP** (secondary): LLM agents and crawlers fetching pages
  without JavaScript. Goals: a complete, current map of the toolkit in
  plain Markdown. Context: `llms.txt` → per-page `.md`. Expertise: N/A —
  they parse text, not UI.
- **The maintainer** (operator): publishes the site on every relevant
  change with no manual step beyond the existing release flow. Context: CI
  and Cloudflare R2.

## Jobs-to-be-done

- When I install ai-gent for Claude Code or opencode, I want a short,
  correct install path for my tool, so I can start using skills today.
- When I have a task ("load-test the API", "model the domain"), I want to
  find the skill that owns it, so I invoke the right one instead of
  improvising.
- When I change a skill or plugin, I want the exact conventions and checks
  that apply, so my change passes CI on the first push.
- When I cut or consume a release, I want per-plugin versions and the
  changelog in one place, so I know what changed and what to pin.
- When an agent needs toolkit knowledge, I want `llms.txt` and per-page
  Markdown, so it can ingest the docs without a browser.

## Capabilities & scope

Grouped by site area. **MVP** is the smallest set that serves both human
audiences and agents end to end.

- **Use area (MVP):** install/update guide per tool (Claude Code, opencode
  V2), usage (invocation, skill routing), per-skill reference pages.
- **Skill reference (MVP):** all 36 skills, one page each, generated from
  the skill files' real descriptions and bodies — reason for MVP: the
  reference is the primary job for both audiences, and partial coverage
  would send readers back to the raw repo.
- **Contribute area (MVP):** editing conventions, `scripts/check.sh`,
  release flow, layout rules, per-plugin version table.
- **Shared reference (MVP):** pipeline, containers, databases, guard
  hooks, license — the README's conceptual sections as pages.
- **Localization-ready (MVP):** English (en-US) only at launch; typed
  namespaces and locale-keyed output mean a missing translation for any
  future locale is a build error, so adding one is additive.
- **LLM-readable output (MVP):** `llms.txt`, `llms-full.txt`, one `.md`
  per page from the same typed content.
- **Search (MVP):** client-side search over titles and content.
- **R2 deploy + environment (MVP):** CI builds and syncs to Cloudflare R2
  under `https://docs.lucasvmigotto.me/ai-gent/`, environment
  `ai-gent-docs`.
- **Version display (Next):** site version from the latest git tag plus the
  generated per-plugin table (already content in MVP; automated staleness
  signaling is Next).
- **Offline bundle (Later):** downloadable archive of the Markdown pages.

## Key journeys

1. **Install and first skill.** A developer lands, picks their tool
   (Claude Code or opencode), runs the install command, verifies with the
   documented check, and invokes their first skill from the usage page. If
   the install fails, the page's troubleshooting (drawn from `setup.sh`
   behavior) tells them what to check. Success: first skill invoked in one
   sitting.
2. **First contribution.** A contributor reads the conventions, edits a
   skill description, runs `scripts/check.sh` locally, and opens a change
   knowing the release automation handles versions. If checks fail, the
   contributing pages map each failure class to its fix.
3. **Agent ingestion.** An agent fetches `llms.txt`, follows a page link to
   its `.md`, and answers a toolkit question with a file reference — no
   JavaScript executed at any step.

## Domain overview

The site documents a fixed-shape repository. Core concepts and relations:

```
ai-gent repo
 ├── plugins/<name>/            9 plugins (project, frontend, backend, qa,
 │   ├── .claude-plugin/         devcontainer, devsecops, db, git, compact)
 │   ├── skills/<skill>/        36 skills total → 36 reference pages
 │   ├── references/            on-demand detail → linked reading
 │   ├── hooks/ · scripts/      enforcement + runners → Contribute pages
 │   └── evals/                 routing tests → Contribute pages
 ├── shared/                    pipeline, containers, reading-code, spec-kit
 │                              → Shared reference pages
 ├── scripts/                   check, release, install, opencode-config
 │                              → Contribute pages
 └── setup.sh / install.sh      → Install pages
```

Lifecycles: a release flows Conventional Commits → `release.py` → per-plugin
version bumps → tag → GitHub Release; the site re-renders that state on
every build. A skill's documented description is loaded by every agent
session, so a stale skill page misleads agents — freshness is a correctness
property, not a nicety.

## Glossary

- **Skill** — a `SKILL.md` with frontmatter (`name`, `description`) plus
  instructions a model loads on demand. Not: plugin, command, agent.
- **Plugin** — a `plugins/<name>/` directory bundling related skills with a
  manifest (`.claude-plugin/plugin.json`). Not: skill, package, extension.
- **Pipeline** — the ordered stage chain (brief → architecture → spec →
  builds → QA → docs) defined by `shared/pipeline.md`. Not: CI pipeline,
  workflow file.
- **Guard hook** — a `PreToolUse` hook mechanically enforcing `git` or `db`
  rules in Claude Code. Not: permission rule, policy.
- **Guard plugin** — `opencode/guard/`, the OpenCode V2 plugin enforcing
  the same guards. Not: guard hook.
- **Spec Kit** — GitHub's spec-driven workflow (`specify` CLI) the pipeline
  builds on. Not: specification (generic).
- **Tools layer** — the thin `tools` stage of a Containerfile where agents
  and CI run. Not: devcontainer, IDE environment.
- **Brief** — `docs/product/brief.md`, the pipeline's first artifact. Not:
  spec, plan.
- **Handoff** — the closing line of a stage: what was written, what's open,
  next stage. Not: summary, changelog entry.
- **Maturity label** — the Implemented / Planned / Partially Implemented /
  Unavailable tag carried inline on site content. Not: badge, status.

## Business rules

- Every behavior claim on the site derives from the repo's code; maturity
  labels mark anything not yet implemented (source: `project:docs` skill,
  non-negotiable rules).
- No secrets in examples, build output, or logs; placeholders only
  (source: `project:docs` skill).
- Skill descriptions stay single-line, ≤ 400 chars, no `: ` or ` #`
  (source: `AGENTS.md` — every session loads them).
- A plugin skill's opencode name `<plugin>-<skill>` must be lowercase
  kebab-case (source: `AGENTS.md`).
- Releases are automated from Conventional Commits; versions are never
  bumped by hand (source: `AGENTS.md`, `README.md`).
- `scripts/check.sh` runs before every commit; CI runs the same script
  (source: `AGENTS.md`).

## Non-functional requirements

- **Performance:** static HTML + hashed assets, long-lived immutable cache;
  entry HTML, `llms.txt`, `llms-full.txt`, `.md` pages served `no-cache`.
  First-load budget [ASSUMPTION: under 200 KB transferred on the landing
  page; to confirm in `project:architecture`].
- **Availability:** served from Cloudflare R2; no runtime backend, so the
  site is as available as the bucket + CDN.
- **Scalability:** read-only static content; expected volumes are modest
  (the toolkit's user base, not a consumer product) — no load targets.
- **Security:** no secrets in the repo or the build; deploy credentials
  live in the `ai-gent-docs` environment's variables/secrets; least
  privilege CI permissions; supply-chain baseline owned by
  `devsecops:pipeline`.
- **Privacy & compliance:** no personal data collected; no analytics in
  MVP — N/A with reason (success is measured without tracking; see
  Success metrics).
- **Accessibility:** WCAG 2.2 AA — keyboard operable, visible focus,
  semantic HTML before ARIA, `prefers-reduced-motion` honored.
- **Localization:** English (en-US) at launch; the i18n architecture
  (typed namespaces, locale-keyed output paths) is in place so a second
  locale is additive; technical identifiers untranslated.
- **Observability:** CI build status and R2 sync result per deploy; no
  runtime to monitor.
- **Supported platforms/browsers:** any modern browser (static site, hash
  routing, no SPA fallback needed).
- **Offline needs:** none in MVP (Later: downloadable Markdown archive).

## Integrations & external systems

| System | Direction | Role | If down |
|---|---|---|---|
| GitHub Actions | out (CI) | builds, tests, syncs the site | no deploy; site stays at last good version |
| Cloudflare R2 | out (deploy target) | hosts the static site under `docs.lucasvmigotto.me/ai-gent/` | deploy fails; previous version keeps serving |
| `docs.lucasvmigotto.me` DNS/CDN | out | serves the `ai-gent/` prefix | site unreachable until DNS recovers |
| Docker Hub (`selenium/standalone-*`) | in (CI only) | containerized browsers for the site's e2e | e2e can't run; unit/a11y gates still hold |

## Constraints

- Branch-per-context git workflow with Conventional Commits; versions and
  tags automated — the site must not invent its own versioning.
- Stack for the site itself is set by `project:docs` (Bun · React · TS ·
  Vite · Tailwind · HashRouter · Biome); this brief states facts only.
- Single maintainer; docs stay current through automation (CI rebuild on
  content changes), not manual effort.
- Budget: static hosting on existing R2; no new paid services.

## Success metrics

- **Leading:** every release that changes documented behavior ships with
  the site rebuilt (CI trigger coverage); zero pages whose content
  contradicts the code at release time (spot-checked per release).
- **Lagging:** fewer repeated install/usage questions to the maintainer;
  `llms.txt` and per-page `.md` fetched by agents (server access logs as
  proxy — no client tracking).
- Target: from the first release after launch, no user question answered
  by content the site should have had.

## Architecture drivers

1. Truthfulness to code (rank 1): stale docs mislead agents, not just
   humans — drives generated-from-source content and CI rebuilds.
2. Localization-readiness (rank 2): the site ships English only, but the
   typed `Messages` contract and locale-keyed output keep a second locale
   an additive content task.
3. Zero-backend static hosting on existing R2 under a path prefix
   (`/ai-gent/`) (rank 3): drives `base: "/ai-gent/"`, hash routing, and
   prefix-aware sync.
4. Agent ingestion without JavaScript (rank 4): drives same-source
   Markdown output (`llms.txt`, per-page `.md`).
5. Usage volumes: modest read traffic; no DAU/SLO numbers to size from —
   explicit unknown, no capacity model needed.
6. Team: one maintainer; automation over process.

## Candidate feature map

In dependency order; `project:spec` is N/A here (the site is built directly
by `project:docs` phases, not Spec Kit features — the host repo has no
`specs/`).

1. **Install & usage pages** — per-tool install, verification, invocation.
   Priority: highest. Depends on: nothing.
2. **Skill reference (36 pages)** — generated from skill sources.
   Priority: highest. Depends on: 1 (shared layout/scaffold).
3. **Contribute pages** — conventions, checks, releases, versions.
   Priority: high. Depends on: 1.
4. **Shared reference pages** — pipeline, containers, databases, guards.
   Priority: high. Depends on: 1.
5. **LLM output** — `llms.txt`, `llms-full.txt`, per-page `.md`.
   Priority: high (launch-blocking). Depends on: 2–4.
6. **Search** — client-side search. Priority: medium. Depends
   on: 2–4.
7. **CI + R2 deploy** — workflow, environment, prefix sync. Priority:
   highest (nothing is "launched" without it). Depends on: 1.

## Risks & assumptions

- **Risk** (medium likelihood, high impact): docs drift from code between
  releases. Mitigation: generate reference content from sources; CI
  rebuilds on content paths; release checklist includes a docs-diff look.
- **Risk** (low, low): a second locale drifts or ships half-translated.
  Mitigation: typed namespaces and a completeness test fail the build on a
  gap; a locale is added deliberately, not partially.
- **Risk** (low, medium): R2 path-prefix sync misconfigured (sibling
  content clobbered or prefix 404s). Mitigation: prefix-scoped sync,
  endpoint validation, staging check before first production sync.
- **[ASSUMPTION: landing-page transfer budget under 200 KB]** — to confirm
  in `project:architecture` (N/A if the docs build stays on defaults).
- **[ASSUMPTION: no analytics in MVP; access logs suffice as proxy]** —
  accepted for privacy; revisit if success can't be judged.
- **[ASSUMPTION: the 9 plugin manifests plus `plugin.json` versions are
  the version source of truth for the versions table]** — latest git tag
  for the site version.

## Open questions

- None open: placement (`docs/site/`), audiences (users + contributors),
  upstream-first, locales (English at launch, i18n-ready), deploy (R2),
  skill depth (all 36), and the public URL were all decided with the user
  before writing.

## Decision log

- 2026-10-04, site placement `docs/site/` over `site/`: the site documents
  the toolkit rather than being the product; alternatives: hub-type root
  `site/`. Reason: `docs/`-of-`docs/` nesting argument doesn't apply —
  there is a host codebase to sit beside.
- 2026-10-04, MVP covers all 36 skill pages (not plugin-level summaries):
  the reference is the primary job; partial coverage sends readers back
  to the raw repo.
- 2026-10-04, pt-BR full parity at launch (not core-first): language must
  never be a reason to read stale material; enforced by typed namespaces.
- 2026-10-05, **reversed**: drop the pt-BR translations; ship English only.
  The i18n architecture stays (typed `Messages`, `LOCALES`, locale-keyed
  `llms`/Markdown paths, a completeness test), so a locale is added
  additively. Reason: parity doubles review load per change and would
  require translating 36 skill descriptions that then drift; the brief's
  own risk register named this. Alternatives: keep pt-BR content
  (rejected — maintenance), silent en-US fallback (rejected — hides gaps).
- 2026-10-04, success = adoption + fewer questions (not freshness-only):
  the site exists to reduce maintainer load and onboard users.
- 2026-10-04, deploy to Cloudflare R2 at
  `https://docs.lucasvmigotto.me/ai-gent/`, environment `ai-gent-docs`.
