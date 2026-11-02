#!/bin/sh
# dev-bin.sh — put the ai-gent binary in bin/ of this checkout, for contributors.
#
#   scripts/dev-bin.sh              build rust/ with cargo; without cargo, download the release binary
#   scripts/dev-bin.sh --build      build only (fails without cargo)
#   scripts/dev-bin.sh --download   download only
#
# Hooks and setup.sh look for <checkout>/bin/ai-gent (a package carries it there too), so
# after this a checkout behaves like an installed package for the binary. bin/ is git-ignored.
#
# The download takes the release matching the checkout: VERSION (environment), else the
# newest release tag reachable from HEAD. It is checked against the release's SHA256SUMS
# before it is used; nothing is written to bin/ on a mismatch.
#
# Environment:
#   VERSION               x.y.z to download instead of the newest tag of this checkout
#   AI_GENT_RELEASE_BASE  where releases live (default: the GitHub releases URL); file:// works
#   AI_GENT_TARGET        platform triple, instead of the one detected from uname
#
# POSIX sh, like install.sh. After a build, run `scripts/dev-bin.sh` again to refresh it.

set -eu

SLUG="lucasvmigotto/ai-gent"
root="$(cd "$(dirname "$0")/.." && pwd)"

say() { printf 'dev-bin: %s\n' "$*"; }
die() { printf 'dev-bin: error: %s\n' "$*" >&2; exit 1; }

mode=auto
for a in "$@"; do
  case "$a" in
    --build) mode=build ;;
    --download) mode=download ;;
    -h | --help) sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown argument '$a' (see --help)" ;;
  esac
done

detect_target() {
  if [ -n "${AI_GENT_TARGET:-}" ]; then printf '%s' "$AI_GENT_TARGET"; return; fi
  os="$(uname -s)"; arch="$(uname -m)"
  case "$os/$arch" in
    Linux/x86_64) printf 'x86_64-unknown-linux-musl' ;;
    Linux/aarch64 | Linux/arm64) printf 'aarch64-unknown-linux-musl' ;;
    Darwin/arm64) printf 'aarch64-apple-darwin' ;;
    Darwin/x86_64)
      if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then printf 'aarch64-apple-darwin'; else printf 'x86_64-apple-darwin'; fi ;;
    MINGW*/x86_64 | MSYS*/x86_64 | CYGWIN*/x86_64) printf 'x86_64-pc-windows-msvc' ;;
    MINGW*/aarch64 | MSYS*/aarch64 | CYGWIN*/aarch64 | MINGW*/arm64 | MSYS*/arm64 | CYGWIN*/arm64) printf 'aarch64-pc-windows-msvc' ;;
    *) die "unsupported platform $os/$arch" ;;
  esac
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | cut -d' ' -f1
  else die "need sha256sum or shasum to verify the download"; fi
}

install_bin() { # install_bin <built or downloaded file>
  mkdir -p "$root/bin"
  cp "$1" "$root/bin/$exe.new.$$"
  chmod 755 "$root/bin/$exe.new.$$"
  mv -f "$root/bin/$exe.new.$$" "$root/bin/$exe"
  say "bin/$exe is ready"
}

case "$(detect_target)" in *-windows-msvc) exe=ai-gent.exe ;; *) exe=ai-gent ;; esac

do_build() {
  command -v cargo >/dev/null 2>&1 || die "cargo is not installed (rustup.rs); use --download for the release binary"
  say "building rust/ (cargo build --release --locked)"
  (cd "$root/rust" && cargo build --release --locked -p ai-gent) || die "the build failed"
  [ -f "$root/rust/target/release/$exe" ] || die "the build left no rust/target/release/$exe"
  install_bin "$root/rust/target/release/$exe"
}

do_download() {
  command -v curl >/dev/null 2>&1 || die "curl is required to download the binary"
  target="$(detect_target)"
  ver="${VERSION:-}"
  if [ -z "$ver" ]; then
    ver="$(git -C "$root" describe --tags --abbrev=0 --match '[0-9]*.[0-9]*.[0-9]*' 2>/dev/null || true)"
  fi
  [ -n "$ver" ] || die "no release version: set VERSION=x.y.z (this checkout has no release tag)"
  case "$ver" in '' | *[!0-9.]*) die "not a release version: '$ver'" ;; esac
  base="${AI_GENT_RELEASE_BASE:-https://github.com/$SLUG/releases/download}"
  case "$exe" in ai-gent.exe) ext=zip ;; *) ext=tar.gz ;; esac
  name="ai-gent-$ver-$target.$ext"
  work="$(mktemp -d "${TMPDIR:-/tmp}/ai-gent-dev-bin.XXXXXX")"
  trap 'rm -rf "$work"' EXIT INT TERM
  say "downloading $base/$ver/$name"
  curl -fsSL --proto '=https,file' --max-time 300 -o "$work/SHA256SUMS" "$base/$ver/SHA256SUMS" || die "could not download $base/$ver/SHA256SUMS"
  want="$(awk -v f="$name" '$2 == f || $2 == "*" f { print $1; exit }' "$work/SHA256SUMS")"
  [ -n "$want" ] || die "release $ver has no archive for $target (SHA256SUMS lists no $name); the binary ships from 2.27, or use --build"
  curl -fsSL --proto '=https,file' --max-time 300 -o "$work/$name" "$base/$ver/$name" || die "could not download $name"
  got="$(sha256 "$work/$name")"
  [ "$want" = "$got" ] || die "checksum mismatch for $name (expected $want, got $got); bin/ was not touched"
  say "checksum ok ($got)"
  mkdir "$work/x"
  if [ "$ext" = zip ]; then
    command -v unzip >/dev/null 2>&1 || die "unzip is required"
    unzip -q "$work/$name" -d "$work/x" || die "extraction failed"
  else
    tar -xzf "$work/$name" -C "$work/x" || die "extraction failed"
  fi
  found="$(find "$work/x" -path "*/bin/$exe" -type f | head -n 1)"
  [ -n "$found" ] || die "the archive has no bin/$exe"
  install_bin "$found"
}

case "$mode" in
  build) do_build ;;
  download) do_download ;;
  auto) if command -v cargo >/dev/null 2>&1; then do_build; else say "cargo not found; downloading the release binary"; do_download; fi ;;
esac
say "run scripts/test-guard.sh or any guard test next; setup.sh links bin/$exe when it is there"
