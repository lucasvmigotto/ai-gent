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

set -u

input="$(tr '\n' ' ')"

decide() { # decide deny <reason>
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
