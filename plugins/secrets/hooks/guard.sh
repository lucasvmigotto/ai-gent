#!/bin/sh
# PreToolUse guard for the Bash tool: secret values in pass never reach the
# agent (shared/secrets.md). ai-gent's runners (dbrun, ...) read an entry
# for the command that needs it; the agent only sees names.
#
#   deny  a pass command that prints or copies a value: show, a bare
#         `pass <entry>`, -c/--clip, -q/--qrcode, edit, grep, generate, otp
#   deny  pass insert/add: the agent never writes a credential
#   ask   pass rm/mv/cp/init/git: changes the store, the user decides
#   deny  gpg on the password store (decrypting it directly)
#   deny  pass-run with an ai-gent/db/ entry (only dbrun reads those), or
#         handing a secret to a command that can print its environment
#   none  pass ls/find, help, version
#   none  `pass show <entry>` (no flags, one entry) when the user's own
#         allowlist names the entry (allowed_entry below); never under
#         ai-gent/db/, never with PASSWORD_STORE_*
#   ask   any command that mentions the allowlist file (pass-allow): the agent
#         doesn't widen its own reach
#
# Pattern matching on the raw command: it catches the commands an agent
# writes, not every possible obfuscation.

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
    ( printf '%s' "$input" | "$b" guard --guard secrets --shadow --shell-decision none >/dev/null 2>&1 & ) >/dev/null 2>&1
  else
    ( printf '%s' "$input" | "$b" guard --guard secrets --shadow --shell-decision "$sd" --shell-reason "$sr" >/dev/null 2>&1 & ) >/dev/null 2>&1
  fi
  return 0
}
trap shadow EXIT

decide() { # decide deny|ask <reason>
  sd="$1" sr="secrets guard: $2"
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

# The user's personal allowlist of pass entries the agent may `pass show`:
# $AI_GENT_PASS_ALLOW, else ${XDG_CONFIG_HOME:-~/.config}/ai-gent/pass-allow.
# One rule per line: `entry` (exact) or `prefix/` (everything under it); blank
# lines and # comments ignored; rules that are empty, start with / - or ~, or
# have a . or .. path part never allow. Missing or unreadable file: no rule.
allow_file="${AI_GENT_PASS_ALLOW:-${XDG_CONFIG_HOME:-${HOME:-}/.config}/ai-gent/pass-allow}"

# shellcheck disable=SC2329 # called below
allowed_entry() { # allowed_entry <entry>: 0 when a rule names it
  [ -f "$allow_file" ] && [ -r "$allow_file" ] || return 1
  awk -v e="$1" '
    { sub(/\r$/, ""); sub(/^[ \t]+/, ""); sub(/[ \t]+$/, "") }
    $0 == "" || substr($0, 1, 1) == "#" { next }
    index("/-~", substr($0, 1, 1)) > 0 { next }
    {
      n = split($0, p, "/"); bad = 0
      for (i = 1; i <= n; i++) if (p[i] == ".." || p[i] == ".") bad = 1
      if (bad) next
      if (substr($0, length($0)) == "/") {
        if (index(e, $0) == 1 && length(e) > length($0)) { found = 1; exit }
      } else if ($0 == e) { found = 1; exit }
    }
    END { exit !found }
  ' "$allow_file" 2>/dev/null
}

# Whatever names the allowlist file is the user's to approve: the agent must
# not widen its own reach through Bash. (Write/Edit are outside this hook.)
if printf '%s' "$cmd" | grep -Fq -e pass-allow -e AI_GENT_PASS_ALLOW ||
  { [ -n "${AI_GENT_PASS_ALLOW:-}" ] && printf '%s' "$cmd" | grep -Fq -e "$AI_GENT_PASS_ALLOW"; }; then
  decide ask "that command mentions the pass allowlist (pass-allow); the user decides what the agent may read"
fi

hit="$(printf '%s\n' "$segments" | grep -E "$prefix([^[:space:]]*/)?pass([[:space:]]|\$)" | head -n1)"
[ -n "$hit" ] || exit 0
npass="$(printf '%s\n' "$segments" | grep -Ec "$prefix([^[:space:]]*/)?pass([[:space:]]|\$)")"

rest="$(printf '%s\n' "$hit" | sed -E "s#$prefix([^[:space:]]*/)?pass([[:space:]]|\$)##")"
set -f
# shellcheck disable=SC2086 # split the arguments on purpose
set -- $rest
set +f
sub="" entry="" nflags=0 nargs=0
for arg in "$@"; do
  case "$arg" in
    -c | --clip | -c[0-9]* | --clip=* | -q | --qrcode | -q[0-9]* | --qrcode=*)
      decide deny "pass -c/-q exposes a secret value; the agent never reads one — the runner that needs it does"
      ;;
    -*) nflags=$((nflags + 1)) ;;
    *)
      nargs=$((nargs + 1))
      if [ -z "$sub" ]; then sub="$arg"; elif [ -z "$entry" ]; then entry="$arg"; fi
      ;;
  esac
done

# Could the allowlist open this command? Only `pass show <entry>`: no flags,
# one clean entry name, the only pass command in the line, no store or config
# override in the text, and never an ai-gent/db/ entry (dbrun reads those).
show_eligible() {
  [ "$sub" = show ] && [ "$nflags" -eq 0 ] && [ "$nargs" -eq 2 ] && [ "$npass" -eq 1 ] || return 1
  case "$entry" in
    "" | /* | -* | */ | *//* | *[!A-Za-z0-9._@+/-]*) return 1 ;;
    . | .. | ./* | ../* | */. | */.. | */./* | */../*) return 1 ;;
  esac
  lower="$(printf '%s' "$entry" | tr '[:upper:]' '[:lower:]')"
  case "$lower" in ai-gent/db | ai-gent/db/*) return 1 ;; esac
  ! printf '%s' "$cmd" | grep -Eq 'PASSWORD_STORE_|XDG_CONFIG_HOME|(^|[^A-Za-z0-9_])HOME='
}

case "$sub" in
  "" | ls | list | find | search | help | --help | version | --version) exit 0 ;;
  insert | add)
    decide deny "the agent never writes a credential; ask the user to run \`pass insert <entry>\` in their own terminal"
    ;;
  rm | remove | delete | mv | rename | cp | copy | init | git)
    decide ask "pass $sub changes the password store; the user decides"
    ;;
  *)
    if show_eligible; then
      allowed_entry "$entry" && exit 0
      decide deny "that shows a secret value from pass; the agent sees names only (pass ls) — the runner that needs a secret reads it; list the entry in ~/.config/ai-gent/pass-allow to let the agent read it"
    fi
    decide deny "that shows a secret value from pass; the agent sees names only (pass ls) — the runner that needs a secret reads it"
    ;;
esac
