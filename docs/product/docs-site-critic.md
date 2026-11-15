# Docs site critic review

Scope: source review for docs-site 2.0.0 preparation and the ai-gent 3.0.0
cutover, against `specs/040-docs-site-2/spec.md`. Keep the existing design and
HashRouter. Findings distinguish current source contradictions from missing
cutover guidance; a release contract is not evidence that an asset is published.
Published downloads, generated-artifact parity, rendered browser behavior and
five-minute completion time are **not verified**. Windows, macOS and aarch64
being untested is an accepted release disclosure, not a defect in this review.

## Recount and baseline

- Recounted manifest-bearing directories and their `skills/*/SKILL.md` files:
  **13 plugins, 50 skills**. Breakdown: agents 3, backend 3, compact 1, db 4,
  devcontainer 4, devsecops 6, frontend 4, git 1, project 12, qa 4, repo 3,
  secrets 1, security 4. `generate-content.mjs:89` onward reads these files;
  its `counts` at line 166 and `pages.tsx:71` use array lengths. No hard-coded
  count discrepancy found in the inspected site source.
- `setup.sh:111` declares **9 agents**; `shared/tiers.json` assigns 1 to Tier 1,
  1 to Tier 2 and 7 to Tier 3, with **10 matrix rows**. These counts agree.
- `i18n.tsx:9,558` registers **1 locale**, en-US. Evaluating the message object
  gives **258 leaf paths**, none empty; missing keys between registered locales:
  **0**. This is an English-only site, not an incomplete multilingual release.
- Reconstructed the search entries in memory from source: **98 entries** =
  50 skills + 6 pages + 42 headings. All **133 quoted trigger phrases** in the
  skill descriptions retrieve their own skill with the real `searchIndex`
  scorer in a skill-only index. This checks retrieval, not first-place ranking
  against page results or agent invocation at runtime.
- The release-wording scan finds no ai-gent release with a `v` prefix in
  `docs/site/src` or `docs/site/scripts`. `i18n.tsx:548,550` uses that notation
  for the GPL license only; do not treat license wording as a release defect.

## Ranked findings

### F01 — High: copy buttons execute mutually exclusive maintenance actions

Evidence: `docs/site/src/content/sections.ts:83` joins list, rollback and prune
into one copyable block. Lines 89–91 join doctor, update check, update and update
rollback. `components.tsx:243` copies the entire block. Pasting the first block
can change the active version and remove old versions; pasting the second can
install an update and immediately undo it. These are presented as the normal
Install/After install path.

Fix: give list, update, rollback and prune separate labeled blocks. Make the
first-run block contain only `ai-gent doctor` and `ai-gent update --check`.
Explain prune separately as an optional removal operation. Check the Markdown
twin has the same separation.

### F02 — High: the pin example installs content without the required binary

Evidence: `docs/site/src/content/install-facts.ts:43` pins `VERSION=2.26.0`.
`install.sh:11` explicitly says 2.26 and older use the content-only package.
The same Install page immediately recommends binary commands through
`sections.ts:89`. Its missing-binary remedy says reinstall, which can send a
reader back through the same pin (`i18n.tsx:297`).

Fix: use a published binary-bearing release as the pin example, switching to
`VERSION=3.0.0` when that release is available. State that a historical
content-only pin cannot satisfy the current binary instructions. Strengthen
`tests/install-drift.test.ts` beyond checking that an old example string still
exists in the installer header.

### F03 — High: native Windows installation exists but the page omits its path

Evidence: `docs/site/src/i18n.tsx:266` offers WSL/Git Bash and says PowerShell
support is not documented. `install.ps1:7` supplies the native command and
lines 8–29 document flags, junctions, install root and exit codes. Contract A5
decides native installation; the matrix's native-Windows reason still calls it
an open decision (`shared/tiers.json`, row `windows`).

Fix: add the exact native command
`irm https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.ps1 | iex`,
pinning with `-Version`, checksum-before-unpack behavior, `-VerifyAttestation`,
`-Rollback`, and `ai-gent doctor` after opening a new terminal. Document
`%LOCALAPPDATA%\ai-gent` and the actual PATH behavior (`setup.ps1:334`). Keep
WSL/Git Bash as alternatives, and retain the untested-platform disclosure.
Published asset availability and native execution are **not verified**; gate
publication on the release, rather than claiming tests that did not run.

### F04 — High: migration instructions omit a known installer stop condition

Evidence: `docs/site/src/i18n.tsx:288` says git mode “goes away” and supplies only
move-and-reinstall guidance. `install.sh:258` already rejects nonempty
`AI_GENT_MODE`, `AI_GENT_BRANCH`, `AI_GENT_REPO` or `AI_GENT_DIR`; its diagnostic
at line 248 explicitly instructs users to unset them. The site's copyable
migration block (`install-facts.ts`, fact `migrate`) does not do that. A reader
can move the clone successfully and then stop at the installer.

Fix: state removal in the past tense for 3.0.0, identify all removed variables,
and tell users to remove them from their shell profile and unset them in the
current shell before installing. Retain a clearly labeled legacy migration
recipe without presenting git mode as an available installation choice.

### F05 — Medium: the five-minute path ends before a concrete first skill

Evidence: `docs/site/src/i18n.tsx:290` only says to type a task or invoke a skill;
`sections.ts:96` adds no example. Verification at `install-facts.ts`, fact
`verifyClaude`, combines Claude and OpenCode checks even for users who selected
only one agent. Troubleshooting uses bare `./setup.sh` from an unspecified
working directory (`i18n.tsx:328,333,342`). The claim that only skill links are
written outside the root (`i18n.tsx:251`) also contradicts the binary link
documented in `install.sh:11` and the site's own PATH remedy.

Fix: provide a short numbered path per agent: install, ensure the binary is on
PATH, start a fresh agent session, invoke `/project:survey` in Claude Code or
`$project-survey` in Codex, and describe the expected read-only survey. Offer
only the selected agent's verification command. Use
`bash ~/.local/share/ai-gent/current/setup.sh ...` for POSIX package remedies,
with the native Windows counterpart. Correct the outside-root statement.
Linux/macOS already have the exact curl command, automatic checksum explanation
and rollback command; timing and actual successful first invocation are
**not verified**.

### F06 — Medium: there is no complete migration or binary reference entry point

Evidence: `App.tsx:25` onward has Install, Skills, Flow, Compatibility,
Contribute, Reference and Changelog, but no migration or CLI reference route.
`referenceSections` lists contracts rather than binary usage. The Contribute
page (`i18n.tsx:368`) omits the checkout + `scripts/dev-bin.sh` setup already
documented in `README.md:499`. The release contract contains the following
**12 breaking-change rows**, recounted B1 through B12:

| Contract row | Site coverage and gap |
|---|---|
| B1: git mode removed | Future-tense text and incomplete recovery; F04. |
| B2: package-only links, no live edits | Install layout exists; no warning that installed edits are replaced. |
| B3: contributor checkout + binary | Contribute omits `scripts/dev-bin.sh`. |
| B4: missing binary, no fallback | Troubleshooting says no Python fallback; lacks the contributor recovery path. |
| B5: shell guard logic removed | Reference describes hooks, but not launcher versus decision engine or cutover status. |
| B6: OpenCode rules replaced | Matrix still cites `opencode/guard/rules.mjs`; no transition explanation. |
| B7: old tool paths replaced | Matrix still names `wt.py` and describes plain scripts; no old-to-new command table. |
| B8: update scripts removed | Binary update commands exist; users of old script paths get no mapping. |
| B9: content-only asset removed | No warning for automation or direct updates from older package installers. |
| B10: Tier 3 frozen/best-effort | Compatibility covers it explicitly. |
| B11: Python/uv exception for db | No concise prerequisite explanation distinguishing db from ported tools. |
| B12: version output format | No guidance for parsers to use `ai-gent version --json`. |

Fix: add a compact migration page, linked from Install and Reference, using
`specs/039-release-3-contract/contracts/migration-guide.md` as the basis.
Generate a CLI summary from actual clap metadata when available, including
flags and exit codes. Do not promise every candidate port has shipped:
`rust/crates/ai-gent/src/cli.rs` currently defines `wt`, `tokens`, `repo` and
`spec`, but no `compact` command. Explain current shadow mode separately from
the eventual binary-only guards; README line 419 and `cli.rs` still describe
shell decisions in shadow mode. Recheck B5–B8 at the cutover, then refresh the
matrix's source references together with the tool names.

### F07 — Medium: search handles triggers but misses common task synonyms

Evidence: source-level calls to `searchIndex` over the reconstructed page and
skill fields return no results for “continuous integration”, “installation”,
“uninstall” and “permission denied”; the skill-only query “upgrade dependencies”
also returns none. “pull request” ranks `devsecops-pipeline` before `repo-pr`.
`search.ts:141` requires every term to match; there is no synonym normalization.
`generate-search-index.ts:40` indexes top-level command blocks but not paragraph,
list or troubleshooting-body text, despite uninstall being documented there.
The reconstruction omits the script-name enrichment; these query words do not
come from that enrichment.

Fix: add a small tested alias map for common user vocabulary, and index relevant
section bodies and troubleshooting commands. Test expected intent for the
queries above alongside existing trigger phrases. Preserve weighted command
matches and typo tolerance. From home, search plus a result click can already
reach a skill; from other pages, Search plus a result click provides a
two-click route when the query matches. Actual visitor success is **not verified**.

### F08 — Medium: heading results do not take readers to the heading

Evidence: `docs/site/scripts/generate-search-index.ts:117` assigns every heading
only `p.url`; `components.tsx:278` renders headings without IDs. A result for
rollback or a compatibility feature opens the top of a long page. Reference
list terms and Flow's custom step headings are also absent from `headingsOf`
(`generate-search-index.ts:44`).

Fix: share stable section IDs between renderers and the index, then scroll and
focus the selected section in a HashRouter-compatible way. Index Reference
terms and Flow custom headings. Keep the existing router; this does not need
the deferred prerendering work.

### F09 — Medium: Markdown invocation instructions differ from the UI

Evidence: `docs/site/src/pages.tsx:173` shows `/plugin-skill` for OpenCode/Cursor
and `$plugin-skill` for Codex. `scripts/llms-pages.ts:36` emits only the bare
`plugin-skill` for all non-Claude agents. A model consuming the Markdown gets
less precise invocation guidance than a human reading the skill page.

Fix: generate both invocation blocks from one per-agent formatter, retaining
the command prefixes and the plain-language alternative. Assert matching
examples in the Markdown and rendered skill page.

### F10 — Medium: Markdown output does not cover every public page

Evidence: `scripts/llms-pages.ts:80` onward declares **6 static twins** and maps
all **50 skills**, yielding **56 Markdown pages** per registered locale.
`App.tsx` also exposes the landing page and Skills index, which have no twins.
`tests/llms.test.tsx` calls its check “every static route” but its PAGES list
excludes those routes. Thus llms.txt and llms-full.txt are complete relative
to the generator's input list, not the site's public page set.

Fix: add lightweight landing/finder and Skills-index twins, including task
discovery guidance, and derive the completeness check from an explicit route
manifest shared with the app. Keep both LLM aggregate files generated from
that complete list. Actual emitted-file parity is **not verified**.

### F11 — Medium: search-control boundary has a contrast risk

Evidence: `components.tsx:506` gives the input a `border-muted/40` boundary on
`bg-surface`. Using the CSS tokens (`theme.css:10–29`) and sRGB alpha compositing,
the border-to-surface ratios are approximately **2.17:1 dark** and **1.90:1 light**,
below 3:1 if this boundary identifies the control. Muted text against surface
is **6.78:1 dark / 7.13:1 light**; signal is **8.19:1 / 6.92:1**, so those tokens
do not themselves establish a text-contrast defect. Computed browser colors
and a complete WCAG conformance result are **not verified**.

Fix: use a dedicated control-border token with sufficient contrast in both
themes. Preserve the signal focus outline (`theme.css:53`). Test input and
select boundaries in the rendered themes rather than strengthening every
decorative divider.

### F12 — Medium: copy feedback announces success after clipboard failure

Evidence: `docs/site/src/components.tsx:243–247` catches a failed clipboard
write but unconditionally sets `copied` true. The polite live region at line
268 therefore announces success even when nothing was copied.

Fix: set success only after a successful write. On failure, show and announce
a translated instruction to select and copy the visible command manually.
Verify both outcomes with a rejected/resolved clipboard stub.

### F13 — Low: reference Markdown drops actionable links

Evidence: HTML reference items use `sourceUrl(item.path)` in
`components.tsx:311`; `content/sections.ts:230` onward serializes list paths as
plain text. A Markdown reader gets `shared/pipeline.md` rather than a link to
the versioned source. Section sharing prevents prose drift but not link loss.

Fix: serialize repository paths as absolute links using the same generated
source base as HTML. Test the Reference twin contains resolvable source URLs.

### F14 — Low: translation completeness does not cover hard-coded UI strings

Evidence: missing registered-locale keys are **0**, but accessible names
“Primary”, “Primary mobile”, “Breadcrumb” and “ai-gent version” live in
`components.tsx:140,180,229` and `pages.tsx:163`. Contribution table headers
are literal strings in `sections.ts:127`. The locale completeness test compares
message objects and cannot see these literals.

Fix: move user-facing names into Messages when those components next change;
extend the check to these labels before adding another locale. Do not add
translations or present English-only scope as a release blocker.

### F15 — Low, later: validate dense layouts without changing the visual system

Evidence: `components.tsx:135` has a nonwrapping header, with desktop navigation
enabled at `md`; skill commands at line 409 have no explicit long-word wrapping.
The compatibility table deliberately uses a 40rem minimum inside a named,
focusable scrolling region (`components.tsx:325,368`), and code blocks scroll
internally at line 264. These are reasonable protections, not proof that every
page fits. Overflow at 320px and 200% zoom is **not verified**.

Fix later: measure header, long command rows and matrix navigation at narrow
widths and zoom. Add local wrapping/min-width adjustments only where overflow
is reproduced. Retain typography, colors, shell and themes; no redesign.

## Review limits and owner decisions

Source provides combobox/listbox semantics, arrow/Enter/Escape handling, a polite
result count and a visible-focus token (`components.tsx:465–510`). The skip
button focuses main without changing the hash (`components.tsx:588`). Actual
focus after route changes, screen-reader announcements and mobile keyboard
scrolling are **not verified**; check these in the existing browser smoke.

Hosting is already decided as R2 in
`specs/040-docs-site-2/contracts/hosting-decision.md`, Sign-off. Keep HashRouter
within this task's scope. No new hosting or Windows policy decision is needed
for the fixes above. Actual deploy-key prefix scope remains an owner-only fact,
**not verified** here; this review does not inspect credentials or require a
hosting change.

| Finding id | Severity | Effort | Type | Needs owner's decision? |
|---|---|---|---|---|
| F01 | high | S | structure | No |
| F02 | high | S | content | No; use the published release |
| F03 | high | M | content | No; native route already decided |
| F04 | high | S | content | No |
| F05 | medium | M | content | No |
| F06 | medium | M | new page | No; release-contract scope |
| F07 | medium | M | generator | No |
| F08 | medium | M | structure | No |
| F09 | medium | S | generator | No |
| F10 | medium | M | generator | No |
| F11 | medium | S | structure | No |
| F12 | medium | S | structure | No |
| F13 | low | S | generator | No |
| F14 | low | S | content | No |
| F15 | low | M | later | Yes, when scheduling later visual work |
