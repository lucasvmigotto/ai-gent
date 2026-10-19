# Filtering raw input before it becomes a product artifact

Shared by `project:init` and `frontend:uiux`: both turn a user-handed
idea, spec document or references folder into a committed artifact
(`brief.md`, `ux-vision.md`). The source document was written for a
human, not for this; it may carry personal remarks, unrelated tangents,
internal politics or other content that has nothing to do with the
product. None of that belongs in anything this toolkit writes.

## The three passes

1. **Segment** the input into discrete passages — one idea per piece,
   paragraph or sentence-group granularity, whichever keeps an idea
   whole. Do the same for every file in a references directory (read
   images with the Read tool as usual; this pass runs on what each file
   actually says, not just the inline text).
2. **Classify** each passage into exactly one bucket:
   - **Product signal** — directly states a business rule, workflow
     step, actor, data entity, constraint, goal, or technical/NFR
     detail. Carries through to the output, attributed to its source
     the way the brief already cites one for business rules.
   - **Indirect signal** — personal, anecdotal or tangential framing
     that *implies* a real requirement once the wrapper is removed (a
     complaint, a story, an aside that reveals a rule). Distill to the
     underlying fact only — the name, department, unrelated system,
     competitor, or emotional framing around it is never kept, and if
     nothing survives once that wrapper is gone, it's noise, not signal.
   - **Noise** — no product signal is recoverable even after stripping
     the wrapper: personal life, unrelated projects, internal politics,
     compensation, scheduling, or anything else with no bearing on what
     to build. Dropped entirely.
3. **Write only Product signal and distilled Indirect signal** into the
   output. Never write a dropped passage anywhere, not even to a scratch
   file — if the raw document needs re-reading later, it's still the
   source file itself, which stays uncommitted per `pipeline.md`.

## A worked example

> "My manager won't stop complaining that invoices sent through Outlook
> keep getting lost — he's been in a bad mood about it since the
> reorg."

- **Noise, dropped**: the manager, his mood, the reorg.
- **Indirect signal, distilled**: "Invoices are currently sent manually
  by email and get lost; the product needs a delivery or read
  confirmation." That sentence contains no person, department or
  unrelated event — it reads the same regardless of who said it.

## PII inside the example, not just the wrapper

A passage can carry a concrete value that looks like personal data — a
name, an email, a phone number, an id — used as a sample rather than as
the point of the sentence. Keep the underlying fact (the product stores
emails and phone numbers, say) as a normal NFR/data signal; the question
is whether the *value itself* is safe to keep alongside it.

- **Clearly a placeholder** — a well-known fake marker: `example.com`/
  `example.org`/`example.net` domains, `John`/`Jane Doe`, `Test User`,
  a `555-01XX` number, a sequential or obviously-fictional id, or the
  document itself says it's sample or test data. Safe to keep verbatim;
  it illustrates shape without risk.
- **Anything else** — a name, address, email, phone number or id that
  isn't a recognized placeholder is **treated as if it's real**, whether
  or not it actually is. Abstract it: drop the specific value, keep only
  its category (an email address, a phone number, a government id).
  When genuinely unsure which bucket a value falls in, abstract it —
  this rule is asymmetric on purpose; the cost of over-abstracting a
  value that turned out to be fake is nothing, the cost of keeping one
  that turned out to be real is a leak.

## The confirmation, every time

Before writing the output, show a short summary and ask for confirmation
— bundled into the same round as the skill's other clarifying questions
(pipeline rule 2), not a separate gate:

- how many passages were used as stated;
- each abstracted passage's **kept result** (the distilled sentence,
  so the user can confirm it's a fair reading) — never the wrapper that
  was stripped from it;
- each dropped passage, by **category label only** — "a scheduling
  note," "an internal politics remark," "an unrelated-product tangent"
  — never a quote, a name, or anything that would identify what was
  removed. The summary exists to be reviewed without reproducing the
  risk it exists to avoid.

Ask the user to confirm this reading or correct it before anything is
written. Pipeline rule 3 covers the rest: when a passage is genuinely
ambiguous between indirect signal and noise, this confirmation step is
the place to surface it — never guess silently either way.
