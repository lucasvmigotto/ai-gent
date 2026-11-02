#!/usr/bin/env bash
# Shape test for the Rust build and release workflows (spec 032 contracts/workflows.md). Pure
# bash: it strips YAML comments and greps the real files, so a pin turned back into a tag, a
# permission added to the rehearsal, or a publish job that can run unguarded fails here before
# it can fail a release. Every check is one `ok` line; the last line is `N/N`.
#
# RUST_WORKFLOWS_ROOT points at another tree (the self-test at the end mutates a copy and
# requires this script to fail on it). Called by scripts/check.sh.

set -uo pipefail

SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
ROOT="${RUST_WORKFLOWS_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
WF="$ROOT/.github/workflows"
total=0 failed=0

ok() { total=$((total + 1)); printf '      ok    %s\n' "$1"; }
bad() { total=$((total + 1)); failed=$((failed + 1)); printf '      FAIL  %s\n' "$1"; }
check() { # check <description> <command...>
  local what="$1"
  shift
  if "$@" >/dev/null 2>&1; then ok "$what"; else bad "$what"; fi
}
# strip <file> — the file without comments and blank lines
strip() { sed -e 's/[[:space:]]*#.*$//' -e '/^[[:space:]]*$/d' "$1"; }
# job <file> <name> — the lines of one job (from `  <name>:` to the next job), comments stripped
job() {
  strip "$1" | awk -v j="  $2:" '$0 == j { on = 1; print; next } on && /^  [A-Za-z0-9_-]+:/ { exit } on { print }'
}

for f in release rust rust-build check; do
  [[ -f $WF/$f.yml ]] || { echo "missing $WF/$f.yml" >&2; exit 1; }
done

# ----------------------------------------------------------------- every workflow
echo "      -- pins"
unpinned=""
for f in "$WF"/*.yml; do
  while read -r ref; do
    case "$ref" in ./*) continue ;; esac
    [[ $ref =~ @[0-9a-f]{40}$ ]] || unpinned+="$(basename "$f"): $ref"$'\n'
  done < <(strip "$f" | sed -n 's/^[[:space:]-]*uses:[[:space:]]*//p')
done
if [[ -z $unpinned ]]; then
  ok "every action in every workflow is pinned by a 40-hex commit SHA"
else
  bad "unpinned actions:"
  printf '%s' "$unpinned"
fi
check "no workflow installs a tool from latest" bash -c '! cat "$1"/*.yml | sed "s/[[:space:]]*#.*$//" | grep -Eq "@latest|/releases/latest/download|version: *latest"' _ "$WF"

# ----------------------------------------------------------------- rust-build.yml
echo "      -- rust-build.yml"
RB="$(strip "$WF/rust-build.yml")"
check "is a reusable workflow with ref, release and dry_run inputs" bash -c 'grep -q "workflow_call:" <<<"$1" && grep -Eq "^      ref:" <<<"$1" && grep -Eq "^      release:" <<<"$1" && grep -Eq "^      dry_run:" <<<"$1"' _ "$RB"
check "top-level permissions are contents: read and nothing else" bash -c 'p="$(awk "/^permissions:/{on=1;next} on&&/^[^ ]/{exit} on{print}" <<<"$1")"; [ "$(grep -c . <<<"$p")" -eq 1 ] && grep -q "contents: read" <<<"$p"' _ "$RB"
check "no secrets context" bash -c '! grep -Eq "secrets[.:]" <<<"$1"' _ "$RB"
check "no write permission, id-token or attestations" bash -c '! grep -Eq "write|id-token|attestations" <<<"$1"' _ "$RB"
for pair in x86_64-unknown-linux-musl:ubuntu-24.04 aarch64-unknown-linux-musl:ubuntu-24.04-arm aarch64-apple-darwin:macos-15 x86_64-apple-darwin:macos-15-intel x86_64-pc-windows-msvc:windows-2025 aarch64-pc-windows-msvc:windows-11-arm; do
  t="${pair%%:*}" os="${pair#*:}"
  check "target $t runs on $os" bash -c 'grep -Eq "target: $1, os: $2," <<<"$3"' _ "$t" "$os" "$RB"
done
check "checkout does not persist credentials" bash -c 'grep -q "persist-credentials: false" <<<"$1"' _ "$RB"
check "the Windows legs get +crt-static from rust/.cargo/config.toml, and cargo runs inside rust/" bash -c 'grep -q "+crt-static" "$1/rust/.cargo/config.toml" && grep -B1 -A2 "name: Build" <<<"$2" | grep -q "working-directory: rust"' _ "$ROOT" "$RB"
check "builds with --release --locked --target" bash -c 'grep -q "cargo build --release --locked --target" <<<"$1"' _ "$RB"
check "Cargo.lock is checked with cargo metadata --locked" bash -c 'grep -q "cargo metadata --locked" <<<"$1"' _ "$RB"
check "a smoke step runs version --json and doctor --json on every leg" bash -c 'grep -q "name: Smoke" <<<"$1" && grep -q "version --json" <<<"$1" && grep -q "doctor --json" <<<"$1"' _ "$RB"
check "a release build asserts the binary reports the tag" bash -c 'grep -q "AI_GENT_RELEASE\" != dev" <<<"$1" && grep -Fq "ai-gent \$AI_GENT_RELEASE (\$TARGET)" <<<"$1"' _ "$RB"
check "uploads bin-<triple>" bash -c 'grep -q "name: bin-\${{ matrix.target }}" <<<"$1"' _ "$RB"

# ----------------------------------------------------------------- rust.yml
echo "      -- rust.yml (the rehearsal)"
R="$(strip "$WF/rust.yml")"
check "runs on pull_request (with a paths filter) and workflow_dispatch" bash -c 'grep -q "^  pull_request:" <<<"$1" && grep -q "^    paths:" <<<"$1" && grep -q "^  workflow_dispatch:" <<<"$1"' _ "$R"
check "calls rust-build.yml with dry_run: true (literal)" bash -c 'grep -q "uses: ./.github/workflows/rust-build.yml" <<<"$1" && grep -Eq "^      dry_run: true$" <<<"$1"' _ "$R"
check "no contents: write, id-token, attestations or secrets" bash -c '! grep -Eq "contents: write|id-token|attestations|secrets[.:]" <<<"$1"' _ "$R"
check "never tags, releases, attests or pushes" bash -c '! grep -Eq "gh release|git tag|git push|attest-build-provenance|crates-io-auth-action" <<<"$1"' _ "$R"
check "packages with the release job's script" bash -c 'grep -q "scripts/package-release.sh" <<<"$1"' _ "$R"
check "dry-runs the crates publish only" bash -c 'grep -q "scripts/rust-publish.sh dry-run" <<<"$1" && ! grep -Eq "rust-publish.sh publish|cargo publish" <<<"$1"' _ "$R"
check "has no publish-crates job" bash -c '! grep -q "publish-crates" <<<"$1"' _ "$R"
check "asserts a Checksums block in the notes it generates" bash -c 'grep -q "Checksums" <<<"$1"' _ "$R"

# ----------------------------------------------------------------- release.yml
echo "      -- release.yml"
REL="$(strip "$WF/release.yml")"
RJ="$(job "$WF/release.yml" release)"
RBJ="$(job "$WF/release.yml" rust-build)"
PJ="$(job "$WF/release.yml" package)"
CJ="$(job "$WF/release.yml" publish-crates)"
check "the release job keeps its main-only condition" bash -c "grep -q \"github.ref == 'refs/heads/main'\" <<<\"\$1\"" _ "$RJ"
check "the release commit adds both crates' Cargo.toml and rust/Cargo.lock" bash -c 'grep -Eq "git add .*rust/crates/\*/Cargo.toml rust/Cargo.lock" <<<"$1"' _ "$RJ"
check "...after the usual files, in the same commit" bash -c 'grep -n "git add\|git commit" <<<"$1" | sed -n "1,3p" | tail -n1 | grep -q "git commit"' _ "$RJ"
VNE="needs.release.outputs.version != ''"
check "rust-build needs release, runs only when a version was released, builds the tag" bash -c 'grep -q "needs: release" <<<"$1" && grep -Fq "$2" <<<"$1" && grep -q "ref: refs/tags/" <<<"$1" && grep -q "uses: ./.github/workflows/rust-build.yml" <<<"$1" && grep -Eq "dry_run: false" <<<"$1"' _ "$RBJ" "$VNE"
check "rust-build passes the version as the release embedded in the binary" bash -c 'grep -Fq "release: \${{ needs.release.outputs.version }}" <<<"$1"' _ "$RBJ"
check "package needs release and rust-build" bash -c 'grep -Eq "needs: \[release, rust-build\]" <<<"$1"' _ "$PJ"
check "package asserts the tag is the checked-out commit and the crate carries the version" bash -c 'grep -q "rev-parse HEAD" <<<"$1" && grep -q "crates/ai-gent/Cargo.toml" <<<"$1"' _ "$PJ"
check "package downloads the bin-* artifacts" bash -c 'grep -q "actions/download-artifact@" <<<"$1" && grep -q "pattern: bin-\*" <<<"$1"' _ "$PJ"
check "package builds everything with scripts/package-release.sh" bash -c 'grep -q "scripts/package-release.sh --version" <<<"$1"' _ "$PJ"
check "the script runs package.py once per target plus content-only, and verifies SHA256SUMS" bash -c 'f="$1/scripts/package-release.sh"; grep -q "package.py\" --version \"\$version\" --out \"\$out\"$" "$f" && grep -q -- "--target \"\$t\" --binary" "$f" && grep -q "sha256sum -c SHA256SUMS" "$f"' _ "$ROOT"
check "the script lists the six targets" bash -c 'for t in x86_64-unknown-linux-musl aarch64-unknown-linux-musl aarch64-apple-darwin x86_64-apple-darwin x86_64-pc-windows-msvc aarch64-pc-windows-msvc; do grep -q "$t" "$1/scripts/package-release.sh" || exit 1; done' _ "$ROOT"
check "the notes step appends a ## Checksums block (D25) in the package step" bash -c 'grep -q "## Checksums" "$1/scripts/package-release.sh" && grep -q "release-notes.md" "$1/scripts/package-release.sh"' _ "$ROOT"
check "attestation lists the content-only archive, the per-target tarballs, the zips, install.sh and install.ps1" bash -c 'a="$(awk "/attest-build-provenance/{on=1} on" <<<"$1")"; grep -Fq "dist/ai-gent-\${{ needs.release.outputs.version }}.tar.gz" <<<"$a" && grep -Fq "}-*.tar.gz" <<<"$a" && grep -Fq "}-*.zip" <<<"$a" && grep -q "dist/install.sh" <<<"$a" && grep -q "dist/install.ps1" <<<"$a"' _ "$PJ"
check "assets uploaded: archives, zips, SHA256SUMS, install.sh and install.ps1" bash -c 'u="$(grep "gh release upload" <<<"$1")"; for p in "-\*.tar.gz" "-\*.zip" SHA256SUMS install.sh install.ps1; do grep -q -- "$p" <<<"$u" || exit 1; done' _ "$PJ"
check "the notes (with Checksums) are edited in before the release is published" bash -c 'n=$(grep -n -- "--notes-file dist/release-notes.md" <<<"$1" | head -n1 | cut -d: -f1); d=$(grep -n -- "--draft=false" <<<"$1" | head -n1 | cut -d: -f1); u=$(grep -n "gh release upload" <<<"$1" | head -n1 | cut -d: -f1); [ -n "$n" ] && [ -n "$d" ] && [ "$u" -lt "$n" ] && [ "$n" -lt "$d" ]' _ "$PJ"
check "the release is published (--draft=false --latest) only in the last command of package" bash -c '[ "$(grep -c -- "--draft=false" <<<"$1")" -eq 1 ] && tail -n1 <<<"$1" | grep -q -- "--draft=false --latest"' _ "$PJ"
check "publish-crates needs release, rust-build and package" bash -c 'grep -Eq "needs: \[release, rust-build, package\]" <<<"$1"' _ "$CJ"
check "publish-crates only runs when vars.AI_GENT_CRATES_PUBLISH is the string true, and a release exists" bash -c "grep -Fq \"vars.AI_GENT_CRATES_PUBLISH == 'true' && needs.release.outputs.version != ''\" <<<\"\$1\"" _ "$CJ"
check "publish-crates uses the crate-release environment" bash -c 'grep -Eq "^    environment: crate-release$" <<<"$1"' _ "$CJ"
check "publish-crates permissions are contents: read and id-token: write" bash -c 'p="$(awk "/^    permissions:/{on=1;next} on&&/^    [a-z]/{exit} on{print}" <<<"$1")"; [ "$(grep -c . <<<"$p")" -eq 2 ] && grep -q "contents: read" <<<"$p" && grep -q "id-token: write" <<<"$p"' _ "$CJ"
check "publish-crates gets its token from the auth action's output" bash -c 'grep -q "rust-lang/crates-io-auth-action@" <<<"$1" && grep -Fq "CARGO_REGISTRY_TOKEN: \${{ steps.auth.outputs.token }}" <<<"$1"' _ "$CJ"
check "CARGO_REGISTRY_TOKEN appears only as env wiring from steps.auth, never from secrets" bash -c '! cat "$1"/*.yml | sed "s/[[:space:]]*#.*$//" | grep CARGO_REGISTRY_TOKEN | grep -v "steps.auth.outputs.token"; ! grep -rEq "secrets\.[A-Za-z_]*(CARGO|CRATES)" "$1"' _ "$WF"
check "release, rust, rust-build and check read no secrets context" bash -c 'cd "$1" && ! cat release.yml rust.yml rust-build.yml check.yml | sed "s/[[:space:]]*#.*$//" | grep -Eq "secrets\.|secrets:|secrets\["' _ "$WF"
check "the metadata gate (T033a) runs before the auth step, and publish runs after it" bash -c 'm=$(grep -n "rust-publish.sh metadata" <<<"$1" | head -n1 | cut -d: -f1); a=$(grep -n "crates-io-auth-action" <<<"$1" | head -n1 | cut -d: -f1); p=$(grep -n "rust-publish.sh publish" <<<"$1" | head -n1 | cut -d: -f1); [ -n "$m" ] && [ "$m" -lt "$a" ] && [ "$a" -lt "$p" ]' _ "$CJ"
check "the metadata gate checks homepage, repository, license, readme, keywords and categories" bash -c 'f="$1/scripts/rust-publish.sh"; grep -q "homepage" "$f" && grep -q "repository" "$f" && grep -q "license readme keywords categories" "$f" && grep -q "https://docs.lucasvmigotto.me/ai-gent/" "$f" && grep -q "https://github.com/lucasvmigotto/ai-gent" "$f"' _ "$ROOT"
check "publish order: libraries first, the binary last, each after an index wait, skipping versions already there" bash -c 'f="$1/scripts/rust-publish.sh"; grep -q "wait_for_index" "$f" && grep -q "already on crates.io; skipped" "$f" && grep -q "binary crate\|then the binary" "$f"' _ "$ROOT"
check "id-token: write appears exactly twice in release.yml: package and publish-crates" bash -c '[ "$(grep -c "id-token: write" <<<"$1")" -eq 2 ] && grep -q "id-token: write" <<<"$2" && grep -q "id-token: write" <<<"$3"' _ "$REL" "$PJ" "$CJ"
check "attestations: write appears once, in package" bash -c '[ "$(grep -c "attestations: write" <<<"$1")" -eq 1 ] && grep -q "attestations: write" <<<"$2"' _ "$REL" "$PJ"
check "contents: write appears only in release and package" bash -c '[ "$(grep -c "contents: write" <<<"$1")" -eq 2 ]' _ "$REL"
check "report-failure watches the new jobs" bash -c 'grep -Eq "needs: \[check, release, rust-build, package, publish-crates\]" <<<"$1"' _ "$REL"
check "the top-level permissions of release.yml stay contents: read" bash -c 'awk "/^permissions:/{getline; print; exit}" <<<"$1" | grep -q "contents: read"' _ "$REL"

# ----------------------------------------------------------------- check.yml and check.sh
echo "      -- check.yml and check.sh"
CY="$(strip "$WF/check.yml")"
check "cargo-deny is a pinned prebuilt release verified by SHA256, not built from source" bash -c 'grep -Eq "DENY_VERSION: [0-9]+\.[0-9]+\.[0-9]+$" <<<"$1" && grep -Eq "DENY_SHA256: [0-9a-f]{64}$" <<<"$1" && grep -q "sha256sum -c" <<<"$1" && ! grep -q "cargo install" <<<"$1"' _ "$CY"
check "the macos job runs test-install.sh and test-package.sh under /bin/bash" bash -c 'grep -q "/bin/bash scripts/test-install.sh" <<<"$1" && grep -q "/bin/bash scripts/test-package.sh" <<<"$1"' _ "$CY"
WJ="$(job "$WF/check.yml" windows)"
check "check.yml has a windows job on windows-2025 and windows-11-arm that is not fail-fast" bash -c 'grep -Fq "os: [windows-2025, windows-11-arm]" <<<"$1" && grep -q "fail-fast: false" <<<"$1" && grep -Fq "runs-on: \${{ matrix.os }}" <<<"$1"' _ "$WJ"
check "the windows job runs scripts\\test-install-ps1.ps1 under Windows PowerShell 5.1 and PowerShell 7" bash -c 'grep -q "shell: powershell" <<<"$1" && grep -q "shell: pwsh" <<<"$1" && [ "$(grep -c "test-install-ps1.ps1" <<<"$1")" -eq 2 ]' _ "$WJ"
check "the windows job reads no secrets and sets no write permission" bash -c '! grep -Eq "secrets[.:]|write" <<<"$1"' _ "$WJ"
check "check.sh runs this test and the publish test" bash -c 'grep -q "scripts/test-rust-workflows.sh" "$1/scripts/check.sh" && grep -q "scripts/test-rust-publish.sh" "$1/scripts/check.sh"' _ "$ROOT"

# ----------------------------------------------------------------- self-test: the checks can fail
if [[ -z ${RUST_WORKFLOWS_SELFTEST:-} ]]; then
  echo "      -- self-test (each mutation must make this script fail)"
  mutate() { # mutate <description> <file under .github/workflows> <sed expression>
    local what="$1" file="$2" expr="$3" tmp rc
    tmp="$(mktemp -d)"
    mkdir -p "$tmp/.github" "$tmp/scripts" "$tmp/rust/.cargo"
    cp -R "$ROOT/.github/workflows" "$tmp/.github/workflows"
    cp "$ROOT"/scripts/package-release.sh "$ROOT"/scripts/rust-publish.sh "$ROOT"/scripts/check.sh "$tmp/scripts/"
    cp "$ROOT/rust/.cargo/config.toml" "$tmp/rust/.cargo/"
    sed -i.bak "$expr" "$tmp/.github/workflows/$file"
    rm -f "$tmp/.github/workflows/$file.bak"
    RUST_WORKFLOWS_ROOT="$tmp" RUST_WORKFLOWS_SELFTEST=1 bash "$SELF" >/dev/null 2>&1
    rc=$?
    rm -rf "$tmp"
    if [[ $rc -ne 0 ]]; then ok "fails when $what"; else bad "did NOT fail when $what"; fi
  }
  mutate "a SHA pin in release.yml becomes a tag" release.yml '0,/actions\/checkout@[0-9a-f]\{40\} # v7.0.1/s//actions\/checkout@v7/'
  mutate "the crates job loses its variable gate" release.yml "s/vars.AI_GENT_CRATES_PUBLISH == 'true' \&\& //"
  mutate "the rehearsal gains contents: write" rust.yml 's/^  contents: read$/  contents: write/'
  mutate "the rehearsal stops being a dry run" rust.yml 's/dry_run: true/dry_run: false/'
  mutate "publish-crates gains a third id-token" release.yml 's/^      attestations: write.*$/      attestations: write\n      id-token: write/'
  mutate "the notes are edited after the release is published" release.yml 's/--notes-file dist\/release-notes.md/--title x/'
  mutate "cargo-deny goes back to cargo install" check.yml 's/DENY_SHA256: [0-9a-f]\{64\}/DENY_SHA256: nothing/'
fi

printf '%s/%s\n' "$((total - failed))" "$total"
exit $((failed > 0))
