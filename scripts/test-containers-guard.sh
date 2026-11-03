#!/usr/bin/env bash
# Tests plugins/devcontainer/hooks/guard.sh through the shared fixtures, and plugins/devcontainer/hooks/leftovers.sh
# against a fake engine (it reports and never removes). Called by scripts/check.sh.

set -euo pipefail
# the guards' shadow call (spec 033) would log these cases from a binary on PATH
export AI_GENT_NO_SHADOW=1

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LEFTOVERS="$REPO_DIR/plugins/devcontainer/hooks/leftovers.sh"

failures=0
# the guard cases: shared/guard-fixtures/containers.json (spec 033)
python3 "$REPO_DIR/scripts/guard-fixtures.py" run --guard containers --impl sh || failures=$((failures + 1))

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
check() { # check <label> <command...>: the command must succeed
  local label="$1"
  shift
  if "$@"; then printf '      ok    leftovers: %s\n' "$label"; else printf '      FAIL  leftovers: %s\n' "$label"; failures=$((failures + 1)); fi
}
removed() { grep -Eq '(^| )(rm|rmi|prune)( |$)' "$tmp/log"; }
out="$(FAKE_DANGLING=1 leftovers)"
check "dangling images counted" grep -q '2 dangling image(s) of shop' <<<"$out"
check "orphan volume found" grep -q '1 volume(s) of shop belong to worktrees that are gone' <<<"$out"
check "nothing removed or pruned" test "$(removed && echo yes)" != yes
: >"$tmp/log"
out="$(leftovers)"
if [[ "$(df -Pk "$tmp" | awk 'NR == 2 { print ($4 / 1000000 >= 20 && $4 * 10 >= $2) }')" == 1 ]]; then
  check "silent when clean" test -z "$out"
fi
out="$(AI_GENT_NO_LEFTOVERS_CHECK=1 FAKE_DANGLING=1 leftovers)"
check "AI_GENT_NO_LEFTOVERS_CHECK=1 disables it" test -z "$out"


# --json: one object whenever it ran; the default text output is unchanged.
leftovers_json() { (cd "$tmp/shop" && CONTAINER_ENGINE="$tmp/engine" FAKE_LOG="$tmp/log" FAKE_STORE="$tmp" sh "$LEFTOVERS" --json </dev/null); }
# shellcheck disable=SC2329 # called through check
jq3() { python3 -c 'import json,sys; d=json.load(sys.stdin); exec(sys.argv[1])' "$1"; }
: >"$tmp/log"
out="$(FAKE_DANGLING=1 leftovers_json)"
check "--json lists dangling, volumes in order with counts" jq3 'k=[f["kind"] for f in d["findings"] if f["kind"]!="disk"]; assert d["checked"] is True and d["project"]=="shop" and k==["dangling","volumes"], d; c={f["kind"]:f["count"] for f in d["findings"] if "count" in f}; assert c=={"dangling":2,"volumes":1}, c' <<<"$out"
check "--json carries no sentences" test -z "$(grep -E 'removes them|ask before' <<<"$out")"
check "--json removes nothing" test "$(removed && echo yes)" != yes
out="$(leftovers_json)"
check "--json is always an object with a findings array" jq3 'assert d["checked"] is True and isinstance(d["findings"], list) and all(f["kind"]=="disk" for f in d["findings"]), d' <<<"$out"
out="$(AI_GENT_NO_LEFTOVERS_CHECK=1 leftovers_json)"
check "--json disabled says so" jq3 'assert d=={"checked": False, "reason": "disabled"}, d' <<<"$out"
out="$(cd "$tmp" && CONTAINER_ENGINE="$tmp/engine" FAKE_LOG="$tmp/log" GIT_CEILING_DIRECTORIES="$tmp" sh "$LEFTOVERS" --json </dev/null)"
check "--json outside a checkout says so" jq3 'assert d=={"checked": False, "reason": "not-a-checkout"}, d' <<<"$out"
out="$(cd "$tmp/shop" && CONTAINER_ENGINE='' PATH=/nonexistent /bin/sh "$LEFTOVERS" --json </dev/null)"
check "--json without an engine says so" jq3 'assert d=={"checked": False, "reason": "no-engine"}, d' <<<"$out"

exit $((failures > 0))
