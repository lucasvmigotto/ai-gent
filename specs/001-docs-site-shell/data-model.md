Reconstructed by project:introspec on 2026-10-18 from 384c771

# Data model — generated content

Build-time only, in `docs/site/src/content/generated.json` (gitignored), typed by `docs/site/src/content.ts` [OBSERVED: docs/site/src/content.ts:3-29].

| Entity | Fields | Source |
|---|---|---|
| Content | `version`, `versions[]`, `sourceBase`, `plugins[]`, `skills[]`, `counts {plugins, skills}`, `readme`, `changelog` | `docs/site/scripts/generate-content.mjs:96-105` |
| Plugin | `name` (directory), `dir`, `version`, `description` (from `plugin.json`), `skills[]` (skill ids) | `generate-content.mjs:87-93` |
| Skill | `id` = `<plugin>-<skill>`, `plugin`, `skill`, `name`, `description` (frontmatter), `body`, `sourcePath`, `sourceUrl` | `generate-content.mjs:74-85` |
| Version | `name` (`ai-gent` or `docs`), `version`, `label` (`name@vX.Y.Z` or `name@dev…`), `href` (tree URL or null) | `docs/site/scripts/version.mjs:37-53` |

Relations: Plugin 1—n Skill by `skill.plugin = plugin.name`. A skill's `id` is also its URL slug (`#/use/skills/<id>`) and its name in the non-Claude agents.

Invariants:

- `id` is unique across the site [OBSERVED: docs/site/tests/content.test.ts:13].
- `sourceUrl` points at the exact tag, else the commit, else `main` [OBSERVED: generate-content.mjs:23].

Browser storage (per viewer): `ai-gent-docs-theme` ∈ {`light`, `dark`}; `ai-gent-docs-locale` ∈ `LOCALES`.
