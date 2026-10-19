# Language policy

Shared across the pipeline: what language each written artifact uses,
decided once here instead of guessed per skill.

## Always English, no exception

Code, comments, identifiers, commit messages, PR descriptions, and log
and error messages — anything a machine or another engineer reads —
stay English regardless of the product's audience, the UI's language,
or the language the developer is using in this conversation. This is a
fixed engineering convention, not a localization choice: whoever
operates or maintains the system next, in any country, reaches it
through English.

## Interface text — inferred, then confirmed

The UI's own language is a product decision, not an engineering one.
Infer it from what's actually given: sample copy already written in a
language, reference files, existing screenshots or docs
(`intake-filter.md` — the passages that pass its filter are what this
draws from). Confirm the inferred language with the user before
`frontend:build` writes it into a locale file — one line in the same
clarifying-questions round `frontend:uiux` and the build stages already
run (pipeline rule 2), not a separate gate.

With nothing to infer from, and the developer hasn't settled it either,
default to English and ask once rather than guess a locale.

**The developer's own language in this conversation decides it only
when nothing else does** — the fallback for breaking a tie, never a
reason to override a clearer signal from the product's own references,
and never a reason to write code, comments or logs in anything but
English.

`frontend:build` keeps every UI string in a locale file by key, never
hardcoded — this is what makes the UI's language a pure data question,
decided here, not a code change.

## Documentation — mirrors the UI's primary language

`README.md`, `docs/product/brief.md`, `architecture.md` and the rest of
the pipeline's own documents, `specs/NNN-*/*.md`, and the docs site
(`project:docs`) default to **en-US**. When the product's interface is
primarily a language other than English, write all of these entirely in
that language instead — not a mix, and not only the user-facing pages.
Example: a UI primarily in pt-BR means the brief, the architecture
record, every spec and the docs site are written in pt-BR throughout.

This only moves once the UI's primary language is actually confirmed
(per the section above) — until then, write in English and revisit once
it's settled. The interface's language is usually confirmed later than
`project:init` (at `frontend:uiux`), so the brief, the architecture
record and early specs are routinely written in English first by
default. If confirmation later lands on a non-English language, that's
a normal upstream gap (the pipeline's own ownership rules) — flag it and
ask whether to translate what's already written, rather than leaving a
silent mismatch or translating on your own initiative.

## What doesn't change

Code, comments, commit messages and logs stay English even when the
product's documentation and UI are in another language — the two
decisions are independent; switching one never drags the other with it.
