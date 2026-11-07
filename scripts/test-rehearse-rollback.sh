#!/usr/bin/env bash
# Tests scripts/rehearse-rollback.sh (spec 039 T051) with no network: the dry run prints the
# plan and starts nothing, every refusal exits 2 before gh is called, and --run against a fake
# gh (state in a temp dir) walks the T051 sequence, writes a transcript and never deletes.

set -euo pipefail
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_DIR/scripts/rehearse-rollback.sh"
SANDBOX="$(mktemp -d)"
trap 'rm -rf "$SANDBOX"' EXIT
failures=0
check() { local what="$1"; shift; if "$@"; then printf '      ok    %s\n' "$what"; else printf '      FAIL  %s\n' "$what"; failures=$((failures + 1)); fi; }

# a fake gh: releases kept in $FAKE_STATE/rel ("tag prerelease"), the latest tag in $FAKE_STATE/latest
mkdir -p "$SANDBOX/bin" "$SANDBOX/state"
cat >"$SANDBOX/bin/gh" <<'FAKE'
#!/bin/sh
printf '%s\n' "$*" >>"$FAKE_STATE/calls"
touch "$FAKE_STATE/rel" "$FAKE_STATE/latest"
case "$1 $2" in
  "auth status") echo "Logged in (fake)"; exit 0 ;;
  "repo view") echo "${FAKE_CANON:-$3}"; exit 0 ;;
  "release view") grep -q "^$3 " "$FAKE_STATE/rel" ;;
  "release create") echo "$3 0" >>"$FAKE_STATE/rel"; echo "$3" >"$FAKE_STATE/latest"; echo "https://example.invalid/$3" ;;
  "release edit")
    tag="$3"; shift 3
    for a in "$@"; do
      case "$a" in
        --prerelease) sed -i "s/^$tag .*/$tag 1/" "$FAKE_STATE/rel"
          if [ "$(cat "$FAKE_STATE/latest")" = "$tag" ]; then awk '$2==0 && $1!="'"$tag"'" {print $1; exit}' "$FAKE_STATE/rel" >"$FAKE_STATE/latest"; fi ;;
        --prerelease=false) sed -i "s/^$tag .*/$tag 0/" "$FAKE_STATE/rel" ;;
        --latest) echo "$tag" >"$FAKE_STATE/latest" ;;
      esac
    done ;;
  "api repos/"*) cat "$FAKE_STATE/latest" ;;
  *) echo "fake gh: unexpected: $*" >&2; exit 9 ;;
esac
FAKE
chmod +x "$SANDBOX/bin/gh"
export FAKE_STATE="$SANDBOX/state"
fresh() { rm -f "$FAKE_STATE"/*; }
calls() { cat "$FAKE_STATE/calls" 2>/dev/null || true; }

# a PATH with the fake gh first; a PATH with no gh at all for the missing-gh refusal
FAKE_PATH="$SANDBOX/bin:$PATH"
cd "$SANDBOX"

# --- dry run: prints the plan, never starts gh
fresh
out="$(PATH="$FAKE_PATH" sh "$SCRIPT")"
check "dry run is the default and exits 0" test -n "$out"
check "dry run prints --prerelease" grep -q 'gh release edit 9.9.1 -R <owner/scratch-repo> --prerelease$' <<<"$out"
check "dry run prints --latest" grep -q 'gh release edit 9.9.0 -R <owner/scratch-repo> --latest$' <<<"$out"
check "dry run says what install.sh resolves" grep -q 'install.sh should resolve: 9.9.0' <<<"$out"
check "dry run never started gh" test -z "$(calls)"
check "dry run left no files" test "$(find "$SANDBOX" -maxdepth 1 -type f | wc -l)" = 0
out="$(PATH="$FAKE_PATH" sh "$SCRIPT" --good 1.0.0 --bad 1.0.1 me/my-scratch)"
check "dry run takes versions and a repository" grep -q 'gh release edit 1.0.1 -R me/my-scratch --prerelease$' <<<"$out"
check "dry run printed no delete" bash -c '! grep -qi "gh .*delete" <<<"$1"' _ "$out"

# --- refusals (exit 2, gh never called)
refuses() { # <description> <args...>
  local what="$1" rc=0 err; shift
  err="$(PATH="$FAKE_PATH" sh "$SCRIPT" "$@" 2>&1)" || rc=$?
  check "refuses $what" test "$rc" = 2
  check "  said so ($what)" grep -q 'refus\|unknown option' <<<"$err"
}
fresh
refuses "--run without a repository" --run
refuses "--run followed by an option" --run --good 1.0.0
refuses "the real project" --run lucasvmigotto/ai-gent
refuses "the real project in other case" --run LucasVMigotto/AI-Gent
refuses "any repository called ai-gent" --run someone/ai-gent
refuses "a repository without scratch in its name" --run me/some-repo
refuses "a malformed repository" --run 'scratch'
refuses "a repository with a path" --run 'me/scratch/x'
refuses "a bad good version" --run me/x-scratch --good v1
refuses "equal versions" --run me/x-scratch --good 1.0.0 --bad 1.0.0
refuses "an unknown option" --nope
refuses "the real project in a dry run" lucasvmigotto/ai-gent
check "no refusal reached gh" test -z "$(calls)"
mkdir -p "$SANDBOX/nogh"
for t in grep tr sed date tee cat; do ln -sf "$(command -v "$t")" "$SANDBOX/nogh/$t"; done
rc=0; err="$(PATH="$SANDBOX/nogh" /bin/sh "$SCRIPT" --run me/x-scratch 2>&1)" || rc=$?
check "refuses when gh is missing (exit 2)" test "$rc" = 2
check "  and says gh" grep -q "gh is not installed" <<<"$err"
echo old >"$SANDBOX/t.txt"
rc=0; PATH="$FAKE_PATH" sh "$SCRIPT" --run me/x-scratch --transcript "$SANDBOX/t.txt" >/dev/null 2>&1 || rc=$?
check "refuses to overwrite a transcript (exit 2)" test "$rc" = 2
check "  and left it alone" test "$(cat "$SANDBOX/t.txt")" = old
check "  and never called gh" test -z "$(calls)"
fresh
rc=0; FAKE_CANON=lucasvmigotto/ai-gent PATH="$FAKE_PATH" sh "$SCRIPT" --run me/x-scratch --transcript "$SANDBOX/c.txt" >/dev/null 2>&1 || rc=$?
check "refuses a scratch name that resolves to the real project" test "$rc" = 2
check "  before any release command" bash -c '! grep -q "^release" "$1"' _ "$FAKE_STATE/calls"

# --- --run against the fake gh
fresh
rc=0; PATH="$FAKE_PATH" sh "$SCRIPT" --run me/x-scratch --transcript "$SANDBOX/run.txt" >"$SANDBOX/run.out" 2>&1 || rc=$?
check "--run exits 0" test "$rc" = 0
check "transcript was written" test -s "$SANDBOX/run.txt"
check "transcript ends with the result" grep -q 'RESULT: every expectation held' "$SANDBOX/run.txt"
check "creates the two releases" test "$(grep -c '^release create' "$FAKE_STATE/calls")" = 2
check "ran --prerelease" grep -qx 'release edit 9.9.1 -R me/x-scratch --prerelease' "$FAKE_STATE/calls"
check "ran --latest on the good release" grep -qx 'release edit 9.9.0 -R me/x-scratch --latest' "$FAKE_STATE/calls"
check "resolved the bad release first, then the good one" bash -c 'grep "resolves:" "$1" | sed "s/.*resolves: \([^ ]*\).*/\1/" | tr "\n" " " | grep -q "^9.9.1 9.9.0 9.9.0 9.9.1 "' _ "$SANDBOX/run.txt"
check "the final state is the armed one (undo)" test "$(cat "$FAKE_STATE/latest")" = 9.9.1
check "never called delete" bash -c '! grep -qi "delete" "$1"' _ "$FAKE_STATE/calls"
check "every gh call named the scratch repo" bash -c '! grep "^release\|^api" "$1" | grep -qv "me/x-scratch"' _ "$FAKE_STATE/calls"
check "never touched the real project" bash -c '! grep -rq "lucasvmigotto/ai-gent" "$1" "$2"' _ "$FAKE_STATE/calls" "$SANDBOX/run.txt"
# a second run reuses the existing releases and creates nothing
rc=0; PATH="$FAKE_PATH" sh "$SCRIPT" --run me/x-scratch --transcript "$SANDBOX/run2.txt" >/dev/null 2>&1 || rc=$?
check "a second run exits 0 and creates nothing more" bash -c 'test "$(grep -c "^release create" "$1")" = 2' _ "$FAKE_STATE/calls"

# --- an expectation that does not hold gives exit 1 and says MISMATCH
fresh
sed 's/^        --latest) echo "\$tag" >"\$FAKE_STATE\/latest" ;;/        --latest) : ;;/' "$SANDBOX/bin/gh" >"$SANDBOX/bin/gh.new"
mv "$SANDBOX/bin/gh.new" "$SANDBOX/bin/gh"; chmod +x "$SANDBOX/bin/gh"
rc=0; PATH="$FAKE_PATH" sh "$SCRIPT" --run me/x-scratch --transcript "$SANDBOX/bad.txt" >/dev/null 2>&1 || rc=$?
check "a failed expectation exits 1" test "$rc" = 1
check "  and the transcript says MISMATCH" grep -q 'MISMATCH' "$SANDBOX/bad.txt"

echo
if [ "$failures" -eq 0 ]; then echo "test-rehearse-rollback: all checks passed"; else echo "test-rehearse-rollback: $failures failed"; exit 1; fi
