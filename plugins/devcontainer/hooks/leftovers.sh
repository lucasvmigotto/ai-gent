#!/bin/sh
# SessionStart hook: report this project's container leftovers and a low disk,
# so they're seen the same day (shared/containers.md §9a). Report only — it
# never removes anything: a hook can't tell whether a build or a parallel
# track is still using what it sees. Silent when there's nothing to say, no
# engine, or not a git checkout. AI_GENT_NO_LEFTOVERS_CHECK=1 disables it.
#
# --json: always prints one JSON object when it ran, counts and numbers only:
#   {"checked":true,"project":"p","findings":[{"kind":"disk","free_gb":N,
#    "floor_gb":F,"store":"/x"},{"kind":"dangling","count":N},
#    {"kind":"volumes","count":N},{"kind":"buildah","count":N}]}
# (findings empty when clean), or {"checked":false,"reason":"no-engine" |
# "not-a-checkout" | "disabled"} when it could not check.

set -u
json=0
[ "${1:-}" = --json ] && json=1
bail() { # bail <reason>: nothing to report; --json says why it couldn't check
  [ "$json" = 1 ] && printf '{"checked":false,"reason":"%s"}\n' "$1"
  exit 0
}
[ "${AI_GENT_NO_LEFTOVERS_CHECK:-}" = 1 ] && bail disabled
[ "$json" = 1 ] || cat >/dev/null 2>&1 || true # the hook event on stdin isn't needed

engine="${CONTAINER_ENGINE:-}"
if [ -z "$engine" ]; then
  if command -v podman >/dev/null 2>&1; then engine=podman
  elif command -v docker >/dev/null 2>&1; then engine=docker
  else bail no-engine
  fi
fi

common="$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null)" || bail not-a-checkout
[ -n "$common" ] || bail not-a-checkout
top="$(dirname "$common")"
project="$(basename "$top" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//')"
label="me.lucasvmigotto.ai-gent.project=$project"

lines=""
found=""
say() { lines="$lines$1
"; }
add() { found="${found:+$found,}$1"; } # add <json object>

store="$("$engine" info --format '{{.Store.GraphRoot}}' 2>/dev/null || "$engine" info --format '{{.DockerRootDir}}' 2>/dev/null)"
[ -n "$store" ] && [ -e "$store" ] || store=/
# shellcheck disable=SC2046 # split df's columns on purpose
set -- $(df -Pk "$store" 2>/dev/null | awk 'NR == 2 { print $2, $4 }')
if [ $# -eq 2 ]; then
  total_gb=$(($1 / 1000000)) free_gb=$(($2 / 1000000))
  floor=$((total_gb / 10))
  [ "$floor" -lt 20 ] && floor=20
  if [ "$free_gb" -lt "$floor" ]; then
    add "{\"kind\":\"disk\",\"free_gb\":$free_gb,\"floor_gb\":$floor,\"store\":\"$(printf '%s' "$store" | sed 's/[\\"]/\\&/g')\"}"
    say "disk: ${free_gb} GB free on the container store ($store), under the ${floor} GB floor; don't start parallel builders (containers.md §9a)"
  fi
fi

count() { "$@" 2>/dev/null | grep -c . || true; }

n="$(count "$engine" images -q --filter dangling=true --filter "label=$label")"
[ "$n" -gt 0 ] && add "{\"kind\":\"dangling\",\"count\":$n}"
[ "$n" -gt 0 ] && say "$n dangling image(s) of $project: \`wt.py finish\` or the project's \`clean\` recipe removes them"

live="$(git worktree list --porcelain 2>/dev/null | sed -n "s#^worktree $top/.worktrees/##p")"
orphans=0
for v in $("$engine" volume ls -q --filter "label=$label" 2>/dev/null); do
  wt="$("$engine" volume inspect --format '{{index .Labels "me.lucasvmigotto.ai-gent.worktree"}}' "$v" 2>/dev/null)"
  slug="${wt#"$project"/}"
  [ -n "$wt" ] && [ "$slug" != main ] || continue
  printf '%s\n' "$live" | grep -qx "$slug" || orphans=$((orphans + 1))
done
[ "$orphans" -gt 0 ] && add "{\"kind\":\"volumes\",\"count\":$orphans}"
[ "$orphans" -gt 0 ] && say "$orphans volume(s) of $project belong to worktrees that are gone: \`wt.py resources\` lists them; ask before removing"

if [ "${engine##*/}" = podman ]; then
  n="$("$engine" ps -a --external --format '{{.Status}}' 2>/dev/null | grep -ci storage || true)"
  [ "$n" -gt 0 ] && add "{\"kind\":\"buildah\",\"count\":$n}"
  [ "$n" -gt 0 ] && say "$n buildah working container(s) in the store (interrupted builds, any project): \`wt.py resources\`; only those new since \`wt.py baseline\` are this run's"
fi

if [ "$json" = 1 ]; then
  printf '{"checked":true,"project":"%s","findings":[%s]}\n' "$project" "$found"
  exit 0
fi
[ -n "$lines" ] || exit 0
printf 'ai-gent containers (report only, nothing removed):\n%s' "$lines"
exit 0
