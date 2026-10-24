#!/usr/bin/env bash
# Tests plugins/devcontainer/hooks/guard.sh with hook-event JSON for commands
# that must be denied or left alone, and plugins/devcontainer/hooks/leftovers.sh
# against a fake engine (it reports and never removes). Called by scripts/check.sh.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GUARD="$REPO_DIR/plugins/devcontainer/hooks/guard.sh"
LEFTOVERS="$REPO_DIR/plugins/devcontainer/hooks/leftovers.sh"

failures=0
expect() { # expect deny|none <command>
  local want="$1" cmd="$2" out got
  out="$(python3 -c 'import json,sys; print(json.dumps({"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":"/tmp","tool_input":{"command":sys.argv[1]}}))' "$cmd" | sh "$GUARD")"
  got="$(printf '%s' "$out" | sed -n 's/.*"permissionDecision":"\([a-z]*\)".*/\1/p')"
  got="${got:-none}"
  if [[ "$got" == "$want" ]]; then
    printf '      ok    %-5s %s\n' "$want" "${cmd//$'\n'/⏎}"
  else
    printf '      FAIL  want %s, got %s: %s\n' "$want" "$got" "${cmd//$'\n'/⏎}"
    failures=$((failures + 1))
  fi
}

# Any prune, through any engine, even filtered.
expect deny 'podman system prune -a -f'
expect deny 'docker image prune'
expect deny 'podman image prune --filter label=me.lucasvmigotto.ai-gent.project=shop'
expect deny 'podman volume prune -f'
expect deny 'docker builder prune --all'
expect deny 'nerdctl container prune'
expect deny '"$CONTAINER_ENGINE" system prune'
expect deny '$CONTAINER_ENGINE network prune'
expect deny 'sudo podman --log-level=info system prune'
expect deny 'buildah prune'
expect deny 'cd /x && /usr/bin/podman image prune -a'
# Removing everything.
expect deny 'podman rm -a -f'
expect deny 'podman rmi --all'
expect deny 'podman volume rm -af'
expect deny 'docker container rm --all'
expect deny 'buildah rm --all'
expect deny 'buildah rmi --prune'
# Removal fed by an unfiltered listing.
expect deny 'podman rmi $(podman images -q)'
expect deny 'podman images -q --filter dangling=true | xargs podman rmi'
expect deny 'docker volume ls -q | xargs -r docker volume rm'
expect deny 'for v in $(podman volume ls -q); do podman volume rm $v; done'
expect deny 'podman rm -f `podman ps -aq`'
# Scoped removal, listings and builds pass.
expect none 'podman images -q --filter dangling=true --filter label=me.lucasvmigotto.ai-gent.project=shop | xargs -r podman rmi'
expect none 'podman ps -aq --filter name=scratch-uv | xargs podman rm -f'
expect none 'podman volume rm shop-tools-target-feat-a'
expect none 'podman rmi 1a2b3c4d'
expect none 'podman rm -f scratch-probe'
expect none '"$CONTAINER_ENGINE" rmi localhost/vsc-shop-1234-features'
expect none 'podman ps -a --external'
expect none 'podman system df'
expect none 'podman images -q'
expect none 'podman build --label me.lucasvmigotto.ai-gent.project=shop --target tools -t localhost/shop-tools:dev .'
expect none 'podman run --rm --name scratch-x img true'
expect none 'git fetch --prune'
expect none 'git worktree prune'
expect none 'git commit -m "docs: never podman system prune"'
expect none 'grep -rn "image prune" shared/'
expect none 'ls -la'

# leftovers.sh: reports, removes nothing, silent with nothing to say.
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
git init -q -b main "$tmp/shop"
cat >"$tmp/engine" <<'FAKE'
#!/bin/sh
echo "$*" >>"$FAKE_LOG"
case "$*" in
  "info --format {{.Store.GraphRoot}}") echo "$FAKE_STORE" ;;
  images\ -q\ --filter\ dangling=true*) [ -n "${FAKE_DANGLING:-}" ] && printf 'i1\ni2\n' ;;
  "volume ls -q --filter label=me.lucasvmigotto.ai-gent.project=shop") [ -n "${FAKE_DANGLING:-}" ] && echo v-gone ;;
  volume\ inspect*) echo "shop/feat-gone" ;;
  "ps -a --external --format {{.Status}}") [ -n "${FAKE_DANGLING:-}" ] && echo Storage ;;
esac
exit 0
FAKE
chmod +x "$tmp/engine"
leftovers() { (cd "$tmp/shop" && CONTAINER_ENGINE="$tmp/engine" FAKE_LOG="$tmp/log" FAKE_STORE="$tmp" sh "$LEFTOVERS" </dev/null); }
check() { # check <label> <condition result>
  if [[ "$2" == 0 ]]; then printf '      ok    leftovers: %s\n' "$1"; else printf '      FAIL  leftovers: %s\n' "$1"; failures=$((failures + 1)); fi
}
out="$(FAKE_DANGLING=1 leftovers)"
grep -q '2 dangling image(s) of shop' <<<"$out"; check "dangling images counted" $?
grep -q '1 volume(s) of shop belong to worktrees that are gone' <<<"$out"; check "orphan volume found" $?
! grep -Eq '(^| )(rm|rmi|prune)( |$)' "$tmp/log"; check "nothing removed or pruned" $?
: >"$tmp/log"
out="$(leftovers)"
if [[ "$(df -Pk "$tmp" | awk 'NR == 2 { print ($4 / 1000000 >= 20 && $4 * 10 >= $2) }')" == 1 ]]; then
  [[ -z "$out" ]]; check "silent when clean" $?
fi
out="$(AI_GENT_NO_LEFTOVERS_CHECK=1 FAKE_DANGLING=1 leftovers)"
[[ -z "$out" ]]; check "AI_GENT_NO_LEFTOVERS_CHECK=1 disables it" $?

exit $((failures > 0))
