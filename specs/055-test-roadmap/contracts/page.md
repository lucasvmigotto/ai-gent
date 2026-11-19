# Contract — the roadmap page

What `ai-gent uat render` produces and what the skill checks before publishing. The page is HTML, CSS and a small inline script that runs in the claude.ai Artifact viewer; it is a template embedded in the binary and filled from `roadmap.json`, not code the agent writes per session. Requirements: spec FR-011 to FR-033, FR-050, FR-051, FR-091.

## Markup

- `<title>`: two to four words naming the session.
- Sections with fixed, English, stable ids, in the order of FR-001: `how-to`, `people`, `rules`, `test-data`, `scripts`, `results`, `limitations`, `changelog`; plus `roster` (rendered for everyone, controls only for the owner).
- Each script: `<h3 id="r<N>">`, then a `<div class="scroll"><table class="steps">` with header `#`, Who, Action and expected result, Result.
- Each active step: `<tr data-step="r<N>-<M>" data-rev="<n>" data-role="<role key>" data-kind="normal|write-check|role-confirmation" data-exclusive="true|false">`. Cell 3 holds the action, the expected result (marked up as such), the precondition, the wait and the exclusivity note. Retired steps are not rendered.
- Controls created by the script carry ids `res-<step>-ok`, `res-<step>-nok`, `obs-<step>`, `links-<step>`, `imp-btn-<step>`, `imp-<step>`, and, only with attachments on, `ev-<step>`. The role-confirmation step adds `prop-role` (a select of role labels) and `prop-note`.
- Status line `#status` (`role="status"`), fixed notice `#notice` (`role="alert"`, hidden unless an error).
- The roadmap version in `<meta name="uat-version">` and in the header text; the session's settings in `<meta name="uat-attachments" content="off|on">`.

## Script behavior (MUSTs)

1. Render all text with no register; light up when `claude.use("db")` and `claude.use("user")` resolve; `null` → read-only with a reason (FR-033).
2. `user.id()` for the own documents; `user.can("data.write")` false → read-only; `null` → keep controls and let a refused write decide.
3. One `onSnapshot` per collection (`marks`, `roster`, `proposals`, `meta`, `triage`), each subscribed once; a 10 s timer for the first server snapshot (FR-032).
4. One queued whole-document `set` of `marks/<uid>` (or `proposals/<uid>`) per change; changes during a write coalesce into the next (FR-031).
5. On `invalid_argument` or `not_granted`: revert local state to the last snapshot of the own document, hide controls, show the notice (FR-030). On `quota_exceeded`: quota notice. Other errors: retry notice, keep local state.
6. Profiles resolved in render from ids on screen; `name || "Validator"`; set by `textContent` only (FR-018).
7. All register text inserted with `textContent`, never `innerHTML`; links rendered only for `https:` URLs, with `rel="noopener noreferrer"` and `target="_blank"`.
8. Stale badge when `mark.rev < data-rev`; reload notice when `meta/roadmap.version` is higher than the page's `uat-version` (FR-022).
9. Coverage counts only `roster` roles; pending proposals show a "proposal pending" tag to everyone and the note and approve/reject controls to the owner (`user.isOwner()`); approving writes `roster/<uid>` and `proposals/<uid>.decision` (FR-026, FR-091).
10. With `uat-attachments` off, no upload control is created and `assets` is never requested (FR-050). With it on: limits and remaining budget shown, files over the budget refused before upload, only the asset id stored, the privacy warning next to the control (FR-051, FR-094).
11. No browser storage for shared state; browser storage only for per-viewer conveniences (collapsed sections), wrapped in try/catch.

## Style

- Color tokens on `:root`, dark values under `@media (prefers-color-scheme: dark)` guarded by `:root:not([data-theme="light"])` and again under `:root[data-theme="dark"]`; `body` has an explicit background (FR-023).
- OK, NOK, stale, improvement and pending-proposal each have a foreground and background token that reach AA contrast in both themes (FR-024).
- 16 px side gutter, tables scroll inside `.scroll`, no horizontal page scroll at 375 px (FR-012).
- No external scripts; fonts only from Google Fonts, optional.

## Publish

- First publish: `capabilities` with the `db` rules and `user` scopes from `data-model.md`; `assets` only when the owner turned attachments on (FR-044, FR-094).
- Republish: same file path, `capabilities` omitted; before it, read the published page and compare with `page.html` (FR-097); after it, the agent writes `meta/roadmap` with the new version (the standing request of FR-071).
- After the first publish, and after any republish that restates capabilities: the preflight test read (`data-model.md`, "Preflight test read"; FR-093), recorded in `preflight.md`.
