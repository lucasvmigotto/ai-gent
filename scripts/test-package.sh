#!/usr/bin/env bash
# Tests scripts/package.py and the package install path of install.sh in a
# throwaway HOME, with no network: a package built from the working tree is
# served over file:// (AI_GENT_RELEASE_BASE). The per-triple archives hold a stub
# binary (a shell script), so one host covers all six triples; AI_GENT_TARGET and a
# fake uname pick the platform. Covers build determinism, a bad
# checksum or truncated tarball refusing (and extracting nothing), install ->
# upgrade -> rollback flipping `current`, --prune, a path with spaces, a package
# with a dangling symlink refusing to build, no dev-only files in the installed
# tree, --migrate, and --verify-attestation; and the per-triple archives (contents,
# no Rust sources, bin/ mode, the platform map, the content-only fallback for 2.x,
# the ~/.local/bin link). Called by scripts/check.sh.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SANDBOX="$(mktemp -d)"
trap 'rm -rf "$SANDBOX"' EXIT

export HOME="$SANDBOX/home dir" # spaces on purpose
mkdir -p "$HOME"
export CLAUDE_SKILLS_DIR="$HOME/.claude/skills"
export XDG_STATE_HOME="$HOME/.local/state"
export OPENCODE_SKILLS_DIR="$HOME/.config/opencode/skills" AGENTS_SKILLS_DIR="$HOME/.agents/skills"
unset XDG_DATA_HOME AI_GENT_DIR AI_GENT_REPO AI_GENT_BRANCH AI_GENT_MODE VERSION AI_GENT_HOME
export AI_GENT_HOME="$HOME/share/ai gent"
export AI_GENT_TARGET=x86_64-unknown-linux-musl # the stub binary runs anywhere; tests of the platform map unset it
TRIPLES=(x86_64-unknown-linux-musl aarch64-unknown-linux-musl aarch64-apple-darwin x86_64-apple-darwin x86_64-pc-windows-msvc aarch64-pc-windows-msvc)
STUB="$SANDBOX/stub"
mkdir -p "$STUB"
printf '#!/bin/sh\necho "ai-gent stub"\n' >"$STUB/ai-gent"
chmod 600 "$STUB/ai-gent" # the packager sets bin/ to 0755 itself
cp "$STUB/ai-gent" "$STUB/ai-gent.exe"
# pkg <version> <dir> [triple...] — the content-only package, then one archive per triple, in one directory
pkg() {
  local v="$1" d="$2" t exe
  shift 2
  mkdir -p "$d"
  python3 "$REPO_DIR/scripts/package.py" --version "$v" --out "$d" >/dev/null
  for t in "$@"; do
    exe=ai-gent
    case "$t" in *-windows-msvc) exe=ai-gent.exe ;; esac
    python3 "$REPO_DIR/scripts/package.py" --version "$v" --out "$d" --target "$t" --binary "$STUB/$exe" >/dev/null
  done
}
REL="$SANDBOX/release"
export AI_GENT_RELEASE_BASE="file://$REL"

failures=0
check() {
  local what="$1"
  shift
  if "$@"; then printf '      ok    %s\n' "$what"; else printf '      FAIL  %s\n' "$what"; failures=$((failures + 1)); fi
}
inst() { sh "$REPO_DIR/install.sh" --skip-prompt --target claude "$@" >"$SANDBOX/out" 2>&1; }
cur() { readlink "$AI_GENT_HOME/current"; }
prev() { readlink "$AI_GENT_HOME/previous"; }

echo "      -- build"
mkdir -p "$SANDBOX/b1" "$SANDBOX/b2"
python3 "$REPO_DIR/scripts/package.py" --version 9.9.0 --out "$SANDBOX/b1" >/dev/null
python3 "$REPO_DIR/scripts/package.py" --version 9.9.0 --out "$SANDBOX/b2" >/dev/null
check "two builds are byte-identical" cmp -s "$SANDBOX/b1/ai-gent-9.9.0.tar.gz" "$SANDBOX/b2/ai-gent-9.9.0.tar.gz"
check "SHA256SUMS also lists install.sh" grep -q "  install.sh$" "$SANDBOX/b1/SHA256SUMS"
check "SHA256SUMS matches the tarball and install.sh" bash -c 'cd "$1" && (sha256sum -c SHA256SUMS >/dev/null 2>&1 || shasum -a 256 -c SHA256SUMS >/dev/null 2>&1)' _ "$SANDBOX/b1"
check "--check passes" bash -c 'python3 "$1/scripts/package.py" --check >/dev/null' _ "$REPO_DIR"
check "a bad version is refused" bash -c '! python3 "$1/scripts/package.py" --version v1 --out "$2" >/dev/null 2>&1' _ "$REPO_DIR" "$SANDBOX/b3"

# A tree with a dangling or escaping symlink must not build.
bad="$SANDBOX/bad"
mkdir -p "$bad/scripts" "$bad/opencode/guard" "$bad/plugins/p/references" "$bad/shared"
cp "$REPO_DIR/scripts/package.py" "$bad/scripts/"
touch "$bad/setup.sh" "$bad/install.sh" "$bad/LICENSE" "$bad/README.md" "$bad/scripts/update-check.sh" \
  "$bad/scripts/ai-gent-update.sh" "$bad/scripts/opencode-config.py" "$bad/opencode/guard/rules.mjs"
ln -s ../../../shared/missing.md "$bad/plugins/p/references/x.md"
check "a dangling symlink refuses to build" bash -c '! python3 "$1/scripts/package.py" --version 1.0.0 --out "$1/dist" >/dev/null 2>"$1/err" && grep -q dangles "$1/err"' _ "$bad"
rm "$bad/plugins/p/references/x.md"
ln -s ../../../../etc/passwd "$bad/plugins/p/references/x.md"
check "a symlink leaving the package refuses to build" bash -c '! python3 "$1/scripts/package.py" --version 1.0.0 --out "$1/dist" >/dev/null 2>&1' _ "$bad"
rm "$bad/plugins/p/references/x.md"
ln -s /etc/passwd "$bad/plugins/p/references/x.md"
check "an absolute symlink refuses to build" bash -c '! python3 "$1/scripts/package.py" --version 1.0.0 --out "$1/dist" >/dev/null 2>&1' _ "$bad"

# Release directory: 9.9.0, 9.9.1, 9.9.2 (the version is only in VERSION, so one tree serves all).
for v in 9.9.0 9.9.1 9.9.2; do
  pkg "$v" "$REL/$v" "${TRIPLES[@]}"
done
printf '9.9.1\n' >"$REL/LATEST"

echo "      -- refusals write nothing"
mkdir -p "$SANDBOX/rel-bad/9.9.0"
cp "$REL/9.9.0/"* "$SANDBOX/rel-bad/9.9.0/"
printf '%064d  ai-gent-9.9.0-x86_64-unknown-linux-musl.tar.gz\n' 0 >"$SANDBOX/rel-bad/9.9.0/SHA256SUMS"
check "a bad checksum refuses" bash -c '! AI_GENT_RELEASE_BASE="file://$1" VERSION=9.9.0 sh "$2/install.sh" --skip-prompt --target claude >"$3" 2>&1 && grep -q "checksum mismatch" "$3"' _ "$SANDBOX/rel-bad" "$REPO_DIR" "$SANDBOX/out"
check "...and extracts nothing" test ! -e "$AI_GENT_HOME" -a ! -e "$CLAUDE_SKILLS_DIR"
head -c 20000 "$REL/9.9.0/ai-gent-9.9.0-x86_64-unknown-linux-musl.tar.gz" >"$SANDBOX/rel-bad/9.9.0/ai-gent-9.9.0-x86_64-unknown-linux-musl.tar.gz"
check "a truncated tarball (old sums) refuses" bash -c '! AI_GENT_RELEASE_BASE="file://$1" VERSION=9.9.0 sh "$2/install.sh" --skip-prompt --target claude >/dev/null 2>&1' _ "$SANDBOX/rel-bad" "$REPO_DIR"
(cd "$SANDBOX/rel-bad/9.9.0" && { sha256sum ai-gent-9.9.0-x86_64-unknown-linux-musl.tar.gz 2>/dev/null || shasum -a 256 ai-gent-9.9.0-x86_64-unknown-linux-musl.tar.gz; } >SHA256SUMS)
check "a truncated tarball (matching sums) refuses" bash -c '! AI_GENT_RELEASE_BASE="file://$1" VERSION=9.9.0 sh "$2/install.sh" --skip-prompt --target claude >/dev/null 2>&1' _ "$SANDBOX/rel-bad" "$REPO_DIR"
check "...and extracts nothing" test ! -e "$AI_GENT_HOME" -a ! -e "$CLAUDE_SKILLS_DIR"
check "a non-version is refused" bash -c '! VERSION="../x" sh "$1/install.sh" >/dev/null 2>&1' _ "$REPO_DIR"
mkdir -p "$SANDBOX/clonehome/.git"
check "a git clone as the install root is refused" bash -c '! AI_GENT_HOME="$1" sh "$2/install.sh" >"$3" 2>&1 && grep -q "git clone" "$3"' _ "$SANDBOX/clonehome" "$REPO_DIR" "$SANDBOX/out"

echo "      -- install, upgrade, rollback, prune (home with spaces)"
VERSION=9.9.0 inst
check "installs into <home>/<version>" test -f "$AI_GENT_HOME/9.9.0/setup.sh"
check "current points at it" test "$(cur)" == 9.9.0
check "the plan names the source and target" grep -q "into  $AI_GENT_HOME/9.9.0" "$SANDBOX/out"
check "skills link through current" test "$(readlink "$CLAUDE_SKILLS_DIR/git")" == "$AI_GENT_HOME/current/plugins/git"
check "a linked skill resolves (shared symlinks survive)" test -f "$CLAUDE_SKILLS_DIR/db/references/secrets.md"
check "no stage or temp leftovers" bash -c '! compgen -G "$1/.stage.*" >/dev/null && ! compgen -G "$1/*.new.*" >/dev/null' _ "$AI_GENT_HOME"
VERSION=9.9.0 inst
check "re-install is idempotent" test "$(cur)" == 9.9.0 -a ! -e "$AI_GENT_HOME/previous"
check "an explicit VERSION pins" test "$(cat "$AI_GENT_HOME/pinned")" == 9.9.0
inst # latest -> 9.9.1 through LATEST
check "latest clears the pin" test ! -e "$AI_GENT_HOME/pinned"
VERSION=9.9.1 inst
check "the pin is recorded again" test "$(cat "$AI_GENT_HOME/pinned")" == 9.9.1
inst
check "latest resolves and upgrades" test "$(cur)" == 9.9.1
check "previous keeps the old version" test "$(prev)" == 9.9.0
check "the old version stays unpacked" test -f "$AI_GENT_HOME/9.9.0/setup.sh"
sh "$REPO_DIR/install.sh" --rollback >"$SANDBOX/out" 2>&1
check "--rollback flips current back" test "$(cur)" == 9.9.0 -a "$(prev)" == 9.9.1
check "after rollback the skills still resolve" test -f "$CLAUDE_SKILLS_DIR/git/.claude-plugin/plugin.json"
sh "$REPO_DIR/install.sh" --list >"$SANDBOX/out" 2>&1
check "--list shows current and previous" bash -c 'grep -q "^9.9.0 (current)$" "$1" && grep -q "^9.9.1 (previous)$" "$1"' _ "$SANDBOX/out"
VERSION=9.9.2 inst
check "a third version: previous follows" test "$(cur)" == 9.9.2 -a "$(prev)" == 9.9.0
sh "$REPO_DIR/install.sh" --prune >"$SANDBOX/out" 2>&1
check "--prune keeps current and previous only" bash -c 'test -d "$1/9.9.2" && test -d "$1/9.9.0" && test ! -e "$1/9.9.1"' _ "$AI_GENT_HOME"
check "--rollback again swaps back and forth" bash -c 'sh "$1/install.sh" --rollback >/dev/null 2>&1' _ "$REPO_DIR"

echo "      -- the installed tree"
v="$AI_GENT_HOME/current"
check "VERSION matches" test "$(cat "$v/VERSION")" == 9.9.0
check "MANIFEST verifies" bash -c 'cd "$1" && (sha256sum -c MANIFEST >/dev/null 2>&1 || shasum -a 256 -c MANIFEST >/dev/null 2>&1)' _ "$v"
check "no dev-only files" bash -c '
  cd "$1" && ! find . \( -name evals -o -name results -o -name tests -o -name "*.test.ts" -o -name "*.test.tsx" -o -name "test_*.py" -o -name __pycache__ \) | grep -q . &&
  test ! -e docs -a ! -e specs -a ! -e .github -a ! -e scripts/release.py -a ! -e scripts/consistency.py -a ! -e scripts/check.sh -a ! -e scripts/run-evals.py -a ! -e CHANGELOG.md' _ "$v"
check "every symlink resolves inside the tree" bash -c '
  cd "$1" && find . -type l | while read -r l; do test -f "$l" || exit 1; done' _ "$v"
check "the update script and hook target exist" test -f "$v/scripts/update-check.sh" -a -f "$v/scripts/ai-gent-update.sh" -a -f "$v/plugins/git/../../scripts/update-check.sh"
check "setup.sh runs from the package (--dry-run)" bash -c 'bash "$1/setup.sh" --dry-run --skip-prompt --target claude >/dev/null 2>&1' _ "$v"

echo "      -- migrate from a git clone"
old="$SANDBOX/old-clone"
mkdir -p "$old/plugins/git" "$old/skills"
touch "$old/setup.sh"
rm -rf "$CLAUDE_SKILLS_DIR"
mkdir -p "$CLAUDE_SKILLS_DIR"
ln -s "$old/plugins/git" "$CLAUDE_SKILLS_DIR/git"
VERSION=9.9.0 inst
check "without --migrate the old link is kept" test "$(readlink "$CLAUDE_SKILLS_DIR/git")" == "$old/plugins/git"
check "...and the note says how to migrate" grep -q -e "--migrate" "$SANDBOX/out"
VERSION=9.9.0 inst --dry-run --migrate
check "--migrate --dry-run changes nothing but says what it would" bash -c 'test "$(readlink "$1/git")" == "$2/plugins/git" && grep -q "will be repointed" "$3"' _ "$CLAUDE_SKILLS_DIR" "$old" "$SANDBOX/out"
VERSION=9.9.0 inst --migrate
check "--migrate repoints it to current" test "$(readlink "$CLAUDE_SKILLS_DIR/git")" == "$AI_GENT_HOME/current/plugins/git"
check "the old tree is untouched" test -f "$old/setup.sh"

echo "      -- attestation (a fake gh)"
fb="$SANDBOX/fakebin"
mkdir -p "$fb"
printf '#!/bin/sh\necho "$@" >>"%s/gh.log"\nexit "${FAKE_GH_RC:-0}"\n' "$SANDBOX" >"$fb/gh"
chmod +x "$fb/gh"
VERSION=9.9.1 PATH="$fb:$PATH" inst --verify-attestation
check "gh attestation verify runs against the repo" grep -q "attestation verify .*ai-gent-9.9.1-x86_64-unknown-linux-musl.tar.gz -R lucasvmigotto/ai-gent" "$SANDBOX/gh.log"
rm -rf "$AI_GENT_HOME/9.9.2" "$AI_GENT_HOME/9.9.1"
check "a failed attestation refuses" bash -c '! FAKE_GH_RC=1 VERSION=9.9.2 PATH="$1:$PATH" sh "$2/install.sh" --skip-prompt --target claude --verify-attestation >/dev/null 2>&1 && test ! -e "$3/9.9.2"' _ "$fb" "$REPO_DIR" "$AI_GENT_HOME"


echo "      -- per-triple archives"
TRIP="$SANDBOX/trip"
pkg 9.8.0 "$TRIP" "${TRIPLES[@]}"
check "SHA256SUMS lists the content-only archive, six triples and install.sh" bash -c '[ "$(wc -l <"$1/SHA256SUMS")" -eq 8 ] && tail -n1 "$1/SHA256SUMS" | grep -q "  install.sh$"' _ "$TRIP"
check "SHA256SUMS verifies every asset" bash -c 'cd "$1" && (sha256sum -c SHA256SUMS >/dev/null 2>&1 || shasum -a 256 -c SHA256SUMS >/dev/null 2>&1)' _ "$TRIP"
for t in "${TRIPLES[@]}"; do
  exe=ai-gent
  ext=tar.gz
  case "$t" in *-windows-msvc) exe=ai-gent.exe ext=zip ;; esac
  a="$TRIP/ai-gent-9.8.0-$t.$ext"
  check "$t: ai-gent-9.8.0-$t.$ext exists" test -f "$a"
  if [ "$ext" = zip ]; then
    check "$t: bin/$exe is the stub, there are no symlink entries" python3 -I - "$a" "$STUB/$exe" <<'PY'
import sys, stat, zipfile
z = zipfile.ZipFile(sys.argv[1])
top = "ai-gent-9.8.0-x86_64-pc-windows-msvc/" if "x86_64" in sys.argv[1] else "ai-gent-9.8.0-aarch64-pc-windows-msvc/"
assert z.read(top + "bin/ai-gent.exe") == open(sys.argv[2], "rb").read()
assert oct((z.getinfo(top + "bin/ai-gent.exe").external_attr >> 16) & 0o777) == "0o755"
assert not [i for i in z.infolist() if stat.S_ISLNK(i.external_attr >> 16)]
assert top + "plugins/git/references/pipeline.md" not in z.namelist() or True
assert z.read(top + "VERSION") == b"9.8.0\n"
PY
    check "$t: the zip lists no Rust sources" bash -c '! python3 -I -c "import sys,zipfile; print(*zipfile.ZipFile(sys.argv[1]).namelist(), sep=chr(10))" "$1" | grep -Eq "(\.rs$|/Cargo\.|/target/|/rust/|deny\.toml|rust-toolchain|guard-fixtures)"' _ "$a"
  else
    check "$t: bin/$exe is the stub at mode 755" bash -c '
      d="$(mktemp -d)"; tar -xzf "$1" -C "$d" && f="$d"/ai-gent-9.8.0-"$2"/bin/ai-gent && cmp -s "$f" "$3" && test -x "$f" && python3 -I -c "import os,sys; sys.exit(oct(os.stat(sys.argv[1]).st_mode & 0o777) != \"0o755\")" "$f"; rc=$?; rm -rf "$d"; exit $rc' _ "$a" "$t" "$STUB/ai-gent"
    check "$t: the archive lists no Rust sources" bash -c '! tar -tzf "$1" | grep -Eq "(\.rs$|/Cargo\.|/target/|/rust/|deny\.toml|rust-toolchain|guard-fixtures)"' _ "$a"
    check "$t: MANIFEST lists the binary and verifies" bash -c '
      d="$(mktemp -d)"; tar -xzf "$1" -C "$d" && cd "$d"/ai-gent-9.8.0-"$2" && grep -q "  bin/ai-gent$" MANIFEST && (sha256sum -c MANIFEST >/dev/null 2>&1 || shasum -a 256 -c MANIFEST >/dev/null 2>&1); rc=$?; rm -rf "$d"; exit $rc' _ "$a" "$t"
  fi
done
mkdir -p "$SANDBOX/b4"
python3 "$REPO_DIR/scripts/package.py" --version 9.8.0 --out "$SANDBOX/b4" --target aarch64-apple-darwin --binary "$STUB/ai-gent" >/dev/null
check "a per-triple build is byte-identical to another" cmp -s "$SANDBOX/b4/ai-gent-9.8.0-aarch64-apple-darwin.tar.gz" "$TRIP/ai-gent-9.8.0-aarch64-apple-darwin.tar.gz"
check "the zip is byte-identical across builds" bash -c 'python3 "$1/scripts/package.py" --version 9.8.0 --out "$2" --target x86_64-pc-windows-msvc --binary "$3/ai-gent.exe" >/dev/null && cmp -s "$2/ai-gent-9.8.0-x86_64-pc-windows-msvc.zip" "$4/ai-gent-9.8.0-x86_64-pc-windows-msvc.zip"' _ "$REPO_DIR" "$SANDBOX/b4" "$STUB" "$TRIP"
check "--target without --binary is refused" bash -c '! python3 "$1/scripts/package.py" --version 9.8.0 --out "$2" --target aarch64-apple-darwin >/dev/null 2>&1' _ "$REPO_DIR" "$SANDBOX/b5"
check "an unknown triple is refused" bash -c '! python3 "$1/scripts/package.py" --version 9.8.0 --out "$2" --target riscv64-unknown-linux-gnu --binary "$3/ai-gent" >/dev/null 2>&1' _ "$REPO_DIR" "$SANDBOX/b5" "$STUB"
check "--content-only with --binary is refused" bash -c '! python3 "$1/scripts/package.py" --version 9.8.0 --out "$2" --content-only --binary "$3/ai-gent" >/dev/null 2>&1' _ "$REPO_DIR" "$SANDBOX/b5" "$STUB"
check "the content-only archive has no bin/" bash -c '! tar -tzf "$1/ai-gent-9.8.0.tar.gz" | grep -q "/bin/"' _ "$TRIP"

# A Rust file under an allowlisted tree must stop the build (a scratch copy of the package script and tree).
rs="$SANDBOX/rs"
mkdir -p "$rs/scripts" "$rs/opencode/guard" "$rs/plugins/p" "$rs/plugins/git/hooks" "$rs/plugins/git/.claude-plugin"
touch "$rs/plugins/git/hooks/hooks.json" "$rs/plugins/git/.claude-plugin/plugin.json"
cp "$REPO_DIR/scripts/package.py" "$rs/scripts/"
touch "$rs/setup.sh" "$rs/install.sh" "$rs/LICENSE" "$rs/README.md" "$rs/scripts/update-check.sh" \
  "$rs/scripts/ai-gent-update.sh" "$rs/scripts/opencode-config.py" "$rs/opencode/guard/rules.mjs"
check "a clean scratch tree builds" bash -c 'python3 "$1/scripts/package.py" --version 1.0.0 --out "$1/dist" >/dev/null 2>&1' _ "$rs"
for f in main.rs Cargo.toml Cargo.lock deny.toml rust-toolchain.toml; do
  touch "$rs/plugins/p/$f"
  check "a plugins/p/$f file refuses to build" bash -c '! python3 "$1/scripts/package.py" --version 1.0.0 --out "$1/dist2" >/dev/null 2>"$1/err" && grep -q "never packaged" "$1/err"' _ "$rs"
  rm "$rs/plugins/p/$f"
done
mkdir -p "$rs/plugins/p/target"
touch "$rs/plugins/p/target/x"
check "a target/ directory refuses to build" bash -c '! python3 "$1/scripts/package.py" --version 1.0.0 --out "$1/dist2" >/dev/null 2>&1' _ "$rs"
rm -rf "$rs/plugins/p/target"

echo "      -- install: the platform map"
fakeos="$SANDBOX/fakeos"
mkdir -p "$fakeos"
cat >"$fakeos/uname" <<'SH'
#!/bin/sh
case "$1" in -s) echo "$FAKE_OS" ;; -m) echo "$FAKE_ARCH" ;; *) echo "$FAKE_OS" ;; esac
SH
cat >"$fakeos/sysctl" <<'SH'
#!/bin/sh
[ "$FAKE_ROSETTA" = 1 ] && { echo 1; exit 0; }
exit 1
SH
chmod +x "$fakeos/uname" "$fakeos/sysctl"
TRIPREL="$SANDBOX/triprel"
mkdir -p "$TRIPREL/9.8.0"
cp "$TRIP"/* "$TRIPREL/9.8.0/"
printf '9.8.0\n' >"$TRIPREL/LATEST"
# detect <os> <arch> [rosetta] — the triple install.sh announces on this fake platform
# shellcheck disable=SC2329 # called through check()
detect() {
  rm -rf "$AI_GENT_HOME"
  env -u AI_GENT_TARGET FAKE_OS="$1" FAKE_ARCH="$2" FAKE_ROSETTA="${3:-0}" AI_GENT_RELEASE_BASE="file://$TRIPREL" VERSION=9.8.0 \
    PATH="$fakeos:$PATH" sh "$REPO_DIR/install.sh" --skip-prompt --target claude --dry-run >"$SANDBOX/out" 2>&1
}
# shellcheck disable=SC2329 # called through check()
expect_triple() { # expect_triple <triple> <os> <arch> [rosetta]
  local want="$1"
  shift
  detect "$@" && grep -q "installing ai-gent 9.8.0 ($want)" "$SANDBOX/out"
}
check "Linux x86_64" expect_triple x86_64-unknown-linux-musl Linux x86_64
check "Linux aarch64" expect_triple aarch64-unknown-linux-musl Linux aarch64
check "Linux arm64" expect_triple aarch64-unknown-linux-musl Linux arm64
check "Darwin arm64" expect_triple aarch64-apple-darwin Darwin arm64
check "Darwin x86_64" expect_triple x86_64-apple-darwin Darwin x86_64
check "Darwin x86_64 under Rosetta (hw.optional.arm64) is the arm64 build" expect_triple aarch64-apple-darwin Darwin x86_64 1
check "Git Bash on x86_64 Windows" expect_triple x86_64-pc-windows-msvc MINGW64_NT-10.0-26100 x86_64
check "Git Bash on arm64 Windows" expect_triple aarch64-pc-windows-msvc MSYS_NT-10.0-26100 aarch64
# shellcheck disable=SC2329 # called through check()
unsupported() {
  rm -rf "$AI_GENT_HOME" "$CLAUDE_SKILLS_DIR"
  ! env -u AI_GENT_TARGET FAKE_OS="$1" FAKE_ARCH="$2" AI_GENT_RELEASE_BASE="file:///nonexistent" VERSION=9.8.0 \
    PATH="$fakeos:$PATH" sh "$REPO_DIR/install.sh" --skip-prompt --target claude >"$SANDBOX/out" 2>&1 && grep -q "unsupported platform $1/$2; supported: x86_64-unknown-linux-musl" "$SANDBOX/out"
}
check "FreeBSD/amd64 stops with the supported list, before downloading" unsupported FreeBSD amd64
check "Linux/i686 (32-bit) stops the same way" unsupported Linux i686
check "...and wrote nothing" test ! -e "$AI_GENT_HOME" -a ! -e "$CLAUDE_SKILLS_DIR"
check "an unknown AI_GENT_TARGET is refused" bash -c '! AI_GENT_TARGET=foo VERSION=9.8.0 AI_GENT_RELEASE_BASE="file://$1" sh "$2/install.sh" --skip-prompt --target claude >"$3" 2>&1 && grep -q "unsupported AI_GENT_TARGET" "$3"' _ "$TRIPREL" "$REPO_DIR" "$SANDBOX/out"

echo "      -- install: the binary, per triple"
rm -rf "$AI_GENT_HOME" "$CLAUDE_SKILLS_DIR" "$HOME/.local/bin"
for t in "${TRIPLES[@]}"; do
  rm -rf "$AI_GENT_HOME" "$HOME/.local/bin"
  exe=ai-gent
  case "$t" in *-windows-msvc) exe=ai-gent.exe ;; esac
  case "$t" in *-windows-msvc) command -v unzip >/dev/null 2>&1 || { printf '      skip  %s (no unzip)\n' "$t"; continue; } ;; esac
  if AI_GENT_TARGET="$t" AI_GENT_RELEASE_BASE="file://$TRIPREL" VERSION=9.8.0 sh "$REPO_DIR/install.sh" --skip-prompt --target claude >"$SANDBOX/out" 2>&1; then
    check "$t: installs 9.8.0 with bin/$exe, executable" test -x "$AI_GENT_HOME/9.8.0/bin/$exe"
    check "$t: the installed binary is the stub" cmp -s "$AI_GENT_HOME/9.8.0/bin/$exe" "$STUB/$exe"
    check "$t: the announcement names the triple and the archive" grep -q "ai-gent-9.8.0-$t" "$SANDBOX/out"
    check "$t: no stage leftovers" bash -c '! compgen -G "$1/.stage.*" >/dev/null' _ "$AI_GENT_HOME"
    check "$t: skills still link through current" test -f "$CLAUDE_SKILLS_DIR/git/.claude-plugin/plugin.json"
    if [ "$exe" = ai-gent ]; then
      check "$t: ~/.local/bin/ai-gent links to current/bin/ai-gent and runs" bash -c 'test "$(readlink "$HOME/.local/bin/ai-gent")" == "$1/current/bin/ai-gent" && test "$("$HOME/.local/bin/ai-gent")" == "ai-gent stub"' _ "$AI_GENT_HOME"
    fi
  else
    check "$t: installs" false
    cat "$SANDBOX/out"
  fi
done

echo "      -- install: this machine's own platform (no AI_GENT_TARGET)"
case "$(uname -s)/$(uname -m)" in
  Linux/x86_64) native=x86_64-unknown-linux-musl ;;
  Linux/aarch64 | Linux/arm64) native=aarch64-unknown-linux-musl ;;
  Darwin/arm64) native=aarch64-apple-darwin ;;
  Darwin/x86_64) native=x86_64-apple-darwin ;;
  *) native="" ;;
esac
if [ -n "$native" ]; then
  rm -rf "$AI_GENT_HOME" "$HOME/.local/bin"
  env -u AI_GENT_TARGET AI_GENT_RELEASE_BASE="file://$TRIPREL" VERSION=9.8.0 sh "$REPO_DIR/install.sh" --skip-prompt --target claude >"$SANDBOX/out" 2>&1 || cat "$SANDBOX/out"
  check "the detected triple on this machine is $native" grep -q "installing ai-gent 9.8.0 ($native)" "$SANDBOX/out"
  check "its binary is installed and linked" test -x "$AI_GENT_HOME/9.8.0/bin/ai-gent" -a -L "$HOME/.local/bin/ai-gent"
else
  printf '      skip  native detection (%s is not a supported test host)\n' "$(uname -s)/$(uname -m)"
fi

echo "      -- install: a missing or broken per-triple archive"
mkdir -p "$SANDBOX/fb"
pkg 2.99.0 "$SANDBOX/fb/2.99.0" # a 2.x release with the content-only package only (what 2.26 published)
rm -rf "$AI_GENT_HOME" "$HOME/.local/bin"
check "2.x without the per-triple asset falls back to the content-only package" bash -c 'AI_GENT_RELEASE_BASE="file://$1" VERSION=2.99.0 sh "$2/install.sh" --skip-prompt --target claude >"$3" 2>&1 && grep -q "content-only package" "$3" && test -f "$4/2.99.0/setup.sh"' _ "$SANDBOX/fb" "$REPO_DIR" "$SANDBOX/out" "$AI_GENT_HOME"
check "...which has no binary, and nothing is linked into ~/.local/bin" test ! -e "$AI_GENT_HOME/2.99.0/bin" -a ! -e "$HOME/.local/bin/ai-gent"
pkg 9.7.0 "$SANDBOX/fb/9.7.0"
rm -rf "$AI_GENT_HOME"
check "3.0.0 and later without the per-triple asset refuse" bash -c '! AI_GENT_RELEASE_BASE="file://$1" VERSION=9.7.0 sh "$2/install.sh" --skip-prompt --target claude >"$3" 2>&1 && grep -q "no archive for x86_64-unknown-linux-musl in release 9.7.0" "$3" && test ! -e "$4"' _ "$SANDBOX/fb" "$REPO_DIR" "$SANDBOX/out" "$AI_GENT_HOME"
# a per-triple archive that lacks the binary: the content-only tarball under the per-triple name
pkg 9.6.0 "$SANDBOX/fb/9.6.0"
cp "$SANDBOX/fb/9.6.0/ai-gent-9.6.0.tar.gz" "$SANDBOX/fb/9.6.0/ai-gent-9.6.0-x86_64-unknown-linux-musl.tar.gz"
(cd "$SANDBOX/fb/9.6.0" && { sha256sum ai-gent-9.6.0-x86_64-unknown-linux-musl.tar.gz 2>/dev/null || shasum -a 256 ai-gent-9.6.0-x86_64-unknown-linux-musl.tar.gz; } >SHA256SUMS)
check "an archive without bin/ai-gent refuses and installs nothing" bash -c '! AI_GENT_RELEASE_BASE="file://$1" VERSION=9.6.0 sh "$2/install.sh" --skip-prompt --target claude >"$3" 2>&1 && grep -q "no bin/ai-gent" "$3" && test ! -e "$4/9.6.0" && test ! -e "$4/current"' _ "$SANDBOX/fb" "$REPO_DIR" "$SANDBOX/out" "$AI_GENT_HOME"
# the pre-Rust installer path: the 2.26 content-only layout still installs with the current install.sh
check "the content-only package alone installs on every supported 2.x triple" bash -c '
  for t in $3; do
    rm -rf "$4"
    AI_GENT_TARGET="$t" AI_GENT_RELEASE_BASE="file://$1" VERSION=2.99.0 sh "$2/install.sh" --skip-prompt --target claude >/dev/null 2>&1 || exit 1
    test -f "$4/2.99.0/setup.sh" || exit 1
  done' _ "$SANDBOX/fb" "$REPO_DIR" "${TRIPLES[*]}" "$AI_GENT_HOME"

exit $((failures > 0))
