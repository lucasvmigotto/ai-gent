# Phase 2 — Scaffold in detail

Bun · React · TypeScript (the current stable major, pinned; verify the
resolved version with `bunx tsc --version` + `bun pm ls typescript`) · Vite · Tailwind CSS ·
React Router **`HashRouter`** (static hosts provide no SPA fallback, so
hash routing keeps deep links working) · Biome (not ESLint). No runtime
backend, no heavy doc framework.

- Versions injected at build time from the host's sources of truth
  (e.g. read `Cargo.toml` in `vite.config.ts`), never hand-maintained.
  The footer names every versioned part and then the site itself —
  `<project>@v<version> · docs@v<docs version>` (`app@v1.2.0 · api@v1.2.0
  · docs@v1.0.0` for a multi-part repo) — each linked to the repository
  tree at its release tag, or at the commit (`v1.2.0+abc1234`) for a build
  past it. The docs version is `docs/site/package.json`'s, raised by the
  release flow when the site changes. Read
  `version-footer.md` when building or fixing the footer.
- Page metadata + Open Graph + favicon + `index.html`; distinct project
  branding (dark-first tokens adapted per project, restrained motifs).
- **Visual identity.** If `docs/product/ux-vision.md` exists, derive the
  site's tokens from it (and from `specs/000-design-system/` if built),
  so the docs look like the product's family. If not, invoke
  `frontend:uiux` scoped to the docs site (audience: the site's readers)
  for a lightweight direction — its visual-direction method avoids the
  templated docs look. Hold the site to `frontend:build`'s quality floor.
