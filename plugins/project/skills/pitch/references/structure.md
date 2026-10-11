# Pitch structure

## By size

| Section | short | medium | large |
|---|---|---|---|
| Hero: name, one-sentence thesis, 3–5 sourced figures | ✓ | ✓ | ✓ |
| Problem and why now | 2 lines | ✓ | ✓ |
| The solution / components (side by side when there are two products) | 2 lines | ✓ | ✓ |
| How it works: one flow or diagram | — | ✓ | ✓ |
| Security, guardrails, traceability | — | ✓ | ✓ |
| Delivery phases (e.g. Alpha / Beta) with exit criteria | — | ✓ | ✓ |
| Current state: phase table with status pills | 1 line | ✓ | ✓ |
| Validation: results, how to read them, a few concrete cases | 1 figure | ✓ | ✓ |
| Expected gains, labelled as targets until measured | — | ✓ | ✓ |
| Risks and mitigations | — | ✓ | ✓ |
| Method: how it was built (extraction, review, pipeline) | — | — | ✓ |
| Quickstart: prerequisites and install steps | — | — | ✓ |
| Day-to-day use: "I want / I type / what happens", an illustrative conversation marked as such | — | — | ✓ |
| AI effort and cost, collapsed (`cost.md`) | — | — | ✓ |
| Next step and the ask | ✓ | ✓ | ✓ |

## By audience

- **non-tech**: outcomes, decisions, risks, cost; no code blocks, no table
  or file names, no jargon (say "the tool reads the ticket", not "the MCP
  server calls `get_ticket`").
- **basic-tech**: a diagram, short command snippets, each guardrail in one
  line, names of components the reader will meet.
- **tech-team**: commands, contracts, test counts, pipelines, branches and
  releases, known limits, open technical decisions.

## Design

- A visual identity from the subject (its domain, its documents, its terms),
  not a template; one accent, neutrals with a slight hue, a display face used
  with restraint, a readable body face.
- Numbered markers only for real sequences (phases, steps).
- Tables for comparisons, status pills for state, a callout for "how to read
  these numbers".
- Details that most readers skip (cost, token tables) behind `<details>`,
  closed by default.
- Real content only: product names, counts, dates. Examples that are not
  real ("illustrative conversation") say so.
