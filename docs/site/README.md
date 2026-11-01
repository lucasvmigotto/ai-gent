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
- Copy lives in `src/i18n.tsx`. `LOCALES` is the single source of truth and
  every registered locale must be a complete `Messages`; a completeness
  test fails the build on a gap, so adding a locale is additive.
- The deploy syncs only under the repository-name R2 prefix
  (`/ai-gent/`); never sync at the bucket root — sibling docs share it.
- e2e smoke drives the built site in containerized Chrome:
  `bun run build`, serve it, then `SELENIUM_URL=<url> bun run e2e` (CI job
  `e2e`, pinned `selenium/standalone-chrome` — never a host driver).
- Open Graph image: `bun run og` regenerates the committed
  `public/og-image.png` (1200×630) from `scripts/og-image.svg`; CI asserts
  it is built and advertised.
