#!/bin/sh
# PreToolUse guard for the Bash tool: enforces git:workflow's
# non-negotiables mechanically.
#
#   deny  --no-verify / commit -n, agent attribution (Co-Authored-By or a
#         "generated with" signature) on a commit, issue or PR, push --force
#         without --force-with-lease
#   ask   git push, commit or merge on main/master or homolog/staging,
#         commit on dev/develop (merging into it is the workflow), branch -D,
#         reset --hard, clean -f, commit --amend, filter-branch/filter-repo,
#         gh pr create
#   deny  writing .ai-gent/autonomy, the grant only the user creates, and
#         running ai-gent autonomy in any form but show/--help
#   lift  while that grant is active (its expires: line is in the future), a
#         plain git push (actions: push) and gh pr create --base <a branch the
#         grant covers> (actions: pr) stop asking; never a push naming
#         main/master/homolog/staging, --all/--mirror/--tags/--delete or a
#         +refspec, and never a push from one of those branches
#
# Reads the hook event JSON on stdin and prints a permission decision, or
# nothing to let the normal permission flow decide. Pattern matching on
# the raw command: it catches the commands an agent writes, not every
# possible obfuscation, and it only sees the session's cwd.

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
    ( printf '%s' "$input" | "$b" guard --guard git --shadow --shell-decision none >/dev/null 2>&1 & ) >/dev/null 2>&1
  else
    ( printf '%s' "$input" | "$b" guard --guard git --shadow --shell-decision "$sd" --shell-reason "$sr" >/dev/null 2>&1 & ) >/dev/null 2>&1
  fi
  return 0
}
trap shadow EXIT

# The command string, still JSON-escaped (newlines stay as \n).
cmd="$(printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p')"
grant_reason="the autonomy grant (.ai-gent/autonomy) is written by the user, never by the agent; ask the user to create or change it"
if [ -z "$cmd" ]; then
  # Edit, Write and the like: the target path of the grant is off limits.
  target="$(printf '%s' "$input" | sed -nE 's/.*"(file_path|notebook_path)"[[:space:]]*:[[:space:]]*"([^"]*)".*/\2/p')"
  case "$target" in
    */.ai-gent/autonomy | .ai-gent/autonomy)
      sd=deny sr="git:workflow guard: $grant_reason"
      printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"git:workflow guard: %s"}}\n' "$grant_reason"
      ;;
  esac
  exit 0
fi
cwd="$(printf '%s' "$input" | sed -n 's/.*"cwd"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"

# One simple command per line, with quoted strings (commit messages) blanked
# so their text can't look like options: split on ; & | ( ) and newlines.
segments="$(printf '%s\n' "$cmd" | awk '{
  gsub(/\\"([^\\]|\\[^"])*\\"/, "\"\""); gsub(/\047[^\047]*\047/, "\047\047")
  gsub(/\\n/, "\n"); gsub(/[;&|()]/, "\n"); print }')"

# git, optionally with global options (-C <dir>, -c <k=v>, --no-pager, ...),
# followed by the subcommand.
git_re='^[[:space:]]*(sudo[[:space:]]+)?git([[:space:]]+(-[Cc][[:space:]]+[^[:space:]]+|--[a-z-]+(=[^[:space:]]+)?))*[[:space:]]+'

has() { # has <subcommand regex> [<argument regex>]
  printf '%s\n' "$segments" | grep -Eq "$git_re$1([[:space:]].*)?\$" || return 1
  [ $# -lt 2 ] && return 0
  printf '%s\n' "$segments" | grep -E "$git_re$1([[:space:]].*)?\$" | grep -Eq -- "$2"
}

decide() { # decide deny|ask <reason>
  sd="$1" sr="git:workflow guard: $2"
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"%s","permissionDecisionReason":"git:workflow guard: %s"}}\n' "$1" "$2"
  exit 0
}

# The directory whose repository counts: the -C argument of the first git
# segment that has one, else the session's cwd.
work_dir() {
  d="$(printf '%s\n' "$segments" | sed -n 's/.*git[[:space:]]\{1,\}-C[[:space:]]\{1,\}\([^[:space:]]*\).*/\1/p' | head -n1)"
  [ -n "$d" ] || d="$cwd"
  printf '%s' "${d:-.}"
}

# The autonomy grant: .ai-gent/autonomy at the repository root, key: value lines
# (expires: 2026-11-06T18:00:00Z, branches: develop dev, actions: push pr merge
# issues). Active only while expires lies in the future; anything unreadable is
# no grant. grant_has <action> succeeds when it is active and names the action;
# grant_covers <branch> when the branch is one of its branches (default
# "develop dev").
grant_value() { # grant_value <key>
  sed -n "s/^$1:[[:space:]]*//p" "$grant_file" | head -n1 | tr -d '\r'
}
grant_load() {
  grant_file=
  top="$(git -C "$(work_dir)" rev-parse --show-toplevel 2>/dev/null)" || return 1
  if [ ! -f "$top/.ai-gent/autonomy" ]; then
    # A linked worktree has no copy of the git-excluded file: use the main checkout's
    common="$(git -C "$(work_dir)" rev-parse --path-format=absolute --git-common-dir 2>/dev/null)" || return 1
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
current_branch() {
  git -C "$(work_dir)" symbolic-ref --quiet --short HEAD 2>/dev/null || true
}

if has '(commit|merge|push|am|rebase|cherry-pick|revert)' '(^|[[:space:]])--no-verify([[:space:]]|$)' ||
  has 'commit' '(^|[[:space:]])-n([[:space:]]|$)'; then
  decide deny "hooks must not be bypassed (--no-verify / -n); fix what the hook reports and try again"
fi
attributed() { # the raw command text carries an actual trailer or signature, not just prose about one
  printf '%s' "$cmd" | grep -Eiq 'co-authored-by[[:space:]]*:|generated[[:space:]]+(with|by)[[:space:]]{0,3}\[?claude'
}
if { has '(commit|merge|revert|cherry-pick|tag|notes)' ||
     printf '%s\n' "$segments" | grep -Eq '^[[:space:]]*gh[[:space:]]+(issue|pr)([[:space:]]|$)'; } &&
  attributed; then
  decide deny "this user's commits, issues, pull requests and comments carry no sign-off or mention of the agent that wrote them; remove it and run the command again"
fi
if has 'push' '(^|[[:space:]])(--force|-[a-zA-Z]*f[a-zA-Z]*)([[:space:]]|$)' &&
  ! has 'push' '--force-with-lease'; then
  decide deny "force pushes use --force-with-lease, and only on the user's explicit order"
fi

# Nothing but the user writes the grant: a command that names it and is not a read.
if printf '%s' "$cmd" | grep -Fq '.ai-gent/autonomy' &&
  { printf '%s' "$cmd" | grep -q '>' ||
    printf '%s\n' "$segments" | grep -F '.ai-gent/autonomy' |
      grep -Evq '^[[:space:]]*(cat|ls|head|tail|grep|stat|wc|test|\[)[[:space:]]'; }; then
  decide deny "$grant_reason"
fi

# Nor does the agent run the grant CLI (ai-gent autonomy ...), in any spelling. The real
# defence is structural (grant and revoke need a real terminal); this is defence in depth.
# Deny a command that has the word ai-gent and an `autonomy` that is not followed by
# show/status/--help/-h, and any `autonomy [flags] grant|revoke` even without ai-gent.
# Tested on the raw command and on a copy with quotes, backslashes and $( ) stripped.
autonomy_denied() { # autonomy_denied <text>
  t="$(printf '%s' "$1" | tr '\n' ' ')"
  w='(^|[^A-Za-z0-9_-])'
  printf '%s' "$t" | grep -Eq "${w}autonomy([[:space:]]+-[^[:space:]]*)*[[:space:]]+(grant|revoke)([^A-Za-z0-9_-]|\$)" && return 0
  printf '%s' "$t" | grep -Eq "${w}ai-gent(\.exe)?([^A-Za-z0-9_/.-]|\$)" || return 1
  rest="$(printf '%s' "$t" | sed -E "s/${w}autonomy[[:space:]]+(show|status|--help|-h)([^A-Za-z0-9_-]|\$)/ /g")"
  printf '%s' "$rest" | grep -Eq "${w}autonomy([^A-Za-z0-9_-]|\$)"
}
cmd_plain="$(printf '%s' "$cmd" | tr -d "'\"\\\\\`" | sed -e 's/\$(//g' -e 's/)//g')"
if autonomy_denied "$cmd" || autonomy_denied "$cmd_plain"; then
  decide deny "the autonomy grant is created and removed by the user, never by the agent (ai-gent autonomy grant / revoke); ask the user to run it in their terminal, or with ! in the prompt"
fi

# A push the grant lifts: every push segment names no protected branch and no
# bulk or forced form, from a branch that is not one either.
push_lifted() {
  grant_has push || return 1
  br="$(current_branch)"
  case "$br" in "" | main | master | homolog | staging) return 1 ;; esac
  if printf '%s\n' "$segments" | grep -E "$git_re""push([[:space:]].*)?\$" |
    grep -Eq '(^|[[:space:]])(--all|--mirror|--tags|--delete|--prune|-d)([[:space:]]|$)|(^|[[:space:]:/])(main|master|homolog|staging)([[:space:]]|$)|(^|[[:space:]])\+'; then
    return 1
  fi
  return 0
}
pr_create_lifted() {
  grant_has pr || return 1
  base="$(printf '%s\n' "$segments" | grep -E '^[[:space:]]*gh[[:space:]]+pr[[:space:]]+create' |
    sed -nE 's/.*[[:space:]](--base|-B)[[:space:]=]+([^[:space:]]+).*/\2/p' | head -n1)"
  [ -n "$base" ] && grant_covers "$base"
}

if has 'push' && ! push_lifted; then
  decide ask "git push publishes; allow only if the user explicitly asked for this push"
fi
if printf '%s\n' "$segments" | grep -Eq '^[[:space:]]*gh[[:space:]]+pr[[:space:]]+create' && ! pr_create_lifted; then
  decide ask "opening a PR publishes the branch; allow only on the user's explicit request"
fi
if has 'branch' '(^|[[:space:]])(-D|--delete[[:space:]]+--force|--force[[:space:]]+--delete)([[:space:]]|$)'; then
  decide ask "git branch -D deletes unmerged work; the workflow uses git branch -d"
fi
if has 'reset' '--hard' || has 'clean' '(^|[[:space:]])-[a-zA-Z]*f' ||
  has 'commit' '--amend' || has '(filter-branch|filter-repo)'; then
  decide ask "this rewrites history or discards work; allow only on the user's explicit request"
fi
if has '(commit|merge)'; then
  dir="$cwd"
  c_dir="$(printf '%s\n' "$segments" | sed -n 's/.*git[[:space:]]\{1,\}-C[[:space:]]\{1,\}\([^[:space:]]*\).*/\1/p' | head -n1)"
  [ -n "$c_dir" ] && dir="$c_dir"
  branch="$(git -C "${dir:-.}" symbolic-ref --quiet --short HEAD 2>/dev/null || true)"
  case "$branch" in
    main | master)
      decide ask "this commits or merges on $branch; the workflow works on a branch (allow only if the user asked for it)"
      ;;
    homolog | staging)
      decide ask "this commits or merges on $branch; promotion up the branch chain needs the user's explicit request"
      ;;
    dev | develop)
      has 'commit' &&
        decide ask "this commits directly on $branch; work goes on a branch merged into it (conclude a merge with git merge --continue)"
      ;;
  esac
fi

exit 0
