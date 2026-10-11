---
name: pitch
description: Build the project's presentation page from what the repository records — brief, specs, reviews, releases, eval results — sized short, medium or large for non-tech, basic-tech or a tech team, as a Claude Artifact, a ChatGPT Canvas or a static HTML page, every number sourced. Use for "pitch", "make a presentation of the project", "briefing for the directors", "update the pitch".
---

# Pitch — the project, presented

`/project:pitch [short|medium|large] [non-tech|basic-tech|tech-team]`,
defaults `medium` and `basic-tech`. Options: `--lang <code>` (the
audience's language; default from the brief, else ask), `--local` (also
write the static page), `--stack "<change>"`, `--apply`.

A presentation of what exists, for people who decide or will use it. It
reads; it never changes code or specs. The documentation site is
`project:docs`; where the pipeline stands is `project:status`.

## 1. Gather the facts

Read, in order, and stop when you have enough:
`docs/product/brief.md`, the pipeline artifacts in
`../../references/pipeline.md` (architecture, specs, reviews, ADRs),
`CHANGELOG.md` and tags, eval or test reports, `git log`. Count what you
cite (cases, tests, features) from the repository, now.

- **Never invent a number.** Every figure has a source in the repository.
  What is planned but not measured is a target and is labelled as such
  ("meta" in pt-BR).
- **Ask once** before writing: anything that must stay off the page (a
  backstory, a person, an internal conflict), and who will read it.
- No personal data; people by role, unless the user asks otherwise.

## 2. Shape it

Sections by size, depth by audience: `references/structure.md`. In short:

- `short`: one screen — problem, solution, current state, next step, the ask.
- `medium`: vision, components, how it works, security, phases, current
  state, validation, risks.
- `large`: medium plus method, quickstart, day-to-day use, AI effort and
  cost (collapsed).
- `non-tech` drops code and file names; `tech-team` adds commands,
  contracts, test counts and release state.

**Honest validation.** Show results as they are, with a short "how to read
these numbers" note that separates test-infrastructure failures from
product failures; compare rounds when there are several.

**Cost, optional and collapsed.** Tokens by role and model, and an
API-price equivalent with a range: `references/cost.md`.

## 3. Publish

Pick the surface of the agent you are running in:

- **Claude** → a Claude Artifact. Load the artifact design guidance first
  and follow its page contract (title, theme tokens for light and dark,
  phone width, allowed CDNs only).
- **ChatGPT or Codex** → a Canvas document.
- **Other agents** → their rich-document surface, else the static page.
- **Static page** (with `--local`, or when there is no rich surface):
  `docs/pitch/<slug>.html`, self-contained, same design quality — palette
  and type chosen for the subject, light and dark, no external assets
  beyond a font stylesheet. Write the content also to
  `docs/pitch/<slug>.md`, so the next run updates instead of rebuilding.

Write in the audience's language and its terms; check grammatical gender
and local names of systems. Tell the user the page is private until they
share it.

## 4. Update

- `--stack "<change>"` appends to `docs/pitch/<slug>.pending.md` and
  changes nothing published.
- `--apply` applies everything pending in one publish, then clears the list.
- Before republishing, read the live page; if someone edited it (a new
  title, a reworded section), merge those edits into yours — never
  overwrite them.
