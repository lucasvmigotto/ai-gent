# UX and language vision — ai-gent docs site

Status: Draft

## Summary

The ai-gent docs site is a reference-first documentation site for a
developer toolkit: its primary job is getting a working developer to the
right skill page in under a minute, in the dark. Copy ships in English
(en-US) at launch, with the i18n architecture in place so another locale
is an additive content task. The landing page is a skill finder, not a sales pitch — search
and the skill index are the hero; install is one step away in the nav. The
visual language is terminal-native (the toolkit's own medium) without
terminal pastiche: a ruled-ledger layout, one serif display face against a
quiet grotesque, monospace reserved strictly for commands and code, on a
violet-ink dark theme with a single marigold signal color.

## Inputs used (and missing)

- Used: `docs/product/brief.md` — audiences (toolkit users, contributors,
  agents over HTTP, maintainer), MVP scope (all 36 skill pages, English
  with the i18n architecture, `llms.txt` output, R2 deploy), glossary, constraints (single
  maintainer, automation over process), public URL
  `https://docs.lucasvmigotto.me/ai-gent/`.
- Used: user decisions — dark-first theme, find-a-skill landing priority,
  terminal-native personality, a site identity of its own (not matched to
  the existing personal site).
- Missing: `docs/product/references/` does not exist — no taste references
  were provided, so the direction below is proposed, not derived.
- Missing: no existing UI to inventory (greenfield site).
- Missing: no domain model or specs exist for ai-gent (toolkit repo, no
  `specs/`) — screen inventory below stands in for them.

## Reference analysis

No references directory was provided, so there is no reference table. The
direction was built from the subject matter instead: agent skills invoked
from a prompt, symlinked installs, plugin manifests, guard hooks, and a
bilingual reader base.

## Personas & jobs

- **Rafael, backend developer (primary).** Uses opencode daily, just
  installed ai-gent. Goal: answer "which skill do I invoke for this task"
  without reading the repo. Context: terminal open beside the browser,
  desktop, broadband. Expertise: senior engineer, new to this toolkit.
  Leaves when search fails twice or a skill page doesn't show the invoke
  command.
- **Mariana, contributor.** Wants to fix a skill description and land it
  with `scripts/check.sh` green. Goal: conventions and checks in one
  place. Context: the ai-gent repo checked out locally. Expertise: knows
  git and CI, learning this repo's rules. Leaves when the contributing
  path contradicts the repo.
- **An agent over HTTP.** Fetches `llms.txt`, follows `.md` links. Goal:
  complete toolkit map as plain Markdown. No UI needs; its job shapes the
  `llms.txt` and per-page `.md` requirements, not the screens.
- **Lucas, maintainer/operator.** Goal: every behavior-changing release
  rebuilds the site with no manual step. Reads deploy status, not pages.

Jobs (from the brief): install for my tool; find the right skill; make a
correct first contribution; consume releases and versions; ingest via
`llms.txt`.

## Principles

1. **The skill page is the product.** Every navigation choice, the search
   index, and the landing hero exist to shorten the path to a skill page.
2. **Copy the terminal's honesty.** Show exact commands, exact paths,
   exact versions. Never paraphrase something the reader will paste.
3. **One memorable thing per screen.** The ledger rules and the marigold
   signal carry the identity; everything else stays quiet.
4. **Globalization is architecture, not a translation pass.** Copy lives in
   a typed `Messages` contract and output is locale-keyed, so a second
   locale is additive; no string is hardcoded into a component.
5. **Stale is wrong.** A skill page older than its skill is a bug, so the
   design must make generated-from-source content visible, not hide it.

## Information architecture

```
landing (skill finder: search + index + install shortcut)
├── /use/install            per-tool install, verify, troubleshoot
├── /use/skills              full index, grouped by plugin, filterable
│   └── /use/skills/<plugin>-<skill>   36 pages: what, when, invoke, source
├── /contribute              conventions, checks, releases, versions
├── /reference/pipeline      the product pipeline
├── /reference/containers     container-first rules
├── /reference/databases      safe database access
├── /reference/guards         guard hooks + guard plugin
├── /changelog                releases, per-plugin versions
└── /llms.txt                 agent entry (not in nav)
```

Navigation model: a slim top bar (site mark, section links for Use /
Contribute / Reference, locale switcher, theme toggle, search trigger) plus
a section sidebar inside Use and Contribute. Rationale: the corpus is a
reference, not an app — persistent top-level orientation plus local
hierarchy beats tabs or a command palette as the primary model; search
(prominent, keyboard-triggered with `/`) is the shortcut for the primary
persona. Section names use brief glossary terms (Skill, Plugin, Pipeline,
Guard hook).

## Key flows

1. **Find a skill (primary).** Entry: landing search or `/use/skills`.
   Steps: type task words ("load test") → results ranked by skill
   description → open `qa-load` page → read when-to-invoke + invoke
   command → done in the terminal. Decision point: search vs. browse by
   plugin; both stay visible. Failure branch: no match → empty state
   names the closest plugins and links the full index (never a dead end).
   Honest step count: three. Nothing to remove.
2. **Install.** Entry: nav or landing shortcut. Steps: pick tool tab
   (Claude Code / opencode) → run command → verify → invoke first skill.
   Failure branch: per-tool troubleshooting drawn from `setup.sh`
   behavior.
3. **Contribute.** Entry: Contribute section. Steps: conventions →
   change → `scripts/check.sh` → release notes. Failure branch: each
   check failure class maps to its fix.

## Screen inventory & wireframes

- **Landing:** purpose — route every visitor in seconds. Primary action:
  search skills. Priority: search field, plugin-grouped skill index,
  install shortcut, changelog pointer. Personas: Rafael first, Mariana
  second.
- **Skill page:** purpose — everything needed to invoke one skill
  correctly. Primary action: copy the invoke command. Priority: invoke
  command, when-to-use, description, link to source `SKILL.md`, related
  skills. States: current (generated-from-source marker with toolkit
  version) — no empty/loading states beyond search-index load.
- **Install page:** purpose — working install per tool. Primary action:
  copy install command. States: success checklist, per-tool
  troubleshooting.
- **Contribute page(s):** purpose — correct first change. Primary action:
  run `scripts/check.sh`. Priority: conventions, checks, release flow.
- **Reference pages:** purpose — the conceptual sections (pipeline,
  containers, databases, guards). Primary action: none (reading).

Landing, desktop:

```
+----------------------------------------------------------+
| ai-gent docs   Use Contribute Reference  [EN] [theme] [/]|
+----------------------------------------------------------+
|  Find the skill for the task.                            |
|  [ what are you trying to do? .................... ]     |
|  36 skills · 9 plugins · en-US                           |
|                                                          |
|  project ......... init architecture spec status docs .. |
|  frontend ........ uiux spec build tui                   |
|  backend ......... domain spec build                     |
|  qa .............. strategy e2e load review              |
|  ...                                                     |
|  [ Install ai-gent ]  [ Read the changelog ]             |
+----------------------------------------------------------+
```

Landing, mobile (≈375px): top bar collapses to mark + search icon +
menu; search field full-width; plugin groups stack; install shortcut
becomes a single full-width button above the index.

Skill page, desktop:

```
| Use / Skills / qa-load                                   |
| Load testing                                    [Copy /qa-load] |
| Design and run performance tests — smoke, load, ...      |
| WHEN TO INVOKE                                            |
| "load test the API", "stress test", ...                  |
| GENERATED FROM qa plugin v1.x · ai-gent 1.x              |
| Source SKILL.md · Related: qa-strategy qa-e2e             |
```

Skill page, mobile: single column; invoke command block first
(horizontally scrollable, never wrapped); related skills as a plain list.

## Visual direction & draft tokens

Two-pass method from `visual-direction.md`. Pass one — grounded in the
subject (skills, manifests, ledgers, symlinks):

- **Layout concept:** the ruled ledger. Skill index rows ruled like
  ledger lines with generous whitespace, not cards — rows encode a list
  to scan, so lines carry information. One memorable thing: the marigold
  invocation bar on skill pages (the copyable `/plugin-skill` command
  strip).
- **Color:** violet-ink dark (distinct from neutral-black defaults),
  parchment text, single marigold signal. Light theme is neutral paper,
  same signal darkened for contrast.
- **Type:** one serif display face for hero and page titles, one quiet
  grotesque for UI and body, monospace for commands and code only.

Pass two — reviewed against the AI-default list; revisions made:

- Light background is neutral paper `#FAFAF7`, not warm cream.
- Dark background is violet ink `#16121E`, not tinted near-black; accent
  is marigold `#E5A829`, not terracotta, acid green, or vermilion.
- No SaaS card kit: index uses ruled rows with two radii at most
  (controls vs. code blocks), no uniform soft shadows, no gradient wash.
- No template chrome: section labels are sentence case, no ALL-CAPS
  eyebrows; metadata uses plain sentences, not middle-dot strings; links
  have no appended arrows; monospace never sets labels or prose.
- Hero is the search field (the product's primary job), not a big
  number with a small label.

Draft tokens for `frontend:spec` to finalize:

```
color.bg / #16121E dark · #FAFAF7 light
color.surface / #1E1828 dark · #FFFFFF light
color.text / #EFE9D8 dark · #1A1626 light
color.text.muted / #A79FB2 dark · #5C5566 light
color.signal / #E5A829 dark · #7A5200 light (text use)
color.on-signal / #16121E (both)
color.ok / #7FB069 · color.danger / #D95F5F (status only)
font.display / Fraunces, Georgia, serif (titles, hero)
font.body / Inter, system-ui, sans-serif (UI, prose)
font.code / "JetBrains Mono", ui-monospace, monospace (commands/code only)
space.1–space.8 / 0.25rem base scale
radius.control / 0.375rem · radius.code / 0.5rem
motion.duration.short / 120ms · motion.duration.med / 200ms
motion.rule / motion answers actions only (open, expand, copy confirm);
  no scroll entrances, no hover transitions on rows
```

Checked contrast pairs (WCAG 2.2 AA): dark text 15.20, dark signal
8.75, dark muted 7.24, ink on signal 8.75; light text 16.92, light
signal-text 6.62, light muted 6.82 — all ≥ 4.5.

## Interaction patterns & states

- **Search:** keyboard-triggered (`/`), filters skill descriptions and
  titles across the active locale; `aria-live` announces result counts;
  empty state names the closest plugins and links the full index.
- **Copy buttons:** on every command block; confirm inline ("Copied")
  for 1.5s, announced to screen readers; no toast.
- **Theme toggle and locale switcher:** persist to `localStorage`;
  `<html lang>` syncs with the locale; switching locale keeps the page.
- **Loading:** search index loads async — skeleton rows, never a
  spinner; content pages render statically, no loading state.
- **Errors:** a missing page renders the 404 with search and the index
  link; a failed search-index fetch degrades to the static index list.
- **Offline:** fully static after first load; no offline-specific UI
  beyond the browser's own.
- **No destructive actions** on the site; nothing to confirm, nothing to
  undo.

## Responsive behavior

Breakpoints by content: below ~40rem the top bar keeps mark, search
trigger, and menu only; the skill index collapses to one column and the
plugin group headers stick while scrolling; the invoke bar stays full
width with horizontal scroll for long commands (code never wraps).
Above ~64rem the skill page gains its right-rail (on this page: source
link, version, related skills). Touch targets ≥ 24px, primary actions
44px. Nothing is hidden without a path: collapsed nav lives behind the
menu button, rail content reflows inline.

## Accessibility targets

WCAG 2.2 AA minimum: all text pairs above are ≥ 4.5 (muted included —
muted is never below AA here). Focus is always visible: 2px marigold
outline offset 2px, never removed. Keyboard paths: `/` focuses search,
`Esc` closes menu/search, every flow (find skill, install, toggle theme)
completes without a pointer. `prefers-reduced-motion`
disables the copy-confirm timing animation and any transition. Async
results (search counts, copy confirmations) announced via live regions.
`<html lang="en-US">`, synced to the active locale (one today).

## Voice & tone

Voice (constant): plain, verb-first, exact. Name things the way the repo
names them (Skill, Plugin, `setup.sh`); write commands to be pasted, not
admired. Tone shifts: onboarding is encouraging and concrete ("Run this,
then check that"); errors are blameless and fix-directed; success states
are brief. Don't: apologize in errors, hedge with "simply" or "just",
sell the toolkit on reference pages. Copy is English today; a future
locale inherits the same voice — direct, second person, technical terms
kept in English where the repo uses them (`skill`, `plugin`, `pipeline`).

## Terminology

UI uses the brief's glossary verbatim: Skill, Plugin, Pipeline, Guard
hook, Guard plugin, Spec Kit, Tools layer, Brief, Handoff. UI-only terms:
section names Use / Contribute / Reference; button verbs Copy, Install,
Verify; states Current (generated-from-source marker). No new domain
terms introduced — none needed, so the brief's glossary is unchanged.

## Microcopy patterns

- CTA verbs name the result and keep it through the flow: "Copy the
  install command" → confirmation "Copied".
- Errors say what happened plus the fix, no apology, no vagueness:
  "No skill matches 'perf test'. Try 'load' or browse the qa plugin."
- Empty states invite action: "No results — browse all 36 skills."
- Numbers and versions use the repo's forms: `1.7.0`, `/qa-load`,
  `docs/site/`. Dates in ISO.
- A future locale may run ~30% longer; layouts must not clip or overlap,
  which the typography scale already allows.

## Key-screen copy

Copy ships in English (en-US) today; keys are stable so a future locale
fills the same keys.

Landing hero:

- `landing.hero.title` — "Find the skill for the task."
- `landing.hero.search.placeholder` — "What are you trying to do?"
- `landing.hero.meta` — "36 skills · 9 plugins · en-US"
- `landing.hero.install` — "Install ai-gent"

Skill page:

- `skill.when` — "When to invoke"
- `skill.copy` — "Copy", confirmed `skill.copied` — "Copied"
- `skill.generated` — "Generated from {plugin} v{version} · ai-gent
  {version}"
- `skill.source` — "Read the source SKILL.md"
- `skill.related` — "Related skills"

Search empty:

- `search.empty.title` — "No skill matches '{query}'."
- `search.empty.body` — "Try fewer words, or browse the full index."

404:

- `notfound.title` — "This page isn't in the index."
- `notfound.body` — "Search the skills, or start from the landing page."

## Localization

English (en-US) at launch. The i18n architecture is deliberately kept:
`LOCALES` is the single source of truth, `Messages` is a typed contract
compiled as `Record<Locale, Messages>`, output and `llms`/Markdown paths
are locale-keyed, and a completeness test compares every locale's key set
against the reference. Adding a locale is therefore additive — add it to
`LOCALES` and supply a complete `Messages`; the compiler and the test fail
on any gap, and the language switcher reappears with no layout change.
No silent fallback: a locale either exists and is complete, or it isn't in
`LOCALES`. Technical identifiers (commands, flags, paths, skill IDs, URLs,
versions) stay untranslated.

## Open questions

- Site version source of truth: latest git tag vs. `CHANGELOG.md` — the
  brief assumes the tag for the site version and `plugin.json` files for
  the versions table; confirm in `project:architecture` (or accept the
  assumption when the scaffold reads it).
- R2 bucket prefix mechanics: sync lands under `ai-gent/` beside sibling
  content — confirm the exact prefix flag in the deploy workflow review
  with `devsecops:pipeline`.

## Decision log

- 2026-10-04, dark-first with light toggle: the readers live in
  terminals; a docs site for a terminal toolkit that opens light-first
  misreads its audience.
- 2026-10-04, landing prioritizes finding a skill over installing:
  install happens once, skill lookup happens daily; hero is search.
- 2026-10-04, terminal-native personality over editorial-neutral: the
  toolkit's own medium (commands, ledgers, manifests) is the distinctive
  material; a neutral editorial look would be interchangeable with any
  docs site.
- 2026-10-04, own identity rather than matching the personal site: only
  the URL prefix ties them; the docs get their own direction.
- 2026-10-04, ruled-ledger rows instead of cards; marigold signal over
  terracotta/acid defaults; mono for code only — all from the
  AI-default review, with what changed stated above.
- 2026-10-05, **reversed**: ship English only; drop the pt-BR strings.
  Keep the i18n architecture (typed `Messages`, `LOCALES`, locale-keyed
  output, completeness test). Reason: parity doubles per-change review
  load and would require translating 36 skill descriptions that then
  drift; the brief recorded that risk. Alternatives: keep pt-BR content
  (rejected — maintenance), silent en-US fallback (rejected — hides gaps).
- 2026-10-04, no terminal-interface section: none was requested for this
  site.
