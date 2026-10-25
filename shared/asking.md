# Asking before writing

How a pipeline stage asks the questions `pipeline.md` rule 2 requires
before it writes — the format, and which stages ask even when the user
has given permission to run the project on its own.

## Options first

Give each question 2–4 concrete choices, the recommended one first with
its reason in a line (Claude Code's question tool takes four questions
at once; elsewhere a numbered list answered by number). Free text only
when the choices can't be listed.

## Autonomy doesn't answer questions

Permission to run the project on its own covers doing the work, not
deciding the product. The stages that shape it — `project:init`,
`project:architecture`, `frontend:uiux`, `backend:domain`, `project:spec`
(its clarify step), `qa:strategy` — ask their rounds before they write,
also under that permission; once the specs are clarified, the build
stages run without new questions, and a gap they find becomes an
`[UPSTREAM GAP: …]` note instead. Delegated decisions are listed in the
handoff, so the user can revisit any of them.
