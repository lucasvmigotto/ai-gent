#!/usr/bin/env bash
# Tests scripts/rust-publish.sh with a fake cargo and a file:// stand-in for the crates.io sparse
# index: dependency order, an index wait after each crate, idempotence (a version already in the
# index is skipped), the metadata gate stopping a publish, a timeout, the dry run, and that a
# token in the environment never reaches the output. Nothing is published. Called by scripts/check.sh.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PUBLISH="$REPO_DIR/scripts/rust-publish.sh"
SANDBOX="$(mktemp -d)"
trap 'rm -rf "$SANDBOX"' EXIT

failures=0
check() {
  local what="$1"
  shift
  if "$@"; then printf '      ok    %s\n' "$what"; else printf '      FAIL  %s\n' "$what"; failures=$((failures + 1)); fi
}

# A workspace of three crates: core, dbrun (a library from spec 038) and the binary.
mkws() { # mkws <dir>
  local d="$1" c
  for c in ai-gent-core ai-gent-dbrun ai-gent; do
    mkdir -p "$d/crates/$c"
    printf '[package]\nname = "%s"\nversion = "1.2.3"\n' "$c" >"$d/crates/$c/Cargo.toml"
  done
}
# The fake cargo. `package` writes a .crate whose Cargo.toml has the fields the gate wants (HOMEPAGE_IN_CRATE overrides
# the homepage); `publish` logs the crate and adds its version to the index unless NO_INDEX is set.
cat >"$SANDBOX/cargo" <<'SH'
#!/usr/bin/env bash
echo "cargo $*" >>"$FAKE_LOG"
case "$1" in
  package)
    for d in crates/*/; do
      n="$(basename "$d")"
      mkdir -p "target/package" "$SANDBOX_TMP/$n-1.2.3"
      printf '[package]\nname = "%s"\nversion = "1.2.3"\ndescription = "d"\nhomepage = "%s"\nrepository = "https://github.com/lucasvmigotto/ai-gent"\nreadme = "README.md"\nkeywords = ["a"]\ncategories = ["b"]\nlicense = "GPL-3.0-or-later"\n' \
        "$n" "${HOMEPAGE_IN_CRATE:-https://docs.lucasvmigotto.me/ai-gent/}" >"$SANDBOX_TMP/$n-1.2.3/Cargo.toml"
      tar -czf "target/package/$n-1.2.3.crate" -C "$SANDBOX_TMP" "$n-1.2.3/Cargo.toml"
    done ;;
  publish)
    case " $* " in *" --dry-run "*) exit 0 ;; esac
    while [ $# -gt 0 ]; do [ "$1" = -p ] && crate="$2"; shift; done
    echo "published $crate" >>"$FAKE_LOG"
    if [ -z "${NO_INDEX:-}" ]; then
      case "${#crate}" in
        *) path="$(printf '%s' "$crate" | cut -c1-2)/$(printf '%s' "$crate" | cut -c3-4)/$crate" ;;
      esac
      mkdir -p "$(dirname "$FAKE_INDEX/$path")"
      echo '{"name":"'"$crate"'","vers":"1.2.3"}' >>"$FAKE_INDEX/$path"
    fi ;;
esac
SH
chmod +x "$SANDBOX/cargo"

run() { # run <env...> — sets rc and out; each run gets a fresh workspace, log and index
  rm -rf "$SANDBOX/ws" "$SANDBOX/index" "$SANDBOX/tmp"
  mkws "$SANDBOX/ws"
  mkdir -p "$SANDBOX/index" "$SANDBOX/tmp"
  : >"$SANDBOX/log"
  set +e
  out="$(cd "$SANDBOX" && env RUST_DIR="$SANDBOX/ws" CARGO="$SANDBOX/cargo" CRATES_INDEX="file://$SANDBOX/index" FAKE_LOG="$SANDBOX/log" \
    FAKE_INDEX="$SANDBOX/index" SANDBOX_TMP="$SANDBOX/tmp" PUBLISH_WAIT_SECS=2 PUBLISH_POLL_SECS=1 "$@" 2>&1)"
  rc=$?
  set -e
}
published() { grep '^published ' "$SANDBOX/log" | sed 's/^published //' | tr '\n' ' '; }

echo "      -- publish"
run bash "$PUBLISH" publish
check "publishes core, dbrun, then the binary, each after the index shows it" test "$rc" -eq 0 -a "$(published)" == "ai-gent-core ai-gent-dbrun ai-gent "
check "  metadata is checked first (cargo package --workspace --locked --no-verify)" bash -c 'head -n1 "$1" | grep -q "^cargo package --workspace --locked --no-verify"' _ "$SANDBOX/log"
check "  every publish passes --locked and -p" bash -c '! grep "^cargo publish" "$1" | grep -qv -- "--locked -p"' _ "$SANDBOX/log"

# Pre-seed the index: core already published at 1.2.3.
run env PRESEED=1 bash -c '
  mkdir -p "$FAKE_INDEX/ai/-g/" && mkdir -p "$FAKE_INDEX/ai/-g"
  echo "{\"name\":\"ai-gent-core\",\"vers\":\"1.2.3\"}" >"$FAKE_INDEX/ai/-g/ai-gent-core"
  exec bash '"$PUBLISH"' publish'
check "idempotent: a version already in the index is skipped" test "$rc" -eq 0 -a "$(published)" == "ai-gent-dbrun ai-gent "
check "  and says so" grep -q "ai-gent-core 1.2.3 is already on crates.io; skipped" <<<"$out"

run env PRESEED=1 bash -c '
  for c in ai-gent-core ai-gent-dbrun ai-gent; do
    p="$FAKE_INDEX/$(printf %s "$c" | cut -c1-2)/$(printf %s "$c" | cut -c3-4)"; mkdir -p "$p"
    echo "{\"name\":\"$c\",\"vers\":\"1.2.3\"}" >"$p/$c"
  done
  exec bash '"$PUBLISH"' publish'
check "everything already published: nothing is published, exit 0" test "$rc" -eq 0 -a -z "$(published)"

run env HOMEPAGE_IN_CRATE=https://example.test/ bash "$PUBLISH" publish
check "the metadata gate stops a publish when the homepage is wrong" test "$rc" -ne 0 -a -z "$(published)"
check "  and names the crate and the field" grep -q "FAIL ai-gent-core: homepage is not https://docs.lucasvmigotto.me/ai-gent/" <<<"$out"

run env NO_INDEX=1 bash "$PUBLISH" publish
check "an index that never shows the version fails after the wait, before the next crate" test "$rc" -ne 0 -a "$(published)" == "ai-gent-core "

run env CARGO_REGISTRY_TOKEN=TOKENVALUE123 bash "$PUBLISH" publish
check "a token in the environment is never printed or logged" bash -c '! grep -q TOKENVALUE123 "$1" && [[ "$2" != *TOKENVALUE123* ]]' _ "$SANDBOX/log" "$out"

echo "      -- dry run and metadata"
run bash "$PUBLISH" dry-run
check "dry-run packages everything, then dry-runs only the first library" test "$rc" -eq 0 -a -z "$(published)" -a "$(grep -c '^cargo publish --dry-run --locked -p ai-gent-core' "$SANDBOX/log")" -eq 1
check "  and prints each packaged Cargo.toml" grep -q "== ai-gent 1.2.3: packaged Cargo.toml" <<<"$out"
run bash "$PUBLISH" metadata
check "metadata alone publishes and dry-runs nothing" test "$rc" -eq 0 -a "$(grep -c '^cargo publish' "$SANDBOX/log")" -eq 0
run bash "$PUBLISH" bogus
check "an unknown mode → exit 2" test "$rc" -eq 2

exit $((failures > 0))
