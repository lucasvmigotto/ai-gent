# Implementation Plan: Workers pane shows the slots over the limit

**Spec**: `spec.md` · **Branch**: `feat/workers-over-limit-color` · **Plugin**: `project` (a mod, optional)

## Approach

Keep the model's text as the source of truth and colour it by ranges.

1. `workers-model.ts`
   - `Line` gets `marks?: { from: number; to: number; tone: Tone }[]`, character ranges of `text`.
   - `capacity()` computes `over`. When over and the cells fit, it builds `cells` as `cap` filled ones plus `used - cap` of `[!]`, and the right-aligned status; the meter line is `{ tone: 'info', marks }` where the marks cover the `[!]` cells and the status. The figure line is `bad`. When over and they do not fit, the existing compact line with `bad`.
   - Under or at the cap nothing changes (the tone stays `info` or `dim`).
   - `compactLine()` returns `bad` when `slotsUsed > cap` (and cap is known), after the failed and before the stalled rule.
   - `add()` in `build` keeps passing `marks` through (the capacity lines are pushed as they are, so only the `l.text` spread there needs to keep the field).
2. `workers-draw.tsx`: a line with marks is drawn as a `Box` row of `Text` segments cut at the marks; each segment uses `look(tone)`; a line without marks is untouched. No wrap on segments: the text is already fitted to the width.
3. Tests: `workers-model.test.ts` for the text, the marks and the fall-backs; `workers.test.tsx` for the drawn segments and tones.
4. Docs: the README row for the project mod and a CHANGELOG line; spec 041's FR-017 is not edited (this spec refines it).

## Risks

- A `Text` segment's width in a row: the segments are cut from an already padded string, so their widths add up to the line's width; a test checks that the concatenated segments equal the text.
- Wide characters: `■` and `⚠` are single-width in the pane today; `[!]` is ASCII.
- `marks` on a line that is later `fit()` (truncated): the capacity lines are built to the exact width and are never re-cut, so the ranges stay valid; the test covers the narrowest width where the cells still show.

## Verification

`claude plugin test plugins/project`, `claude plugin validate plugins/project`, `scripts/check.sh --quick --quiet` while working, the full `scripts/check.sh` once before the last commit, and a mutation pass (remove the over tone, the overflow cells and the compact `bad`; each makes a test fail).
