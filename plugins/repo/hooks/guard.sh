#!/bin/sh
# PreToolUse guard for the Bash tool: reporun (plugins/repo/scripts/reporun.py)
# refuses a change to a platform — creating, commenting, updating, merging —
# unless it gets --write, and this guard turns every --write into a question,
# so the user confirms each change to their issues, work items and pull
# requests.
#
#   deny  a --write call whose command text carries agent attribution
#         (Co-Authored-By or a "generated with" signature) — the same rule
#         git:workflow's guard enforces for commits and PRs/issues opened
#         with `gh`
#   ask   any other reporun call with --write
#
# Pattern matching on the raw command; reporun's own refusals are the
# primary guard.

set -u

input="$(tr '\n' ' ')"

cmd="$(printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p')"
[ -n "$cmd" ] || exit 0

# One simple command per line, quoted strings blanked, split on ; & | ( ).
segments="$(printf '%s\n' "$cmd" | awk '{
  gsub(/\\"([^\\]|\\[^"])*\\"/, "\"\""); gsub(/\047[^\047]*\047/, "\047\047")
  gsub(/\\n/, "\n"); gsub(/[;&|()]/, "\n"); print }')"

# reporun as the command word: bare, or run by python3 / uv run, after env assignments.
runner='^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*[[:space:]]+)*((python3?|uv[[:space:]]+run([[:space:]]+python3?)?)[[:space:]]+)?([^[:space:]]*/)?reporun(\.py)?[[:space:]]'
if printf '%s\n' "$segments" | grep -E "$runner" | grep -Eq '[[:space:]]--write([[:space:]]|$)'; then
  if printf '%s' "$cmd" | grep -Eiq 'co-authored-by[[:space:]]*:|generated[[:space:]]+(with|by)[[:space:]]{0,3}\[?claude'; then
    printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"repo guard: %s"}}\n' \
      "this user's issues, work items, pull requests and comments carry no sign-off or mention of the agent that wrote them; remove it and run the command again"
    exit 0
  fi
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"repo guard: %s"}}\n' \
    "reporun --write changes an issue, work item or pull request on the platform; confirm this exact change"
fi
exit 0
