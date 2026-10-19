# Detection engineering

Shared by `security:blue` and `security:purple`: how to turn a technique
into something that actually fires, and how to write a runbook someone
can execute at 3am.

## The process

1. **Technique** — what the attacker does (from a `security:red` finding,
   or from a domain checklist directly).
2. **Data source** — what already logs it, or would need to: app
   authn/authz events and audit logs; cloud audit logs (CloudTrail,
   Activity Log, Cloud Audit Logs) and VPC flow logs; container/orchestrator
   runtime events; for AI/LLM, tool-call logs and prompt/response pairs
   where policy allows, plus rate-limit counters.
3. **Rule** — the actual query or alert condition, specific enough to
   fire on the technique and not on everything that looks adjacent.
4. **Test** — fire a known-bad event (from the red finding's reproduction
   steps, inside the same scope) and confirm the rule catches it; then
   confirm it stays quiet on ordinary traffic for a reasonable window.
5. **Tune** — adjust threshold/window from the false-positive rate
   observed, not from a guess.

## Mapping coverage

Use MITRE ATT&CK for application/infrastructure techniques and MITRE
ATLAS for AI/ML-specific ones, not to chase 100% coverage but to see
*where* the gaps are by tactic (initial access, persistence, exfiltration,
...) rather than by a flat list of individual findings.

## Runbook template

| Field | Content |
|---|---|
| Trigger | the alert or report that starts this runbook |
| Severity | Critical · High · Medium · Low |
| First 15 minutes | who's paged, what to check first, how to confirm it's real |
| Containment | the fastest safe way to stop it spreading or recurring |
| Eradication | removing the cause, not just the symptom |
| Recovery | restoring normal operation, what to verify before calling it closed |
| Communication | who gets told, when, and what's said at each stage |
| Postmortem | what changes so this class of incident is caught earlier next time |

Keep it to what this project's own team can actually execute — a runbook
that assumes tooling or access the team doesn't have is worse than none.
