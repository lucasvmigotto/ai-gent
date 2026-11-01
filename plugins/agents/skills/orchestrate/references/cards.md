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

Containers: only through the project's recipes, with AI_GENT_WORKTREE=<slug>;
no image tags or named volumes of your own, `run --rm` for anything else, no
prune and no rm of anything you didn't start. Report every image, volume or
container you created.

Reading and checking: read files with Read offset/limit or `sed -n 'a,bp' file | head -80`,
never `cat` a file over ~150 lines; `grep -c` or `| head` before dumping matches.
Batch shell steps into one call. Inside your loop run a scoped test (or
`check.sh --quick --quiet`); run the full check once, before the commit. Keep
to about 40 tool calls; if the card needs more, stop and report what is left.

Acceptance: <commands that must pass, e.g. the test suite and the linters>.

Final report (concise): what changed per item, files, test results, anything
you could not verify and why. No file contents.
```

## The card in `cards.md`

The plan keeps one section per card in `specs/NNN-*/cards.md`: a summary
table, then for each card its route and the prompt above.

```markdown
## Card `004-foundation`: T001, T002

**Route**: Executor `builder` · Reviewer `thinker` · Effort `high` · Tags: extend, backend, protocol, rust, tokio, known-pattern, integration, M · Tokens: projected ~420k | used — · Dispatch description: `T001, T002: 004-daemon foundation`
```

- **`Tags:` and `Tokens: projected` are required** on every card
  (`scripts/consistency.py` fails a `cards.md` without them, or with other
  than one value per facet). Tags come from the controlled list in
  `../../../references/token-tags.md`; project at planning with
  `$TOKREP estimate --role builder --effort high --tags "<the card's tags>"`
  (`../../../references/token-tracking.md`). `$TOKREP tasks` later fills
  `used` and copies tags, projection and result onto the card's first task
  line in `tasks.md`.

## Notes

- **Name the task id(s) and the card id or feature number first in the
  dispatch's own short description** (not the card body) —
  `T012, T013: 005-routing-vocabulary core`, not just `routing vocabulary`
  (task ids repeat across features). `token_report.py tasks` attributes a card's cost
  from that description (a card id, or task ids plus the feature number);
  without them the cost is reported as unattributed, never guessed.
- The dispatch call's `description` is the card's `Dispatch description:`
  and starts with the card id (`004-foundation: T001, T002 …`): the token
  report attributes by it and never by branch name or a free title.
- Prefer one scoped, self-contained card on a general-purpose agent over a
  fork: a fork starts at about 78k of context, a fresh subagent at about 40k.
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
