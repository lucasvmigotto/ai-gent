# ai-gent docs site

A static, English-only documentation site for this toolkit (Bun · React ·
TypeScript · Vite · Tailwind · HashRouter · Biome), deployed to Cloudflare
R2 at `https://docs.lucasvmigotto.me/ai-gent/` by
`.github/workflows/docs-ci.yml`. The brief and UX vision live in
`docs/product/`.

- Run commands in `docs/site/`: `bun install`, then
  `bun run dev|build|test|lint|typecheck`.
- Content is generated from this repository by
  `scripts/generate-content.mjs` into `src/content/generated.json`
  (gitignored). Never hand-edit it — change the source and re-run
  `bun run gen` (it runs automatically before `dev`, `build`, `test` and
  `typecheck`).
- The Compatibility page is generated from `shared/tiers.json` (rows, tiers,
  cell state, reason, source, verified flag); `scripts/consistency.py` and the
  build both fail when a `setup.sh` `AGENT_IDS` agent is not in exactly one
  tier or a cell lacks a state, a reason or an existing source file. A cell the
  repository cannot show says `unverified` (and `verified: false`).
- The Install page's commands live in `src/content/install-facts.ts`, each with
  the repository file and text that backs it; `tests/install-drift.test.ts`
  fails when `README.md`, `install.sh` or the binary's CLI no longer match.
- Search runs over `src/content/search-index.json` (gitignored, written by
  `scripts/generate-search-index.ts` as part of `bun run gen`) through the pure
  scorer in `src/search.ts`. `tests/search.test.ts` is its contract: change a
  weight only if that table still reads right. Skill names, descriptions and
  triggers are not copied into the index, so the bundle holds each once.
- Releases are written without a `v` (`2.27.0`, `ai-gent@2.27.0`);
  `tests/release-names.test.ts` fails on a `v1.2.3` or `@v1` in `src/` or
  `scripts/`.
- Copy lives in `src/i18n.tsx`. `LOCALES` is the single source of truth and
  every registered locale must be a complete `Messages`; a completeness
  test fails the build on a gap, so adding a locale is additive.
- The deploy syncs only under the repository-name R2 prefix
  (`/ai-gent/`); never sync at the bucket root — sibling docs share it.
- e2e smoke (install page and copy button, skip link by keyboard on a deep
  link, compatibility table, keyboard search) drives the built site in
  containerized Chrome:
  `bun run build`, serve it, then `SELENIUM_URL=<url> bun run e2e` (CI job
  `e2e`, pinned `selenium/standalone-chrome` — never a host driver).
- Open Graph image: `bun run og` regenerates the committed
  `public/og-image.png` (1200×630) from `scripts/og-image.svg`; CI asserts
  it is built and advertised.
