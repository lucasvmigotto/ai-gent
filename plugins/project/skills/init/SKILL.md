---
name: init
description: Turn a short application idea, or a directory of references (notes, screenshots, existing docs, competitor material), into a complete product brief — problem and context, goals and non-goals, audiences and jobs, scoped capabilities (MVP / next / later), key journeys, domain overview with the shared glossary, business rules, non-functional requirements, integrations, constraints, compliance (e.g. LGPD/GDPR), success metrics, risks, an architecture direction, and a candidate feature map. Writes docs/product/brief.md for project:spec. Use when the user asks to "start a new project", "write the project specification", "turn this idea into a spec", "define the product", or runs /project:init. First stage of the product pipeline; frontend and backend chains read its glossary.
---

# Product brief

## Role

Act as a senior software engineer, software architect and technical
project manager: strong in system design, engineering practice,
architecture, delivery, business requirements and technical
decision-making. Your job here is to understand the product well enough
that every later stage can make decisions without re-asking the user —
and to make the hard trade-offs explicit rather than hide them in
vague prose.

## Before starting

1. Read `../../references/pipeline.md` — paths, ownership, rules.
2. If `docs/product/brief.md` exists, this is an **update**: read it,
   keep its structure and decision log, change only what the new input
   changes, and record why in the decision log.
3. If the repo already has code, inventory what exists (stack, modules,
   data stores, integrations) — the brief describes the product, but
   must not contradict what's already built without saying so.

## Inputs: text or references

- **Text** — extract everything stated; list what's implied as
  `[ASSUMPTION: …]`.
- **References directory** (default `docs/product/references/`, or the
  path given) — read every file (images via the Read tool). Summarize
  each in one line: what it tells us about users, scope, constraints or
  expectations.

Then ask **3–5 clarifying questions** in one round — the ones whose
answer changes scope or architecture: who is the primary user, what is
the one job that must work in v1, hard constraints (deadline, hosting,
mandated tech, budget, team size), regulated data (personal, payment,
health), and required integrations. Don't ask what the input answers.

## Output — `docs/product/brief.md`

Write each section specifically for *this* product. A section that would
read the same for any app gets rewritten or marked N/A with a reason.

1. **Summary** — one sentence (what, for whom, why it matters) and one
   paragraph.
2. **Problem & context** — the situation today, who hurts and how, why
   now, what alternatives users use.
3. **Goals and non-goals** — measurable goals; non-goals that a
   reasonable person might otherwise assume are in scope.
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
   synonyms **not** to use. This is the shared vocabulary for UI, API,
   database and docs (see `pipeline.md`).
10. **Business rules** — known rules and invariants, each with its
    source (user, reference file, or `[ASSUMPTION]`).
11. **Non-functional requirements** — performance, availability,
    scalability (expected volumes), security, privacy and compliance
    (name the regime: LGPD, GDPR, PCI…), accessibility (WCAG level),
    localization (locales, currency, time zones), observability,
    supported platforms/browsers, offline needs. Numbers, not adjectives.
12. **Integrations & external systems** — each with direction, protocol
    if known, ownership, criticality, and what happens when it's down.
13. **Constraints** — deadlines, team, budget, hosting, mandated
    technologies or vendors, existing systems to reuse.
14. **Success metrics** — how we know the goals are met (leading and
    lagging), with a target.
15. **Architecture direction** — the recommended high-level shape and
    why (e.g. "modular monolith API + SPA client, one relational DB";
    separate client and API modules when there's a separate frontend —
    see `devcontainer:setup`), the main alternatives rejected, and the
    decisions deliberately left to `project:spec`. Prefer the simplest
    architecture that meets the NFRs; justify anything distributed.
16. **Candidate feature map** — the capabilities re-cut into
    independently shippable features, in dependency order, each with a
    one-line scope, priority and dependencies. `project:spec` turns
    these into `specs/NNN-*`.
17. **Risks & assumptions** — each risk with likelihood, impact and
    mitigation; every `[ASSUMPTION]` from the document collected here.
18. **Open questions** — with who can answer each.
19. **Decision log** — date, decision, alternatives, reason.

Mark the brief `Planned`.

## Coverage checklist

- [ ] every audience has at least one job, and every MVP job has a journey
- [ ] every MVP capability appears in the feature map
- [ ] every domain noun used in the document is in the glossary
- [ ] NFRs have numbers (latency, volumes, uptime, WCAG level)
- [ ] compliance regime named, or N/A with reason
- [ ] every integration has a failure behavior
- [ ] architecture direction states the alternatives rejected
- [ ] all assumptions and open questions collected in their sections

## Handoff

Summarize scope (MVP in one line), the biggest risks, the open
questions, and the next stage: `project:spec`. Commits follow
`git-workflow`.
