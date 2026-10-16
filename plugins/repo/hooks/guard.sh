#!/bin/sh
# PreToolUse guard for the Bash tool: reporun (plugins/repo/scripts/reporun.py)
# refuses a change to a platform — creating, commenting, updating, merging —
# unless it gets --write, and this guard turns every --write into a question,
# so the user confirms each change to their issues, work items and pull
# requests.
#
#   ask   a reporun call with --write
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
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"repo guard: %s"}}\n' \
    "reporun --write changes an issue, work item or pull request on the platform; confirm this exact change"
fi
exit 0
