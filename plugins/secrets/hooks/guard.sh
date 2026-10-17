#!/bin/sh
# PreToolUse guard for the Bash tool: secret values in pass never reach the
# agent (shared/secrets.md). ai-gent's runners (dbrun, ...) read an entry
# for the command that needs it; the agent only sees names.
#
#   deny  a pass command that prints or copies a value: show, a bare
#         `pass <entry>`, -c/--clip, -q/--qrcode, edit, grep, generate, otp
#   deny  pass insert/add: the agent never writes a credential
#   ask   pass rm/mv/cp/init and pass git (but pull, fetch, status, log and
#         diff, which a team store's sync runs routinely): the user decides
#   deny  gpg on the password store (decrypting it directly)
#   deny  pass-run with an ai-gent/db/ entry (only dbrun reads those), or
#         handing a secret to a command that can print its environment
#   none  pass ls/find, help, version
#
# Pattern matching on the raw command: it catches the commands an agent
# writes, not every possible obfuscation.

set -u

input="$(tr '\n' ' ')"

decide() { # decide deny|ask <reason>
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"%s","permissionDecisionReason":"secrets guard: %s"}}\n' "$1" "$2"
  exit 0
}

cmd="$(printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p')"
[ -n "$cmd" ] || exit 0

# One simple command per line, quoted strings blanked (so text inside quotes
# can't look like a command), split on ; & | ( ) and newlines.
segments="$(printf '%s\n' "$cmd" | awk '{
  gsub(/\\"([^\\]|\\[^"])*\\"/, "\"\""); gsub(/\047[^\047]*\047/, "\047\047")
  gsub(/\\n/, "\n"); gsub(/[;&|()]/, "\n"); print }')"

prefix='^[[:space:]]*((sudo([[:space:]]+-[^[:space:]]+([[:space:]]+[^-[:space:]][^[:space:]]*)?)*|env|exec|nice|timeout[[:space:]]+[0-9smh]+|[A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*)[[:space:]]+)*'

# gpg anywhere in a command that names the password store.
if printf '%s\n' "$segments" | grep -Eq "$prefix([^[:space:]]*/)?gpg2?([[:space:]]|\$)" &&
  printf '%s' "$cmd" | grep -Eq '\.password-store|PASSWORD_STORE_DIR'; then
  decide deny "decrypting the password store directly shows secret values; the runner that needs a secret reads it"
fi

# pass-run (dottod) and similar runners: a secret for one command.
runner="$(printf '%s\n' "$segments" | grep -E "$prefix([^[:space:]]*/)?pass-run([[:space:]]|\$)" | head -n1)"
if [ -n "$runner" ]; then
  if printf '%s' "$cmd" | grep -Eq '(^|[=:[:space:]])([A-Za-z0-9_.@+-]+/)?ai-gent/db/'; then
    decide deny "ai-gent/db/ entries are read only by dbrun; use dbrun test / query"
  fi
  after="$(printf '%s\n' "$runner" | awk '{ for (i = 1; i < NF; i++) if ($i == "--") { print $(i + 1); exit } }')"
  case "${after##*/}" in
    env | printenv | set | export | declare | typeset | echo | printf | cat | sh | bash | dash | zsh | ksh | fish | python* | node | perl | ruby | eval)
      decide deny "that hands the secret to a command that can print it; run the tool that needs it after --"
      ;;
  esac
fi

hit="$(printf '%s\n' "$segments" | grep -E "$prefix([^[:space:]]*/)?pass([[:space:]]|\$)" | head -n1)"
[ -n "$hit" ] || exit 0

rest="$(printf '%s\n' "$hit" | sed -E "s#$prefix([^[:space:]]*/)?pass([[:space:]]|\$)##")"
set -f
# shellcheck disable=SC2086 # split the arguments on purpose
set -- $rest
set +f
sub=""
sub2=""
for arg in "$@"; do
  case "$arg" in
    -c | --clip | -c[0-9]* | --clip=* | -q | --qrcode | -q[0-9]* | --qrcode=*)
      decide deny "pass -c/-q exposes a secret value; the agent never reads one — the runner that needs it does"
      ;;
    -*) ;;
    *) if [ -z "$sub" ]; then sub="$arg"; elif [ -z "$sub2" ]; then sub2="$arg"; fi ;;
  esac
done

case "$sub" in
  "" | ls | list | find | search | help | --help | version | --version) exit 0 ;;
  insert | add)
    decide deny "the agent never writes a credential; ask the user to run \`pass insert <entry>\` in their own terminal"
    ;;
  git)
    # A team store syncs through git: reading its state and fast-forwarding
    # are routine; publishing (push) and anything else stay the user's call.
    case "$sub2" in
      pull | fetch | status | log | diff) exit 0 ;;
      *) decide ask "pass git ${sub2:-} changes or publishes the password store; the user decides" ;;
    esac
    ;;
  rm | remove | delete | mv | rename | cp | copy | init)
    decide ask "pass $sub changes the password store; the user decides"
    ;;
  *)
    decide deny "that shows a secret value from pass; the agent sees names only (pass ls) — the runner that needs a secret reads it"
    ;;
esac
