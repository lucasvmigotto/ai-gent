---
name: red
description: Simulate an attacker against a scoped target — application (authz, injection, OWASP Top 10), infrastructure (cloud and IaC misconfigurations) and AI/LLM systems (prompt injection, agent abuse) — with a threat model and evidenced, severity-ranked findings with fixes. Use for a penetration test, a red team exercise, "think like an attacker".
---

# Red team security testing

## Before anything — scope

Read `security:scope`'s latest `docs/product/security/scope-*.md`, or
write one now through that skill if none exists. Refuse to touch
anything the record doesn't name, and refuse any technique it excludes.
`../../references/principles.md` has what's off the table always — no
denial of service, no real malware, no third parties, no live-bypassing
a production control outside a `security:purple` exercise both sides
agreed to.

## 1. Threat model

From the architecture (`docs/product/architecture.md` if it exists, else
read the code) and the scope record: the assets worth attacking, trust
boundaries, entry points, and the attacker profiles that actually apply
(anonymous internet user, authenticated low-privilege user, insider,
compromised dependency). A simple asset-to-threat table is enough for
most systems; use STRIDE when the system is large enough to need it.

## 2. Pick the domain(s) in scope

A system usually spans more than one — run each relevant checklist:

- **Application** → `../../references/app-security.md`
- **Infrastructure** → `../../references/infra-security.md`
- **AI/LLM** (only if the app has an LLM, agent or retrieval component)
  → `../../references/ai-security.md`

## 3. Attack plan before action

Write the planned attack paths (what, why, the evidence expected) before
running anything that changes state — this is what the user confirmed
when they wrote the scope. Read-only reconnaissance and passive checks
need no extra confirmation; anything that could change state,
authenticate as someone else, or touch real data gets a one-line
heads-up in the handoff even inside an approved scope.

## 4. Findings

Same table shape as `devsecops:audit`:

| Field | Content |
|---|---|
| Severity | Critical · High · Medium · Low · Info (exploitability × impact) |
| Location | file:line, endpoint, or the setting's path |
| Evidence | the exact request/response, snippet or setting value |
| Scenario | how it's exploited, concretely |
| Fix | the corrected code, config or control |
| Effort | S · M · L |

Order by severity then effort; group duplicates (one finding, many
locations); mark anything you couldn't verify as **Unverified** with
what to check.

## 5. Output

`docs/product/security/red-<date>.md`: scope reference, threat model,
what was tested, the findings table, and what's out of scope and why (so
the next round of testing knows where the gap is).

## Handoff

The riskiest finding in one line, what needs the user's decision
(credentials, systems you couldn't reach), and offer `security:blue` to
design detections for what was found, or `security:purple` to validate
the fixes once they land.
