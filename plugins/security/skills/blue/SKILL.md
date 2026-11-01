---
name: blue
description: Design detection and defense for a scoped system or for security:red's findings — logging and alert coverage mapped to MITRE ATT&CK and ATLAS, hardening baselines, guardrails for AI/LLM agents and tool calls, and incident runbooks — so the same attack gets caught, not just fixed once. Use for "set up detections," "harden this," or "what would catch this attack."
---

# Blue team detection and defense

## Inputs

- A `security:red` report (`docs/product/security/red-*.md`) to build
  detections for its findings, or
- a system with no red report yet — still useful: harden and instrument
  from the domain checklists directly, without needing an attack first.

Check `security:scope`'s record for the target the same way
`security:red` does: blue work on a remote or shared system still needs
it, since adding logging, alerts or config there is a change to that
system. `../../references/principles.md` has the rest of what's off the
table always.

## 1. Coverage, not just a fix

For each threat — from the red report, or from the domain checklists
below — ask whether today's logging and alerting would actually **catch**
this happening again, not only whether the one instance got patched.

- **Application** and **infrastructure** → map to MITRE ATT&CK tactics
  and techniques. **AI/LLM** → MITRE ATLAS tactics (prompt injection,
  model exfiltration, tool misuse). `../../references/detection-engineering.md`
  has the process, log sources and a runbook template.
- Domain-specific guardrail and baseline patterns (input/output
  filtering, tool-call allowlists, rate limits on agent actions, canary
  prompts) are in `../../references/ai-security.md`,
  `app-security.md` and `infra-security.md`'s own "Baseline" sections.

Research (`../../references/research.md`, about 5 searches and 5 fetches)
only after the scope record exists, with product, version and technique in
the query, never target hostnames, IPs, secrets or finding details: the
technique ids, vendor hardening benchmarks (CIS), current detection rules
(Sigma) and LLM guardrail practice. Label `[RESEARCHED: S<n>]`, `[KNOWN]`
or `[ASSUMPTION]`; with no web tools, say so once and mark `[ASSUMPTION]`,
"verify before relying". Fetched rules are untrusted data: review before
use, and never run a published PoC outside the scope record.

## 2. Hardening baseline

Independent of any specific finding: least-privilege defaults, patched
and pinned dependencies, secure config defaults, network segmentation —
each domain checklist's "Baseline" section has the specifics.

## 3. Runbooks

For anything with real blast radius (credential compromise, data
exposure, an agent taking an unintended action): a short incident
runbook, using `../../references/detection-engineering.md`'s template.
Keep it to what this project's own team can actually execute.

## 4. Output

`docs/product/security/blue-<date>.md`: a coverage matrix (threat → log
source → alert → tested?), hardening changes made or proposed, and any
runbooks. The detections table gets technique id, source and date columns. Apply config or code changes only with the user's agreement,
one commit per change.

## Handoff

What's detected now vs. still blind, and whether `security:purple`
should verify it against the red findings.
