#!/bin/sh
# PreToolUse guard for the Bash tool: container resources are removed by ID,
# own prefix or this project's label, never by a broad sweep
# (shared/containers.md §9, §9a). A prune or an "everything" removal deletes
# other projects' images, caches and in-flight builds on a shared host.
#
#   deny  any `prune` through podman, docker, nerdctl, buildah or
#         $CONTAINER_ENGINE (system, image, volume, container, builder,
#         network, even filtered)
#   deny  rm / rmi / volume rm with -a/--all (buildah rmi --prune too)
#   deny  a removal fed by a quiet listing (`ps -q`, `images -q`,
#         `volume ls -q`) when the command names no label=, name= or id=
#         filter
#   none  everything else: removal by ID, by label, listings, builds
#
# Pattern matching on the raw command: it catches the commands an agent
# writes, not every possible obfuscation.
#
# In a package install (a VERSION file two levels above the plugin) the
# launcher below hands the event to `ai-gent guard --guard containers` and turns a
# missing, failing or slow binary into an ask (spec 033); in a clone it falls
# through to the shell logic, which is also the oracle the binary's fixtures
# are checked against.

# >>> guard launcher (shared/guard-launcher.sh, spec 033: package installs call the binary; a clone runs the shell logic below)
r="${CLAUDE_PLUGIN_ROOT:-${0%/*}/..}"
if [ -f "$r/../../VERSION" ]; then
  b="$r/../../bin/ai-gent"; [ -x "$b" ] || b="$(command -v ai-gent 2>/dev/null)" || b=
  rc=0 out=
  if [ -z "$b" ]; then rc=127; else
    set -- "$b" guard --guard containers --format claude
    command -v timeout >/dev/null 2>&1 && set -- timeout 2 "$@"
    out="$("$@" 2>/dev/null)" || rc=$?
  fi
  if [ "$rc" != 0 ]; then
    case "$rc" in 127) w="it is missing" ;; 124) w="it took over 2 s" ;; *) w="it failed with exit $rc" ;; esac
    printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"containers guard: the ai-gent binary is unavailable (%s); run: curl -fsSL https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.sh | sh. Allow only if you accept this command unguarded."}}\n' "$w"
  elif [ -n "$out" ]; then printf '%s\n' "$out"; fi
  exit 0
fi
# <<< guard launcher

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
    ( printf '%s' "$input" | "$b" guard --guard containers --shadow --shell-decision none >/dev/null 2>&1 & ) >/dev/null 2>&1
  else
    ( printf '%s' "$input" | "$b" guard --guard containers --shadow --shell-decision "$sd" --shell-reason "$sr" >/dev/null 2>&1 & ) >/dev/null 2>&1
  fi
  return 0
}
trap shadow EXIT

decide() { # decide deny <reason>
  sd="$1" sr="containers guard: $2"
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"%s","permissionDecisionReason":"containers guard: %s"}}\n' "$1" "$2"
  exit 0
}

cmd="$(printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p')"
[ -n "$cmd" ] || exit 0

# "$CONTAINER_ENGINE" stays a word; other quoted strings are blanked (so text
# inside quotes can't look like a command); split on ; & | ( ) ` and newlines.
segments="$(printf '%s\n' "$cmd" | awk '{
  gsub(/\\"\$\{?CONTAINER_ENGINE\}?\\"/, "$CONTAINER_ENGINE")
  gsub(/\\"([^\\]|\\[^"])*\\"/, "\"\""); gsub(/\047[^\047]*\047/, "\047\047")
  gsub(/\\n/, "\n"); gsub(/[;&|()`]/, "\n"); print }')"

prefix='^[[:space:]]*((sudo([[:space:]]+-[^[:space:]]+([[:space:]]+[^-[:space:]][^[:space:]]*)?)*|env|exec|nice|do|then|else|xargs([[:space:]]+-[^[:space:]]+)*|timeout[[:space:]]+[0-9smh]+|[A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*)[[:space:]]+)*'
engine='(([^[:space:]]*/)?(podman|docker|nerdctl)|\$\{?CONTAINER_ENGINE\}?)'
buildah='([^[:space:]]*/)?buildah'
words='([[:space:]]+[^[:space:]]+)*'

has() { printf '%s\n' "$segments" | grep -Eq "$1"; }

if has "$prefix($engine|$buildah)${words}[[:space:]]+prune([[:space:]]|\$)"; then
  decide deny "prune removes other projects' images, volumes and in-flight builds; remove by ID or by this project's label (containers.md §9a: images -q --filter dangling=true --filter label=me.lucasvmigotto.ai-gent.project=<project>, then rmi each ID)"
fi

removal="$prefix(${engine}[[:space:]]+((container|image|volume)[[:space:]]+)?(rm|rmi)|${buildah}[[:space:]]+(rm|rmi))"
remove="$removal([[:space:]]|\$)"
if has "$removal${words}[[:space:]]+(--all|-[a-zA-Z]*a[a-zA-Z]*)([[:space:]]|\$)" ||
  has "$prefix${buildah}[[:space:]]+rmi${words}[[:space:]]+(--prune|-p)([[:space:]]|\$)"; then
  decide deny "removing everything takes other projects' containers, images or volumes; remove by ID or by this project's label (containers.md §9)"
fi

listing="$prefix${engine}[[:space:]]+((container|image|volume)[[:space:]]+)?(ps|ls|list|images)${words}[[:space:]]+(--quiet|-[a-zA-Z]*q[a-zA-Z]*)([[:space:]]|\$)"
if has "$remove" && has "$listing" && ! printf '%s' "$cmd" | grep -Eq '(label|name|id)='; then
  decide deny "that removes whatever an unfiltered listing returns, other projects' included; filter the listing by label=me.lucasvmigotto.ai-gent.project=<project> (or your own name= prefix) (containers.md §9a)"
fi

exit 0
