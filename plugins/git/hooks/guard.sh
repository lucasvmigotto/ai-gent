#!/bin/sh
# PreToolUse guard for the Bash tool: enforces git:workflow's
# non-negotiables mechanically.
#
#   deny  --no-verify / commit -n, Co-Authored-By trailers, push --force
#         without --force-with-lease
#   ask   git push, commit or merge on main/master, branch -D, reset --hard,
#         clean -f, commit --amend, filter-branch/filter-repo, gh pr create
#
# Reads the hook event JSON on stdin and prints a permission decision, or
# nothing to let the normal permission flow decide. Pattern matching on
# the raw command: it catches the commands an agent writes, not every
# possible obfuscation, and it only sees the session's cwd.

set -u

input="$(tr '\n' ' ')"

# The command string, still JSON-escaped (newlines stay as \n).
cmd="$(printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p')"
[ -n "$cmd" ] || exit 0
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
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"%s","permissionDecisionReason":"git:workflow guard: %s"}}\n' "$1" "$2"
  exit 0
}

if has '(commit|merge|push|am|rebase|cherry-pick|revert)' '(^|[[:space:]])--no-verify([[:space:]]|$)' ||
  has 'commit' '(^|[[:space:]])-n([[:space:]]|$)'; then
  decide deny "hooks must not be bypassed (--no-verify / -n); fix what the hook reports and try again"
fi
if has '(commit|merge|revert|cherry-pick|tag|notes)' && printf '%s' "$cmd" | grep -Eiq 'co-authored-by'; then
  decide deny "this user's commits carry no Co-Authored-By trailers; remove it and commit again"
fi
if has 'push' '(^|[[:space:]])(--force|-[a-zA-Z]*f[a-zA-Z]*)([[:space:]]|$)' &&
  ! has 'push' '--force-with-lease'; then
  decide deny "force pushes use --force-with-lease, and only on the user's explicit order"
fi

if has 'push'; then
  decide ask "git push publishes; allow only if the user explicitly asked for this push"
fi
if printf '%s\n' "$segments" | grep -Eq '^[[:space:]]*gh[[:space:]]+pr[[:space:]]+create'; then
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
  esac
fi

exit 0
