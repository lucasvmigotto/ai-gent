# Cards

A card is the whole prompt a builder gets. It must stand alone: the builder
has none of your context.

## Template

```markdown
Work ONLY in <worktree path> (branch <name>, off <dev branch>; do not switch
branches). Conventional Commits on this branch; no commit trailers; never
--no-verify; do not merge or push.

Goal: <two or three lines>.

Inputs: <exact paths to read first: spec, card, contracts, files to imitate>.
May change: <paths>. Must not touch: <paths other agents own>.

Do:
1. <step>
2. <step>

Never: <paid calls, production, other branches, secrets, personal data>.

Acceptance: <commands that must pass, e.g. the test suite and the linters>.

Final report (concise): what changed per item, files, test results, anything
you could not verify and why. No file contents.
```

## Notes

- **Name the task id(s) first in the dispatch's own short description**
  (not the card body) — `T012, T013: routing vocabulary`, not just
  `routing vocabulary`. `token_report.py tasks` links a card's cost back
  to `tasks.md` by that leading id; without it the card's cost lands in
  the roll-up's Unmapped section instead of its feature's `tokens.md`.
- Name the tests to write first when the work is code ("tests first; show
  them failing").
- For shared checkouts, say "do not commit; stage nothing" and commit
  yourself after review.
- For follow-ups, message the same builder with only the delta; it keeps its
  context.
- For a resumed card after a limit: "Resume: you stopped on a session
  limit. Check what you already did (git status, files under <path>) and
  finish as specified."
- Keep the role preamble identical across cards of the same role (it is
  cached); put the card-specific part after it.
