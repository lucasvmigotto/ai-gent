#!/usr/bin/env bash
# Shadow mode in the five shell guards (spec 033 FR-009, T042): after the decision is printed, the
# guard hands the event to the installed ai-gent binary, detached. Checked here with fake binaries
# on PATH: the output and exit status stay exactly the shell guard's, a missing, failing, noisy or
# slow binary changes nothing and costs no wait, and AI_GENT_NO_SHADOW=1 turns the call off.
# Called by scripts/check.sh.

set -euo pipefail
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SANDBOX="$(mktemp -d)"
trap 'rm -rf "$SANDBOX"' EXIT
unset AI_GENT_NO_SHADOW CLAUDE_PLUGIN_ROOT

failures=0
check() { local what="$1"; shift; if "$@"; then printf '      ok    %s\n' "$what"; else printf '      FAIL  %s\n' "$what"; failures=$((failures + 1)); fi; }

# guard id | script | a command the guard decides on (deny or ask) | the same decision's marker
GUARDS=(
  "git|plugins/git/hooks/guard.sh|git commit --no-verify -m x|git:workflow guard: "
  "db|plugins/db/hooks/guard.sh|psql -c 'DROP TABLE t'|db guard: "
  "secrets|plugins/secrets/hooks/guard.sh|cat ~/.password-store/ai-gent/db/x.gpg; pass show ai-gent/db/x|secrets guard: "
  "repo|plugins/repo/hooks/guard.sh|python3 reporun.py issue create --write --title x|repo guard: "
  "containers|plugins/devcontainer/hooks/guard.sh|docker system prune -af|containers guard: "
)

# fake binaries: ok (records argv and stdin), noisy (prints and exits 1), slow (sleeps)
mkdir -p "$SANDBOX/ok" "$SANDBOX/noisy" "$SANDBOX/slow" "$SANDBOX/none"
cat >"$SANDBOX/ok/ai-gent" <<'STUB'
#!/bin/sh
{ printf '%s\n' "$*"; cat >"$SHADOW_LOG.in"; } >"$SHADOW_LOG.tmp" 2>&1
mv "$SHADOW_LOG.tmp" "$SHADOW_LOG"
STUB
cat >"$SANDBOX/noisy/ai-gent" <<'STUB'
#!/bin/sh
echo '{"hookSpecificOutput":{"permissionDecision":"allow"}}'
echo oops >&2
exit 1
STUB
cat >"$SANDBOX/slow/ai-gent" <<'STUB'
#!/bin/sh
sleep 3
STUB
chmod +x "$SANDBOX"/*/ai-gent

event() { python3 -c 'import json,sys; print(json.dumps({"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":sys.argv[1],"tool_input":{"command":sys.argv[2]}}))' "$SANDBOX" "$1"; }
# run <script> <path dir or ""> <event> [env...] -> stdout, exit status in $RC
run() {
  local script="$1" dir="$2" ev="$3"
  shift 3
  RC=0
  OUT="$(printf '%s' "$ev" | env PATH="${dir:+$dir:}$BASEPATH" "$@" sh "$REPO_DIR/$script")" || RC=$?
}
wait_for() { local f="$1"; for _ in $(seq 1 100); do [[ -s "$f" ]] && return 0; sleep 0.05; done; return 1; }

# a PATH without any real ai-gent
mkdir -p "$SANDBOX/base"
for t in sh env cat tr sed awk grep head python3 printf dirname git mv sleep; do
  p="$(command -v "$t" 2>/dev/null || true)"
  [[ -n "$p" && "$p" = /* ]] && ln -sf "$p" "$SANDBOX/base/$t"
done
BASEPATH="$SANDBOX/base"

for row in "${GUARDS[@]}"; do
  IFS='|' read -r gid script cmd marker <<<"$row"
  ev="$(event "$cmd")"
  neutral="$(event 'ls -la')"
  printf '%s\n' "  $gid"

  run "$script" "" "$ev"
  base_out="$OUT"; base_rc=$RC
  check "$gid: the command is decided (not allowed outright)" test -n "$base_out"
  decision="$(printf '%s' "$base_out" | sed -n 's/.*"permissionDecision":"\([a-z]*\)".*/\1/p')"

  export SHADOW_LOG="$SANDBOX/$gid.log"
  rm -f "$SHADOW_LOG" "$SHADOW_LOG.in"
  run "$script" "$SANDBOX/ok" "$ev" SHADOW_LOG="$SHADOW_LOG"
  check "$gid: output and exit status unchanged with the binary on PATH" test "$OUT" = "$base_out" -a "$RC" = "$base_rc"
  if wait_for "$SHADOW_LOG"; then
    check "$gid: the binary got --guard $gid --shadow and the shell decision" grep -q -- "^guard --guard $gid --shadow --shell-decision $decision --shell-reason $marker" "$SHADOW_LOG"
    check "$gid: the binary got the event on stdin" grep -q '"command"' "$SHADOW_LOG.in"
  else
    check "$gid: the shadow call ran" false
  fi

  rm -f "$SHADOW_LOG" "$SHADOW_LOG.in"
  run "$script" "$SANDBOX/ok" "$neutral" SHADOW_LOG="$SHADOW_LOG"
  check "$gid: an allowed command prints nothing" test -z "$OUT" -a "$RC" = 0
  wait_for "$SHADOW_LOG" || true
  check "$gid: an allowed command is shadowed with --shell-decision none" grep -q -- "--shell-decision none" "$SHADOW_LOG"

  rm -f "$SHADOW_LOG"
  run "$script" "$SANDBOX/ok" "$ev" SHADOW_LOG="$SHADOW_LOG" AI_GENT_NO_SHADOW=1
  sleep 0.3
  check "$gid: AI_GENT_NO_SHADOW=1 skips the call" test ! -e "$SHADOW_LOG" -a "$OUT" = "$base_out"

  run "$script" "$SANDBOX/none" "$ev"
  check "$gid: no binary: same output, silent" test "$OUT" = "$base_out" -a "$RC" = "$base_rc"

  run "$script" "$SANDBOX/noisy" "$ev"
  check "$gid: a binary that prints and exits 1 changes nothing" test "$OUT" = "$base_out" -a "$RC" = "$base_rc"
  run "$script" "$SANDBOX/noisy" "$neutral"
  check "$gid: a noisy binary does not turn an allow into output" test -z "$OUT" -a "$RC" = 0

  start=$(date +%s%N)
  run "$script" "$SANDBOX/slow" "$ev"
  ms=$((($(date +%s%N) - start) / 1000000))
  check "$gid: a slow binary does not delay the hook (${ms} ms)" test "$OUT" = "$base_out" -a "$RC" = "$base_rc" -a "$ms" -lt 1500
done

# the package layout: the binary next to the plugin (bin/ai-gent) is used without PATH
PKG="$SANDBOX/pkg"
mkdir -p "$PKG/bin" "$PKG/plugins"
cp -R "$REPO_DIR/plugins/git" "$PKG/plugins/git"
cp "$SANDBOX/ok/ai-gent" "$PKG/bin/ai-gent"
export SHADOW_LOG="$SANDBOX/pkg.log"
rm -f "$SHADOW_LOG"
OUT="$(printf '%s' "$(event 'git commit --no-verify -m x')" | env PATH="$BASEPATH" SHADOW_LOG="$SHADOW_LOG" CLAUDE_PLUGIN_ROOT="$PKG/plugins/git" sh "$PKG/plugins/git/hooks/guard.sh")"
check "package: bin/ai-gent beside the plugin is called" wait_for "$SHADOW_LOG"

if ((failures > 0)); then
  printf '%d failure(s)\n' "$failures"
  exit 1
fi
