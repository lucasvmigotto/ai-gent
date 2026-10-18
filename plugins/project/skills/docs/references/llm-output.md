# LLM-readable output (Phase 4b)

Part of `project:docs` (`../SKILL.md`): read it when the build emits `llms.txt`, `llms-full.txt` and the per-page Markdown.

A single-page app is close to invisible to LLMs: crawlers and agent fetch
tools don't run JavaScript, and hash routes never reach the server. So the
build also emits plain Markdown from the **same typed content** the pages
render (never a second, hand-maintained copy):

- `llms.txt` at the site root, following llmstxt.org: an `# <project>`
  title, a one-paragraph `>` summary, then `##` sections listing every
  page as `- [Title](url.md): one-line description`, with an `## Optional`
  section for material an agent can skip.
- `llms-full.txt`: every page concatenated in navigation order, for
  one-shot ingestion.
- One `.md` per page and locale (`/docs/<locale>/<page>.md`), linked from
  `llms.txt` and advertised on the HTML page with
  `<link rel="alternate" type="text/markdown" href="…">`.
- **Maturity labels in the text itself.** Every Planned, Partially
  Implemented or Unavailable item carries the label inline
  (`**Planned — not implemented yet.**`), since an LLM reading the
  Markdown never sees a badge. Roadmap pages stay separate from how-to and
  reference pages, exactly as on the site.
- Code blocks keep their language tag; tables stay Markdown tables; no
  HTML-only components (tabs, accordions) — render their content in
  full.
- Offer to create or update the repository's `AGENTS.md` (commands,
  layout, conventions for agents working *on* the code) from the same
  discovery — a different audience from `llms.txt`, which serves agents
  *using* the product. Only with the user's agreement.
