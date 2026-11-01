#!/usr/bin/env bash
# Publish the rust/ crates to crates.io, idempotently and in dependency order (spec 032 T033,
# D23, D28): the library crates (ai-gent-core, then ai-gent-dbrun once it exists), each after an
# index wait, then the binary crate `ai-gent`. Used by release.yml's publish-crates job and by
# rust.yml's dry run. A real publish needs CARGO_REGISTRY_TOKEN (the Trusted Publishing token the
# crates-io-auth-action step puts there); this script never reads, prints or stores it.
#
#   scripts/rust-publish.sh metadata   package every crate (no verify) and check the metadata
#   scripts/rust-publish.sh dry-run    metadata, then `cargo publish --dry-run` for the first
#                                      library crate (the binary's dry run needs that crate
#                                      on crates.io at its version; `cargo package --workspace`
#                                      in the metadata step covers it from the workspace)
#   scripts/rust-publish.sh publish    for each crate: skip when crates.io already has the version,
#                                      else `cargo publish --locked` and wait for the index
#
# Environment (for tests): CARGO, CURL (commands), CRATES_INDEX (default https://index.crates.io),
# PUBLISH_WAIT_SECS (default 600), PUBLISH_POLL_SECS (default 10), HOMEPAGE, REPOSITORY.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO="${CARGO:-cargo}"
CURL="${CURL:-curl}"
CRATES_INDEX="${CRATES_INDEX:-https://index.crates.io}"
WAIT="${PUBLISH_WAIT_SECS:-600}"
POLL="${PUBLISH_POLL_SECS:-10}"
HOMEPAGE="${HOMEPAGE:-https://docs.lucasvmigotto.me/ai-gent/}"
REPOSITORY="${REPOSITORY:-https://github.com/lucasvmigotto/ai-gent}"
BINARY=ai-gent
RUST_DIR="${RUST_DIR:-$REPO_DIR/rust}"

mode="${1:-}"
shift || true
EXTRA=()
for a in "$@"; do
  case "$a" in
    --allow-dirty) EXTRA+=(--allow-dirty) ;;
    *) echo "usage: scripts/rust-publish.sh metadata|dry-run|publish [--allow-dirty]" >&2; exit 2 ;;
  esac
done
case "$mode" in metadata | dry-run | publish) ;; *) echo "usage: scripts/rust-publish.sh metadata|dry-run|publish [--allow-dirty]" >&2; exit 2 ;; esac

# toml_value <Cargo.toml> <key> — the [package] value of a key (strings only)
toml_value() {
  awk -v k="$2" '/^\[/ { in_pkg = ($0 == "[package]") } in_pkg && $1 == k && $2 == "=" { v = $0; sub(/^[^=]*= */, "", v); gsub(/^"|"$/, "", v); print v; exit }' "$1"
}

# crates — "name version" per line in publish order: libraries by name, then the binary
crates() {
  local f name version libs="" bin=""
  for f in "$RUST_DIR"/crates/*/Cargo.toml; do
    [[ -f $f ]] || continue
    name="$(toml_value "$f" name)"
    version="$(toml_value "$f" version)"
    [[ -n $name && -n $version ]] || { echo "rust-publish.sh: $f has no [package] name and version" >&2; exit 1; }
    if [[ $name == "$BINARY" ]]; then bin="$name $version"; else libs+="$name $version"$'\n'; fi
  done
  [[ -n $bin ]] || { echo "rust-publish.sh: no crate named $BINARY under $RUST_DIR/crates" >&2; exit 1; }
  printf '%s' "$libs" | sort
  printf '%s\n' "$bin"
}

# index_path <name> — the sparse index path of a crate
index_path() {
  local n="$1" lower
  lower="$(printf '%s' "$n" | tr '[:upper:]' '[:lower:]')"
  case "${#lower}" in
    1) printf '1/%s' "$lower" ;;
    2) printf '2/%s' "$lower" ;;
    3) printf '3/%s/%s' "${lower:0:1}" "$lower" ;;
    *) printf '%s/%s/%s' "${lower:0:2}" "${lower:2:2}" "$lower" ;;
  esac
}

# in_index <name> <version> — 0 when the sparse index lists that version (and it is not yanked-only knowledge: listed is enough)
in_index() {
  local body
  body="$("$CURL" -fsS --proto '=https,file' --max-time 30 "$CRATES_INDEX/$(index_path "$1")" 2>/dev/null)" || return 1
  printf '%s\n' "$body" | grep -q "\"vers\":\"$2\""
}

wait_for_index() {
  local waited=0
  until in_index "$1" "$2"; do
    if ((waited >= WAIT)); then
      echo "rust-publish.sh: $1 $2 is not in the index after ${WAIT}s" >&2
      return 1
    fi
    sleep "$POLL"
    waited=$((waited + POLL))
  done
}

# metadata — package everything, print each packaged Cargo.toml, and fail unless the fields crates.io shows are right
metadata() {
  local name version crate toml status=0
  (cd "$RUST_DIR" && "$CARGO" package --workspace --locked --no-verify ${EXTRA[@]+"${EXTRA[@]}"})
  while read -r name version; do
    crate="$RUST_DIR/target/package/$name-$version.crate"
    [[ -f $crate ]] || { echo "rust-publish.sh: $crate was not produced" >&2; status=1; continue; }
    toml="$(tar -xzOf "$crate" "$name-$version/Cargo.toml")"
    printf '== %s %s: packaged Cargo.toml\n%s\n' "$name" "$version" "$toml"
    printf '%s\n' "$toml" | grep -Fxq "homepage = \"$HOMEPAGE\"" || { echo "FAIL $name: homepage is not $HOMEPAGE" >&2; status=1; }
    printf '%s\n' "$toml" | grep -Fxq "repository = \"$REPOSITORY\"" || { echo "FAIL $name: repository is not $REPOSITORY" >&2; status=1; }
    for key in license readme keywords categories description; do
      printf '%s\n' "$toml" | grep -Eq "^$key = " || { echo "FAIL $name: $key is missing from the packaged Cargo.toml" >&2; status=1; }
    done
    if tar -tzf "$crate" | grep -Eq '(^|/)(target|rust-toolchain\.toml|deny\.toml)(/|$)'; then
      echo "FAIL $name: the crate holds build output or repository config" >&2
      status=1
    fi
  done < <(crates)
  return "$status"
}

case "$mode" in
  metadata) metadata ;;
  dry-run)
    metadata
    first="$(crates | sed -n '1s/ .*//p')"
    echo "== cargo publish --dry-run -p $first"
    (cd "$RUST_DIR" && "$CARGO" publish --dry-run --locked -p "$first" ${EXTRA[@]+"${EXTRA[@]}"})
    ;;
  publish)
    metadata
    while read -r name version; do
      if in_index "$name" "$version"; then
        echo "== $name $version is already on crates.io; skipped"
        continue
      fi
      echo "== publishing $name $version"
      (cd "$RUST_DIR" && "$CARGO" publish --locked -p "$name")
      wait_for_index "$name" "$version"
      echo "== $name $version is in the index"
    done < <(crates)
    ;;
esac
