---
name: skill-docs
description: After adding, updating, renaming or removing a skill or plugin, bring ai-gent's own docs along — the docs-site reference pages, README, CHANGELOG and AGENTS.md — then run scripts/check.sh. Use whenever a skill or plugin changed.
---

# Skill docs sync

You just changed what ai-gent teaches. Its own documentation describes
those same skills, so finish the change by syncing it. Load this skill
whenever you add, update, rename or remove a skill or plugin — or change
a skill description or plugin manifest.

## Checklist

1. **Docs-site reference.** Skill reference pages are generated from the
   skill sources. Regenerate them and confirm the change appears; never
   hand-edit generated output to fake it. While the site has no generated
   reference yet, update `README.md`'s skill lists instead.
2. **README.md.** `Contents` names every plugin and skill — add, rename or
   drop the entry, and fix any counts. Usage examples that name the skill
   change too.
3. **CHANGELOG.md.** One entry under `## Unreleased` describing the
   skill-facing change; releases are built from it.
4. **AGENTS.md.** Only when conventions changed (new layout rules, new
   checks) — not for routine skill edits.
5. **Re-run `setup.sh`.** A new, renamed or removed skill changes what
   gets installed; start a new session afterwards to pick it up.
6. **Run `scripts/check.sh`.** It enforces the description budget (a
   single line, at most 400 characters), `name:` matching the directory,
   and that every `plugin:skill` and relative-path reference resolves.
   Fix what it reports; CI runs the same script.

## Notes

- A skill description loads in every session: keep it tight and say when
  the skill fires, or every future session pays for the words.
- A new skill usually also needs trigger evals under
  `plugins/<name>/evals/`; that is code, not docs, but say so in the
  handoff rather than silently skipping it.
