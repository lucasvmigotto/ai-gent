---
name: purple
description: Pair each security:red finding with its security:blue detection or fix and retest to prove the pairing actually works — a coverage matrix of attack to detection, re-run after every change, closing the loop between the two reports. Use for "did we actually fix it," a purple team exercise, or validating detections against real findings.
---

# Purple team validation

## Inputs

Both a `security:red` report and a `security:blue` report — this skill
pairs their output rather than replacing either; run them first if one
is missing. `security:scope`'s record still applies: retesting a red
finding means repeating the original technique, which needs the same
authorization it needed the first time.

## Procedure

For each red finding:

1. **Confirm a paired control exists.** A finding with neither a fix
   nor a detection from `security:blue` isn't purple-ready yet — hand it
   back instead of inventing one here.
2. **Re-run the original red technique**, inside the same approved
   scope and window, and confirm: the fix actually blocks it, and/or the
   detection actually fires — with the alert or log evidence attached,
   not "should work."
3. **If it doesn't hold**, that's a new red finding (the fix is
   incomplete, or the detection has a gap), logged as a regression, not
   a success.

## Coverage matrix

| Finding | Red evidence | Blue control | Retested | Result |
|---|---|---|---|---|

One row per red finding: its id, what the evidence was, what fix or
detection `security:blue` paired with it, the retest's evidence, and
Pass / Fail / Partial.

## Output

`docs/product/security/purple-<date>.md`: the matrix, the closed-loop
percentage (how many red findings now have a proven control), and
what's still open.

## Handoff

What's provably fixed vs. still exposed; offer to feed the closed items
to `devsecops:pipeline` as a gate where a CI check can catch a
regression of the same class.
