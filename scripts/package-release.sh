#!/usr/bin/env bash
# Build every asset of one release into a directory: the content-only package (2.x), one
# archive per target from the binaries a rust-build run uploaded, install.sh and SHA256SUMS
# (all through scripts/package.py), then the release notes with the `## Checksums` block
# (spec 032 FR-023, D25). release.yml's `package` job and rust.yml's rehearsal both run
# this script, so a rehearsal exercises the code the release runs. It publishes nothing.
#
#   scripts/package-release.sh --version 2.28.0 --bins bins --out dist [--notes body.md]
#
#   --bins DIR   holds bin-<triple>/ai-gent[.exe] (what download-artifact lays out)
#   --notes FILE the notes body (release.py --notes); without it a one-line placeholder
#
# Every target must have its binary, except aarch64-pc-windows-msvc, whose runner leg is
# allowed to fail before the 3.0.0 gate (rust-build.yml, continue-on-error): it is skipped
# with a warning and then has no archive and no line in SHA256SUMS.
# Writes into --out: ai-gent-<ver>.tar.gz, ai-gent-<ver>-<triple>.tar.gz|zip, install.sh,
# SHA256SUMS, LATEST and release-notes.md.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGETS=(x86_64-unknown-linux-musl aarch64-unknown-linux-musl aarch64-apple-darwin x86_64-apple-darwin x86_64-pc-windows-msvc aarch64-pc-windows-msvc)
OPTIONAL=" aarch64-pc-windows-msvc "

version="" bins="" out="" notes=""
while (($#)); do
  case "$1" in
    --version) version="${2:-}"; shift ;;
    --bins) bins="${2:-}"; shift ;;
    --out) out="${2:-}"; shift ;;
    --notes) notes="${2:-}"; shift ;;
    *) echo "usage: scripts/package-release.sh --version X.Y.Z --bins DIR --out DIR [--notes FILE]" >&2; exit 2 ;;
  esac
  shift
done
[[ -n $version && -n $bins && -n $out ]] || { echo "package-release.sh: --version, --bins and --out are required" >&2; exit 2; }
[[ -d $bins ]] || { echo "package-release.sh: $bins is not a directory" >&2; exit 1; }

# A stale file (an older SHA256SUMS, a leftover archive) must not slip into a release.
if [[ -e $out && -n "$(ls -A "$out")" ]]; then
  echo "package-release.sh: $out is not empty; give it a fresh directory" >&2
  exit 1
fi
mkdir -p "$out"
python3 "$REPO_DIR/scripts/package.py" --version "$version" --out "$out"
for t in "${TARGETS[@]}"; do
  exe=ai-gent
  case "$t" in *-windows-msvc) exe=ai-gent.exe ;; esac
  bin="$bins/bin-$t/$exe"
  if [[ ! -f $bin ]]; then
    if [[ $OPTIONAL == *" $t "* ]]; then
      echo "::warning::no binary for $t in $bins; its archive is skipped"
      continue
    fi
    echo "package-release.sh: no binary for $t ($bin)" >&2
    exit 1
  fi
  python3 "$REPO_DIR/scripts/package.py" --version "$version" --out "$out" --target "$t" --binary "$bin"
done

(cd "$out" && { sha256sum -c SHA256SUMS || shasum -a 256 -c SHA256SUMS; })

{
  if [[ -n $notes ]]; then cat "$notes"; else echo "Release $version."; fi
  printf '\n## Checksums\n\n'
  printf 'SHA256 of the release assets, so they can be checked without trusting the download host (`sha256sum -c`):\n\n'
  printf '```text\n'
  cat "$out/SHA256SUMS"
  printf '```\n'
} >"$out/release-notes.md"

echo "assets in $out:"
(cd "$out" && ls -l)
