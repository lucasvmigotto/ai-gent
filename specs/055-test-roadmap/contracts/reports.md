# Contract — reports and messages

Outputs the agent writes to the owner (in the terminal) or into the session folder, and drafts for the team (the owner pastes them). Team messages use lists, never tables, and no code or file names. Language: the owner's (`shared/language-policy.md`: the developer's language decides only when nothing else does; ask once). Requirements: spec FR-072 to FR-077, FR-092, FR-093, FR-095.

## Preflight record (`preflight.md`, and shown to the owner)

1. **Checklist** — one line per validator: label, assigned role, sharing level needed (the level that grants `interact` on the register), the caveat if any (outside the organization; link sharing removes their write; may not upload attachments; Editors can republish and change rules), the exact change to make in the sharing menu, and the owner's confirmation (`confirmed` / `pending`).
2. **Test read** — per collection and level (`view`, `interact`, `admin`): expected, seen, pass or mismatch. Any mismatch is a blocker for the invitation.
3. **Declaration check** — the rules published compared with `data-model.md` (`ai-gent uat preflight verify`).
4. **What this proves** — the register's rules, as enforced for reading, and the declared write rules matching the contract.
5. **What this does not prove** — the write side as enforced, and each person's sharing level; both are proven per person only by their R0 write check.
6. **Write checks** — validators whose R0 write-check mark has appeared, and those still missing (updated by every status report).

## Status report (to the owner)

Every report starts with `Last real check: <date hh:mm>` (the snapshot's time). When periodic follow-up is on, it adds `Next check: <hh:mm>` or `Timer expired on <date>; checks are on request now`.

No change since the previous snapshot — exactly one line:

```
No news since <hh:mm> (last real check <hh:mm>). Next check <hh:mm> | on request.
```

Otherwise, in this order, each part omitted when empty:

1. Progress: per script, `R<N> <title>: <ok>/<steps> OK, <nok> NOK, <unmarked> unmarked, <stale> stale`.
2. New NOKs: per NOK — script and step, who (display label), time, the observation quoted, class, probable cause in plain words, confidence, suggested reply.
3. New improvements: script and step, who, text quoted.
4. Role proposals pending: who, assigned role, proposed role, note quoted.
5. Coverage gaps: steps marked only by a role other than their target; steps whose only marks came from people who left.
6. Write checks missing: validators with no R0 write check yet (and a reminder to re-check their sharing).
7. Stale marks: steps rewritten after marks were made.
8. For the owner to decide: change requests and open questions (the decision list delta).

## Follow-up setup message (to the owner, only when turning periodic follow-up on)

The interval; that it runs only while this session is open and idle; that it expires after 7 days; that it notifies nobody (the owner reads the reports here); how to turn it off. Turning it off, or closing, removes it.

## Triage entry (`triage.md`)

`<step> · <validator label> · <at>` then class, cause, evidence the agent read (labelled, paths allowed here), suggested reply, status.

## Team messages (drafts)

- **Invitation**: what the session is, the link, the environment, the dates, who executes which scripts, how to mark (OK; NOK with what to write; improvement for "works, but should change"), the exclusivity and wait rules, the R0 write check and role confirmation, who to ask about access, a reminder not to paste passwords, tokens, cookies or personal data.
- **Status**: one line of overall progress, then a list per script with progress, then "what we found" (NOKs in plain words, without blaming), then "what we need from you".
- **Reply to a NOK**: thanks, what we found in plain words, what happens next (fix in test, a decision, a corrected step), and, for a change request, that it is recorded for decision and that "works as designed but should change" is best written as an improvement next time.
- **Closing**: totals, coverage by role, what was fixed, what goes to decision, known limitations, thanks.

## Consolidated report (`.ai-gent/uat/<slug>/report.md`, `ai-gent uat report`)

Local only, never committed by the skill (Decision D2). Markdown with: header (roadmap version, environment, dates, validators by assigned role, last real check), per-script table of results (tables are fine here: it is a file, not a chat message), coverage by role, NOKs by class with status, improvements, role proposals and their decisions, the decision list, steps not covered by their role, steps marked only by people who left, stale marks and retired steps, the preflight record summary, the review record, the changelog, and in mode B the scenario-to-step traceability table.
