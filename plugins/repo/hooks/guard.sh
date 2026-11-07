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
#   lift  while the autonomy grant (.ai-gent/autonomy at the repository root,
#         written by the user; see the git guard) is active, a GitHub
#         `reporun cli github --write -- ...` that creates a PR into a branch
#         the grant covers (actions: pr), merges one (actions: merge; reporun
#         itself refuses a base the grant does not cover) or creates, comments
#         on or edits an issue (actions: issues) stops asking
#
# Pattern matching on the raw command; reporun's own refusals are the
# primary guard.

set -u

input="$(tr '\n' ' ')"

# Shadow mode (spec 033 FR-009): once the decision is printed, the installed
# ai-gent binary, when there is one, gets the same event detached and only
# records whether it agrees. It never changes the decision, never writes to
# stdout and is silent without the binary; AI_GENT_NO_SHADOW=1 turns it off.
sd=none sr=
# shellcheck disable=SC2329 # run by the EXIT trap
shadow() {
  [ "${AI_GENT_NO_SHADOW:-0}" = 0 ] || return 0
  b="${CLAUDE_PLUGIN_ROOT:-${0%/*}/..}/../../bin/ai-gent"
  [ -x "$b" ] || b="$(command -v ai-gent 2>/dev/null)" || return 0
  [ -x "$b" ] || return 0
  if [ "$sd" = none ]; then
    ( printf '%s' "$input" | "$b" guard --guard repo --shadow --shell-decision none >/dev/null 2>&1 & ) >/dev/null 2>&1
  else
    ( printf '%s' "$input" | "$b" guard --guard repo --shadow --shell-decision "$sd" --shell-reason "$sr" >/dev/null 2>&1 & ) >/dev/null 2>&1
  fi
  return 0
}
trap shadow EXIT

cmd="$(printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p')"
[ -n "$cmd" ] || exit 0
cwd="$(printf '%s' "$input" | sed -n 's/.*"cwd"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"

# One simple command per line, quoted strings blanked, split on ; & | ( ).
segments="$(printf '%s\n' "$cmd" | awk '{
  gsub(/\\"([^\\]|\\[^"])*\\"/, "\"\""); gsub(/\047[^\047]*\047/, "\047\047")
  gsub(/\\n/, "\n"); gsub(/[;&|()]/, "\n"); print }')"

# The autonomy grant (same format and rules as the git guard's): key: value lines
# in .ai-gent/autonomy at the repository root, active while expires lies ahead.
grant_value() { # grant_value <key>
  sed -n "s/^$1:[[:space:]]*//p" "$grant_file" | head -n1 | tr -d '\r'
}
grant_load() {
  grant_file=
  top="$(git -C "${cwd:-.}" rev-parse --show-toplevel 2>/dev/null)" || return 1
  if [ ! -f "$top/.ai-gent/autonomy" ]; then
    # A linked worktree has no copy of the git-excluded file: use the main checkout's
    common="$(git -C "${cwd:-.}" rev-parse --path-format=absolute --git-common-dir 2>/dev/null)" || return 1
    [ "${common##*/}" = ".git" ] || return 1
    top="${common%/.git}"
  fi
  [ -f "$top/.ai-gent/autonomy" ] || return 1
  grant_file="$top/.ai-gent/autonomy"
  exp="$(grant_value expires)"
  printf '%s\n' "$exp" | grep -Eq '^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$' || return 1
  now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  [ "$now" != "$exp" ] && [ "$(printf '%s\n%s\n' "$now" "$exp" | LC_ALL=C sort | head -n1)" = "$now" ]
}
grant_has() {
  grant_load || return 1
  printf ' %s ' "$(grant_value actions)" | grep -Eq "[[:space:]]$1[[:space:]]"
}
grant_covers() {
  b="$(grant_value branches)"
  printf ' %s ' "${b:-develop dev}" | grep -Eq "[[:space:]]$1[[:space:]]"
}

# Whether the grant lifts this reporun segment: reporun cli github, then the gh
# words (pr create --base <covered>, pr merge, issue create|comment|edit).
lifted() { # lifted <segment>
  words="$(printf '%s\n' "$1" | sed -E 's#^.*reporun(\.py)?[[:space:]]+cli[[:space:]]+github[[:space:]]*##')"
  [ "$words" != "$1" ] || return 1
  words="$(printf '%s\n' "$words" | sed -E 's/--write([[:space:]]|$)//g; s/--entry[[:space:]]+[^[:space:]]+//; s/^[[:space:]]*(--[[:space:]]+)?(gh[[:space:]]+)?//')"
  noun="$(printf '%s\n' "$words" | awk '{print $1}')"
  verb="$(printf '%s\n' "$words" | awk '{print $2}')"
  case "$noun $verb" in
    "pr create")
      grant_has pr || return 1
      base="$(printf '%s\n' "$words" | sed -nE 's/.*[[:space:]](--base|-B)[[:space:]=]+([^[:space:]]+).*/\2/p' | head -n1)"
      [ -n "$base" ] && grant_covers "$base"
      ;;
    "pr merge") grant_has merge ;;
    "issue create" | "issue comment" | "issue edit") grant_has issues ;;
    *) return 1 ;;
  esac
}

# reporun as the command word: bare, or run by python3 / uv run, after env assignments.
runner='^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*[[:space:]]+)*((python3?|uv[[:space:]]+run([[:space:]]+python3?)?)[[:space:]]+)?([^[:space:]]*/)?reporun(\.py)?[[:space:]]'
writes="$(printf '%s\n' "$segments" | grep -E "$runner" | grep -E '[[:space:]]--write([[:space:]]|$)')"
if [ -n "$writes" ]; then
  if printf '%s' "$cmd" | grep -Eiq 'co-authored-by[[:space:]]*:|generated[[:space:]]+(with|by)[[:space:]]{0,3}\[?claude'; then
    sd=deny sr="repo guard: this user's issues, work items, pull requests and comments carry no sign-off or mention of the agent that wrote them; remove it and run the command again"
    printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"repo guard: %s"}}\n' \
      "this user's issues, work items, pull requests and comments carry no sign-off or mention of the agent that wrote them; remove it and run the command again"
    exit 0
  fi
  # Every write segment lifted by the grant: no question.
  all_lifted=1
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    lifted "$line" || all_lifted=0
  done <<EOF
$writes
EOF
  [ "$all_lifted" = 1 ] && exit 0
  sd=ask sr="repo guard: reporun --write changes an issue, work item or pull request on the platform; confirm this exact change"
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"repo guard: %s"}}\n' \
    "reporun --write changes an issue, work item or pull request on the platform; confirm this exact change"
fi
exit 0
