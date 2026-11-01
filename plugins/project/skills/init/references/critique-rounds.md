# Critique rounds

Read this at each phase boundary and before any reviewer agent. A round
questions the brief's own suggestions so the user does not have to find
the holes later. It is short: about 1-3k tokens inline.

## When to run, when to skip

- At most one round per phase, right after it: (1) problem and goals,
  with audiences and jobs; (2) capability scope, with journeys;
  (3) feature map and architecture drivers, with NFRs, compliance and
  volumes.
- A second round in a phase only on high-risk topics: regulated data,
  payments, safety, hard vendor dependence, or a number that drives cost
  by 10x.
- Skip when nothing is in doubt: a detailed, consistent spec, or an
  update that changes one detail. State it in one line ("Round 2
  skipped: spec is detailed and consistent"). Never skip on a vague idea.

## The round

1. **Steelman** the current position in two sentences: the strongest
   honest case for it.
2. **Attack** with only the lenses that fit the phase (below). At most 5
   objections, ranked by impact.
3. **Check** with evidence where cheap: use the research method in
   `../../../references/research.md` within its soft budget
   (`research.md`). Ask the user only what only they can settle, folded
   into the existing prompt of at most 4 questions, never a separate
   interrogation.
4. **Resolve** each objection:
   - **Changed**: the brief was edited.
   - **Confirmed**: kept, with the reason and the evidence.
   - **Deferred**: an open question or risk, with an owner.
   - **Rejected**: not valid, with why.

Do not show the transcript in chat. Say 3-5 lines: "Challenged X,
changed Y, confirmed Z."

## Lenses by phase

| Phase | Lenses |
|---|---|
| 1 Problem, goals, audiences, jobs | Pre-mortem (why would this fail in a year); assumptions ("how could this be false, and what would we see"); alternatives users already have; missing audiences or stakeholders; "what would make us wrong" |
| 2 Scope, journeys | Scope creep in MVP; alternatives not chosen and why; failure paths missing from journeys; one job that must work end to end; stakeholders who veto |
| 3 Feature map, drivers | Contradictions across sections (glossary vs rules vs NFRs vs feature map); numbers that drive cost by 10x; regulatory exposure; hard vendor dependence; dependency order; unknown volumes |

## Recording

Section **Challenges and decisions** (just before Open questions):

| Round | Objection | Evidence | Resolution | Section |
|---|---|---|---|---|
| 1 Problem | Clinics may already keep records in WhatsApp; why switch? | S3 (survey of 120 clinics) | Changed | 2 Problem and context |
| 3 Drivers | 8,000 patients may be 10x higher if clinics add chains | Reasoning: pricing is per clinic, growth is by clinic | Deferred (owner: product, before architecture) | 15 Architecture drivers |

A material choice also gets one line in the Decision log (decision,
alternatives, reason); do not repeat the objection text there.

## Anti-theatre tests

- "No issues found" is not an outcome. A round changes or confirms
  something concrete, with a stated reason.
- Each row has an objection, evidence (a source number or stated
  reasoning) and one of the four resolutions.
- Confirmed needs a reason that would not fit any other brief.
- More than 5 objections means ranking was skipped; cut to 5.

## Reviewer agent (opt-in)

- Only on request ("rigorous", "red team") or for high-risk products
  (regulated data, payments). One run per init.
- A fresh-context agent given only the draft brief, the Sources table
  and this checklist, never your conclusions. It returns a ranked
  objection list with evidence. Resolve each as above.
- Cost: about 10-25k tokens, against 1-3k for an inline round. Say so
  before running it.
- No sub-agent facility: do a "fresh read" instead. Re-read the draft
  top to bottom, as if someone else wrote it, before writing the attack.
