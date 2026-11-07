# Asking before writing

How a pipeline stage asks the questions `pipeline.md` rule 2 requires
before it writes — the format, and which stages ask even when the user
has given permission to run the project on its own.

## Options first

Give each question 2–4 concrete choices, the recommended one first with
its reason in a line (Claude Code's question tool takes four questions
at once; elsewhere a numbered list answered by number). Free text only
when the choices can't be listed.

## Rounds

Ask in rounds, not one long list, so each round can use what the last one
settled.

1. **Plan the questions first.** List what could change the output and drop
   whatever the inputs already answer. Order the rest by how much of the
   output hangs on the answer; the structural ones go first.
2. **One round: at most four questions** (the format above).
3. **Recap in a few lines**, in the user's words, as decisions rather than
   as a transcript: `Decided: <choice> — <reason>`. Give the user the chance
   to correct a line before it is built on.
4. **Re-plan the rest.** Strike each pending question the answers already
   settle and name it (`No longer needed: <question> — answered by <choice>`),
   so a skipped question is visible rather than silently lost. Add a new
   question only when an answer opened a fork. When an answer contradicts an
   earlier one, ask which one holds instead of picking.
5. **Stop** when nothing that would change the output is open, or when the
   user says "you decide": record each remaining choice as
   `[DECIDED: … — delegated by the user]` with its default and reason, as
   `pipeline.md` rule 2 says.
6. **List every decision in the handoff**, so any of them can be revisited.

A question the user answered once is not asked again in a later round or a
later stage: the recap is what the next stage reads first.

## Autonomy doesn't answer questions

Permission to run the project on its own covers doing the work, not
deciding the product. The stages that shape it — `project:init`,
`project:architecture`, `frontend:uiux`, `backend:domain`, `project:spec`
(its clarify step), `qa:strategy` — ask their rounds before they write,
also under that permission; once the specs are clarified, the build
stages run without new questions, and a gap they find becomes an
`[UPSTREAM GAP: …]` note instead. Delegated decisions are listed in the
handoff, so the user can revisit any of them.
