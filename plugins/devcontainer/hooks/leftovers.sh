#!/bin/sh
# SessionStart hook: report this project's container leftovers and a low disk,
# so they're seen the same day (shared/containers.md §9a). Report only — it
# never removes anything: a hook can't tell whether a build or a parallel
# track is still using what it sees. Silent when there's nothing to say, no
# engine, or not a git checkout. AI_GENT_NO_LEFTOVERS_CHECK=1 disables it.

set -u
[ "${AI_GENT_NO_LEFTOVERS_CHECK:-}" = 1 ] && exit 0
cat >/dev/null 2>&1 || true # the hook event on stdin isn't needed

engine="${CONTAINER_ENGINE:-}"
if [ -z "$engine" ]; then
  if command -v podman >/dev/null 2>&1; then engine=podman
  elif command -v docker >/dev/null 2>&1; then engine=docker
  else exit 0
  fi
fi

common="$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null)" || exit 0
[ -n "$common" ] || exit 0
top="$(dirname "$common")"
project="$(basename "$top" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//')"
label="me.lucasvmigotto.ai-gent.project=$project"

lines=""
say() { lines="$lines$1
"; }

store="$("$engine" info --format '{{.Store.GraphRoot}}' 2>/dev/null || "$engine" info --format '{{.DockerRootDir}}' 2>/dev/null)"
[ -n "$store" ] && [ -e "$store" ] || store=/
# shellcheck disable=SC2046 # split df's columns on purpose
set -- $(df -Pk "$store" 2>/dev/null | awk 'NR == 2 { print $2, $4 }')
if [ $# -eq 2 ]; then
  total_gb=$(($1 / 1000000)) free_gb=$(($2 / 1000000))
  floor=$((total_gb / 10))
  [ "$floor" -lt 20 ] && floor=20
  if [ "$free_gb" -lt "$floor" ]; then
    say "disk: ${free_gb} GB free on the container store ($store), under the ${floor} GB floor; don't start parallel builders (containers.md §9a)"
  fi
fi

count() { "$@" 2>/dev/null | grep -c . || true; }

n="$(count "$engine" images -q --filter dangling=true --filter "label=$label")"
[ "$n" -gt 0 ] && say "$n dangling image(s) of $project: \`wt.py finish\` or the project's \`clean\` recipe removes them"

live="$(git worktree list --porcelain 2>/dev/null | sed -n "s#^worktree $top/.worktrees/##p")"
orphans=0
for v in $("$engine" volume ls -q --filter "label=$label" 2>/dev/null); do
  wt="$("$engine" volume inspect --format '{{index .Labels "me.lucasvmigotto.ai-gent.worktree"}}' "$v" 2>/dev/null)"
  slug="${wt#"$project"/}"
  [ -n "$wt" ] && [ "$slug" != main ] || continue
  printf '%s\n' "$live" | grep -qx "$slug" || orphans=$((orphans + 1))
done
[ "$orphans" -gt 0 ] && say "$orphans volume(s) of $project belong to worktrees that are gone: \`wt.py resources\` lists them; ask before removing"

if [ "${engine##*/}" = podman ]; then
  n="$("$engine" ps -a --external --format '{{.Status}}' 2>/dev/null | grep -ci storage || true)"
  [ "$n" -gt 0 ] && say "$n buildah working container(s) in the store (interrupted builds, any project): \`wt.py resources\`; only those new since \`wt.py baseline\` are this run's"
fi

[ -n "$lines" ] || exit 0
printf 'ai-gent containers (report only, nothing removed):\n%s' "$lines"
exit 0
