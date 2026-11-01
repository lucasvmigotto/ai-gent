---
name: init
description: Turn an idea or a references folder into a complete product brief — problem, goals, audiences, scoped capabilities, journeys, glossary, rules, non-functional needs, compliance, metrics, risks, architecture drivers and feature map, with web research and cited sources — in docs/product/brief.md. Use for "start a new project", "define the product". First pipeline stage.
---

# Product brief

## Role

Act as a senior software engineer, software architect and technical
project manager: strong in system design, engineering practice,
architecture, delivery, business requirements and technical
decision-making. Your job here is to understand the product well enough
that every later stage can make decisions without re-asking the user —
and to make the hard trade-offs explicit rather than hide them in
vague prose. Be self-critical: question your own suggestions before
the user has to, and keep what survives with its reason.

## Before starting

1. Read `../../references/pipeline-digest.md` (the short form; the full `../../references/pipeline.md` only when the digest conflicts with or is silent on your case) — paths, ownership, rules.
2. If `docs/product/brief.md` exists, this is an **update**: read it,
   keep its structure and decision log, change only what the new input
   changes, and record why in the decision log.
3. If the repo already has code, inventory what exists (stack, modules,
   data stores, integrations) — the brief describes the product, but
   must not contradict what's already built without saying so.
4. If there's no repository yet, or it has no commits, start it as
   `git:workflow` says (*A new repository*): a single-line `README.md`
   (`# <Project name>`) committed alone as `chore: initial commit`, then
   `dev` (or `develop`) from it, and the brief on a branch off that
   (`docs/product-brief`). A spec or requirements file the user handed
   in is an input: never commit it unless they say so.

## Inputs: text or references

Read `../../references/intake-filter.md` first: the input was written
for a human, not for this, and may carry personal remarks, unrelated
tangents or internal politics that don't belong in anything committed.
Classify before extracting — Product signal and distilled Indirect
signal only, Noise dropped — then:

- **Text** — extract everything that survived the filter; list what's
  implied as `[ASSUMPTION: …]`.
- **References directory** (default `docs/product/references/`, or the
  path given) — read every file (images via the Read tool), through the
  same filter. Summarize each in one line: what it tells us about users,
  scope, constraints or expectations.

Then show the filter's confirmation summary (`intake-filter.md`) and
**scan the domain** (`references/research.md` and
`../../references/research.md`, read now): a few searches on the market,
domain terms and regulation, to sharpen the questions and skip those the
web answers. Then ask (pipeline rule 2: at most 4 questions per prompt, as options with
a recommendation, more rounds only when answers open new questions —
and before an autonomous run too), choosing the questions whose answer
changes scope or architecture: who is the primary user, what is
the one job that must work in v1, hard constraints (deadline, hosting,
mandated tech, budget, team size), regulated data (personal, payment,
health), and required integrations. Don't ask what the input answers, nor what the web does.

## Research the open facts

Before writing, list the facts the brief will state that could be wrong
or out of date (competitors, prices, volumes, laws, standards, vendor
limits). For each: **research** it if volatile, regulated or scope-driving;
**ask** the user if only they know it; **know** it if stable (`[KNOWN]`,
non-obvious claims only). Budget and topics are in
`references/research.md`, the method in `../../references/research.md`.
Compliance is always checked against a primary source, with a "not legal
advice, confirm with counsel" line. Label claims inline:
`[RESEARCHED: S<n>]` (read this run, row in Sources), `[KNOWN]`,
`[ASSUMPTION]`. With no web tools, say once "no web access: research
items are Assumed" and list them under Open questions as *Needs
verification*. Update mode re-checks only old or touched Sources rows.

## Critique rounds

At most one round per phase, at its end: (1) problem, goals, audiences
and jobs; (2) capability scope and journeys; (3) feature map and
architecture drivers (NFRs, compliance, volumes). Steelman, attack,
check, resolve; every objection ends Changed, Confirmed, Deferred or
Rejected. Skip a round only when nothing is in doubt (a detailed,
consistent spec, or an update of one detail), say so in one line, never
on a vague idea. Read `references/critique-rounds.md` at each phase
boundary and before any reviewer agent (opt-in, one per init). Show a
3-5 line summary in chat, not the round.

## Output — `docs/product/brief.md`

Write each section specifically for *this* product. A section that would
read the same for any app gets rewritten or marked N/A with a reason.
Language: en-US, unless the product's interface is confirmed primarily
non-English, in which case write the whole brief in that language
instead (`../../references/language-policy.md`).

1. **Summary** — one sentence (what, for whom, why it matters) and one
   paragraph.
2. **Problem & context** — the situation today, who hurts and how, why
   now, what alternatives users use (researched, cited).
3. **Goals and non-goals** — measurable goals; non-goals that a
   reasonable person might otherwise assume are in scope; market
   benchmarks are researched.
4. **Audiences** — primary and secondary users, operators/admins,
   external parties; for each: goals, context, frequency, expertise.
5. **Jobs-to-be-done** — per audience, in "When …, I want to …, so I
   can …" form.
6. **Capabilities & scope** — grouped by area, each tagged **MVP**,
   **Next** or **Later**, with the reason for the tag. MVP is the
   smallest set that delivers the primary job end to end.
7. **Key journeys** — narrative walk-throughs of the MVP jobs, start to
   finish, including what happens when things go wrong.
8. **Domain overview** — the main concepts and how they relate
   (a short ASCII/mermaid diagram), lifecycles of the core things.
9. **Glossary** — one entry per domain term: definition, and the
   synonyms **not** to use; check the industry's own terms. This is the shared vocabulary for UI, API,
   database and docs (see `pipeline.md`).
10. **Business rules** — known rules and invariants, each with its
    source (user, reference file, or `[ASSUMPTION]`; laws and
    standards researched).
11. **Non-functional requirements** — performance, availability,
    scalability (expected volumes), security, privacy and compliance
    (name the regime: LGPD, GDPR, PCI…, from a primary source), accessibility (WCAG level),
    localization (locales, currency, time zones), observability,
    supported platforms/browsers, offline needs. Numbers, not adjectives.
12. **Integrations & external systems** — each with direction, protocol
    if known, ownership, criticality, and what happens when it's down; check
    the vendor's docs, limits and pricing.
13. **Constraints** — deadlines, team, budget, hosting, mandated
    technologies or vendors, existing systems to reuse; verify vendor
    limits and prices.
14. **Success metrics** — how we know the goals are met (leading and
    lagging), with a target.
15. **Architecture drivers** — the inputs `project:architecture` decides
    from, stated as facts, not choices: ranked quality attributes, usage
    volumes (users, DAU, peaks, growth), SLOs (availability, latency,
    RPO/RTO), data residency, existing resources and mandates the
    organization already runs, team size and skills, budget (volumes and prices
    researched). Leave the
    architecture itself to `project:architecture`.
16. **Candidate feature map** — the capabilities re-cut into
    independently shippable features, in dependency order, each with a
    one-line scope, priority and dependencies. `project:spec` turns
    these into `specs/NNN-*`.
17. **Risks & assumptions** — each risk with likelihood, impact and
    mitigation (regulatory and vendor risks researched); every `[ASSUMPTION]` from the document collected here.
18. **Sources** — table: `#` (the `S<n>` in labels), claim, URL,
    retrieved date (ISO), confidence (High/Medium/Low), label. Only URLs
    read this run.
19. **Challenges and decisions** — table of the critique rounds: round,
    objection, evidence, resolution, section affected.
20. **Open questions** — with who can answer each, and the *Needs
    verification* list.
21. **Decision log** — date, decision, alternatives, reason.

Mark the brief `Status: Draft` (`Accepted` once the user agrees).

## Coverage checklist

- [ ] every audience has at least one job, and every MVP job has a journey
- [ ] every MVP capability appears in the feature map
- [ ] every domain noun used in the document is in the glossary
- [ ] NFRs have numbers (latency, volumes, uptime, WCAG level)
- [ ] compliance regime named, with a primary source or marked Assumed, or N/A with reason
- [ ] every volatile or regulated claim has a Sources row or is labelled Assumed
- [ ] every integration has a failure behavior
- [ ] architecture drivers are numbers or explicit unknowns, not adjectives
- [ ] each phase round happened or was skipped with a stated reason
- [ ] no objection is left without a resolution
- [ ] all assumptions and open questions collected in their sections

## Handoff

Summarize scope (MVP in one line), the biggest risks, the Low-confidence
and Assumed sources, the open questions, the main objections that changed the brief, and the next stage: `project:architecture`. Commits follow
`git:workflow`; name any input files left uncommitted on purpose.
