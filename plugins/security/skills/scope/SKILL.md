---
name: scope
description: Record the authorization, targets, time window and allowed techniques before any offensive security work runs — the project's own local environment by default, a remote or shared target only with the user's explicit, documented authorization. Use for starting a penetration test, a red team engagement, or any security:red or security:blue work.
---

# Security scope and authorization

Read `../../references/principles.md` first: it sets the rule this skill
exists to enforce. `security:red`, `security:blue` and `security:purple`
all refuse to start without a current record from here — read it fresh
each time, don't assume an old session's record still applies.

## What a scope record contains

- **Targets** — hostnames, IPs, repositories or cloud accounts, each
  marked `local` or `remote`.
- **Authorization** — for `local` targets, none needed beyond the user's
  own request (their sandbox, nothing leaves it). For `remote` or shared
  targets, a specific, checkable basis quoted or linked verbatim: a
  signed pentest agreement, a bug bounty program's published scope, or
  the user's written statement that they own the asset and accept
  responsibility for the test.
- **Window** — start and end dates. No end date is a bug; ask for one.
- **Allowed techniques and explicit exclusions** (e.g. no denial of
  service, no social engineering, no physical access).
- **Stop condition** — what "stop immediately" looks like for this
  engagement (any sign of real user data, any production impact), and
  who to tell if it happens.

## Procedure

1. **Ask** (at most 4 questions per round, pipeline rule 2): what's being
   tested, is it this project's own local stack or something else, and
   if something else, what the documented authorization is.
2. **Local only** — write the record without further questions: the
   devcontainer or compose stack already running for this project,
   reached the way `devcontainer:workflow` reaches it.
3. **Remote or shared named** — do not authorize `security:red` until
   the authorization is quoted or linked and a window is given. "Yes
   it's authorized" alone is not a basis; ask for the document, the
   program URL, or a specific written statement, and record it verbatim.
4. **Write** `docs/product/security/scope-<date>.md` with the fields
   above.
5. **Hand off** — name the record's path and which of `security:red`,
   `security:blue`, `security:purple` it authorizes.

## Refuse, don't negotiate

A request to skip this step, to test "just to see," or to target
something the user can't document authorization for, is refused: say
why, and offer the local-sandbox alternative instead.
