#!/usr/bin/env bash
# Tests scripts/update-check.sh and scripts/ai-gent-update.sh in a throwaway
# HOME: version comparison, tag/slug resolution, the cached notice (fresh,
# stale, missing, pinned, opted-out, offline), the update script's
# clean/dirty/pinned/already-at cases, and that the SessionStart hook
# command resolves to an existing file. Called by scripts/check.sh.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SANDBOX="$(mktemp -d)"
trap 'rm -rf "$SANDBOX"' EXIT

export HOME="$SANDBOX/home"
export XDG_STATE_HOME="$SANDBOX/state"
mkdir -p "$HOME" "$XDG_STATE_HOME"
unset AI_GENT_NO_UPDATE_CHECK AI_GENT_UPDATE_DAYS AI_GENT_UPDATE_REPO AI_GENT_UPDATE_ROOT
export STUBBIN="$SANDBOX/bin"
mkdir -p "$STUBBIN"
export PATH="$STUBBIN:$PATH"

failures=0
check() { # check <description> <command...>
  local what="$1"
  shift
  if "$@"; then
    printf '      ok    %s\n' "$what"
  else
    printf '      FAIL  %s\n' "$what"
    failures=$((failures + 1))
  fi
}

expect_notice() { # expect_notice <desc> <env-assignments...>: exactly one notice line
  local desc="$1"
  shift
  local out
  out="$(env "$@" AI_GENT_UPDATE_ROOT="$REPO" bash "$REPO_DIR/scripts/update-check.sh")"
  if [[ "$(printf '%s' "$out" | grep -c 'update available')" == 1 ]]; then
    printf '      ok    %s\n' "$desc"
  else
    printf '      FAIL  %s (got: %q)\n' "$desc" "$out"
    failures=$((failures + 1))
  fi
}

expect_silent() { # expect_silent <desc> <env-assignments...>: no output at all
  local desc="$1"
  shift
  local out
  out="$(env "$@" AI_GENT_UPDATE_ROOT="$REPO" bash "$REPO_DIR/scripts/update-check.sh")"
  if [[ -z "$out" ]]; then
    printf '      ok    %s\n' "$desc"
  else
    printf '      FAIL  %s (got: %q)\n' "$desc" "$out"
    failures=$((failures + 1))
  fi
}

# Fake curl: always reports release 9.9.9.
cat >"$STUBBIN/curl" <<'STUBEOF'
#!/usr/bin/env bash
printf '%s' '{"tag_name": "9.9.9"}'
STUBEOF
chmod +x "$STUBBIN/curl"

make_repo() { # make_repo <dir> [tag...]: git repo with optional tags
  local dir="$1"
  mkdir -p "$dir"
  git -C "$dir" init -q
  git -C "$dir" config user.email t@t
  git -C "$dir" config user.name t
  git -C "$dir" commit -q --allow-empty -m init
  shift
  for tag in "$@"; do git -C "$dir" tag "$tag"; done
}

REPO="$SANDBOX/repo"
make_repo "$REPO" 1.1.0 1.2.0

check "version comparison orders triples" bash -c "
  source '$REPO_DIR/scripts/update-check.sh'
  _ai_gent_ver_gt 1.3.0 1.2.0 && _ai_gent_ver_gt 1.10.0 1.9.9 &&
  _ai_gent_ver_gt v2.0.0 1.9.9 && ! _ai_gent_ver_gt 1.2.0 1.2.0 &&
  ! _ai_gent_ver_gt 1.2.0 1.3.0 && ! _ai_gent_ver_gt 1.2 1.2.0 &&
  _ai_gent_ver_gt 1.2.1 1.2"

check "local tag resolves newest tag" bash -c "
  source '$REPO_DIR/scripts/update-check.sh'
  [[ \"\$(_ai_gent_local_tag '$REPO')\" == 1.2.0 ]]"

check "slug prefers override, then origin, then default" bash -c "
  source '$REPO_DIR/scripts/update-check.sh'
  [[ \"\$(AI_GENT_UPDATE_REPO=someone/else _ai_gent_repo_slug '$REPO')\" == someone/else ]] &&
  git -C '$REPO' remote add origin 'git@github.com:acme/widget.git' &&
  [[ \"\$(_ai_gent_repo_slug '$REPO')\" == acme/widget ]] &&
  git -C '$REPO' remote remove origin &&
  [[ \"\$(_ai_gent_repo_slug '$REPO')\" == lucasvmigotto/ai-gent ]]"

# Stale cache + stubbed curl: exactly one notice naming both versions.
# (The cached remote must be newer than the local 1.2.0 to notify.)
OLD=$(( $(date +%s) - 30 * 86400 ))
mkdir -p "$XDG_STATE_HOME/ai-gent"
printf '%s 9.9.9\n' "$OLD" >"$XDG_STATE_HOME/ai-gent/update-check"
OUT="$(AI_GENT_UPDATE_ROOT="$REPO" bash "$REPO_DIR/scripts/update-check.sh")"
if [[ "$(printf '%s' "$OUT" | grep -c 'update available')" == 1 ]] \
  && [[ "$OUT" == *"1.2.0 → 9.9.9"* ]]; then
  printf '      ok    stale cache prints one notice with both versions\n'
else
  printf '      FAIL  stale cache notice (got: %q)\n' "$OUT"
  failures=$((failures + 1))
fi

# A stale cache forks a detached refresh; drain it by removing the cache
# and waiting for the fork to rewrite it — proving no refresh is still in
# flight, so later cases see a deterministic cache.
drain_refresh() {
  rm -f "$XDG_STATE_HOME/ai-gent/update-check"
  for _ in $(seq 1 50); do
    [[ -f "$XDG_STATE_HOME/ai-gent/update-check" ]] || { sleep 0.1; continue; }
    [[ "$(cut -d' ' -f2 <"$XDG_STATE_HOME/ai-gent/update-check")" == 9.9.9 ]] && return 0
    sleep 0.1
  done
  return 1
}

# Missing cache: silent, and the detached refresh writes the cache.
check "stale refresh landed" drain_refresh
rm -f "$XDG_STATE_HOME/ai-gent/update-check"
expect_silent "missing cache stays silent"
for _ in $(seq 1 50); do
  [[ -s "$XDG_STATE_HOME/ai-gent/update-check" ]] && break
  sleep 0.1
done
check "detached refresh populates the cache" test -s "$XDG_STATE_HOME/ai-gent/update-check"

# Fresh cache with equal versions: silent.
printf '%s 1.2.0\n' "$(date +%s)" >"$XDG_STATE_HOME/ai-gent/update-check"
expect_silent "fresh cache at same version stays silent"

# Pinned checkout (detached at a tag): silent even with a newer remote.
git -C "$REPO" checkout -q --detach 1.1.0
printf '%s 9.9.9\n' "$OLD" >"$XDG_STATE_HOME/ai-gent/update-check"
expect_silent "pinned checkout stays silent"
# (no drain here: the pinned path returns before reading the cache, so it
# forks no refresh — and the stale/poll cases above already drained theirs)
git -C "$REPO" checkout -q main 2>/dev/null || git -C "$REPO" checkout -q master 2>/dev/null || git -C "$REPO" checkout -q --detach 1.2.0

# Opt-out and offline: silent.
expect_silent "opt-out stays silent" AI_GENT_NO_UPDATE_CHECK=1
mkdir -p "$SANDBOX/nocurl"
cat >"$SANDBOX/nocurl/curl" <<'STUBEOF'
#!/usr/bin/env bash
exit 7
STUBEOF
chmod +x "$SANDBOX/nocurl/curl"
rm -f "$XDG_STATE_HOME/ai-gent/update-check"
PATH="$SANDBOX/nocurl:/usr/bin:/bin" expect_silent "offline refresh stays silent"

# --- ai-gent-update.sh ---
ORIGIN="$SANDBOX/origin.git"
git init -q --bare "$ORIGIN"
git clone -q "$ORIGIN" "$SANDBOX/clone" 2>/dev/null
git -C "$SANDBOX/clone" config user.email t@t
git -C "$SANDBOX/clone" config user.name t
git -C "$SANDBOX/clone" commit -q --allow-empty -m init
git -C "$SANDBOX/clone" tag 1.0.0
git -C "$SANDBOX/clone" push -q origin HEAD:refs/heads/main 1.0.0 2>/dev/null || git -C "$SANDBOX/clone" push -q origin HEAD:refs/heads/master 1.0.0
git -C "$SANDBOX/clone" commit -q --allow-empty -m second
git -C "$SANDBOX/clone" tag 2.0.0
git -C "$SANDBOX/clone" push -q origin HEAD 2.0.0

WORK="$SANDBOX/work"
git clone -q "$ORIGIN" "$WORK" 2>/dev/null
git -C "$WORK" checkout -q 1.0.0 2>/dev/null || true
git -C "$WORK" checkout -q --detach 1.0.0
if OUT="$(AI_GENT_UPDATE_ROOT="$WORK" bash "$REPO_DIR/scripts/ai-gent-update.sh" 2>&1)"; then
  printf '      FAIL  pinned update without ref should refuse\n'
  failures=$((failures + 1))
elif [[ "$OUT" == *"pinned"* ]]; then
  printf '      ok    pinned update without ref refuses\n'
else
  printf '      FAIL  pinned refusal text (got: %q)\n' "$OUT"
  failures=$((failures + 1))
fi

BRANCH="$(git -C "$WORK" symbolic-ref --short HEAD 2>/dev/null || git -C "$WORK" branch --show-current)"
if [[ -z "$BRANCH" ]]; then
  git -C "$WORK" checkout -q -b main origin/HEAD 2>/dev/null || git -C "$WORK" checkout -q -b main
  git -C "$WORK" reset -q --hard 1.0.0
fi
OUT="$(AI_GENT_UPDATE_ROOT="$WORK" bash "$REPO_DIR/scripts/ai-gent-update.sh" 2>&1)"
if [[ "$OUT" == *"updated 1.0.0 → 2.0.0"* ]]; then
  printf '      ok    clean checkout fast-forwards to newest tag\n'
else
  printf '      FAIL  clean update (got: %q)\n' "$OUT"
  failures=$((failures + 1))
fi
OUT="$(AI_GENT_UPDATE_ROOT="$WORK" bash "$REPO_DIR/scripts/ai-gent-update.sh" 2>&1)"
if [[ "$OUT" == *"already at 2.0.0"* ]]; then
  printf '      ok    already-at reports without changing\n'
else
  printf '      FAIL  already-at (got: %q)\n' "$OUT"
  failures=$((failures + 1))
fi
touch "$WORK/dirty-file"
AI_GENT_UPDATE_ROOT="$WORK" bash "$REPO_DIR/scripts/ai-gent-update.sh" >/dev/null 2>&1 && {
  printf '      FAIL  dirty checkout should refuse\n'
  failures=$((failures + 1))
} || printf '      ok    dirty checkout refuses\n'

# --- hook wiring ---
HOOK_CMD="$(python3 -c "import json; print(json.load(open('$REPO_DIR/plugins/git/hooks/hooks.json'))['hooks']['SessionStart'][0]['hooks'][0]['command'])")"
HOOK_PATH="${HOOK_CMD#bash }"
HOOK_PATH="${HOOK_PATH%\"}"
HOOK_PATH="${HOOK_PATH#\"}"
HOOK_PATH="${HOOK_PATH//\$\{CLAUDE_PLUGIN_ROOT\}/$REPO_DIR/plugins/git}"
check "SessionStart hook command resolves" test -x "$HOOK_PATH"

if ((failures)); then
  printf '%d check(s) failed\n' "$failures" >&2
  exit 1
fi
