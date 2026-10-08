#!/usr/bin/env bash
#
# ai-gent-update.sh — move the ai-gent checkout to the newest release tag.
#
# Run by path from inside the checkout (an agent session proposes it; the
# user approves; it takes effect next session — never self-apply inside
# the session that noticed the update):
#
#   scripts/ai-gent-update.sh           # newest release tag (or fast-forward
#                                       # the current branch); pinned stays put
#   scripts/ai-gent-update.sh <ref>     # a branch, tag, or sha
#
# AI_GENT_UPDATE_ROOT overrides checkout detection (tests).

set -euo pipefail

say() { printf 'ai-gent-update: %s\n' "$*"; }
die() { printf 'ai-gent-update: error: %s\n' "$*" >&2; exit 1; }

root="${AI_GENT_UPDATE_ROOT:-$(dirname "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")")}"
[[ -d "$root/.git" ]] || die "$root is not a git checkout — re-clone https://github.com/lucasvmigotto/ai-gent.git"
[[ -n "$(git -C "$root" status --porcelain)" ]] && die "$root has local changes — stash or commit them first"

newest_tag() {
    git -C "$root" tag --list '[0-9]*.[0-9]*.[0-9]*' --sort=-v:refname 2>/dev/null | head -n 1
}

if [[ -n "${1:-}" ]]; then
    # Explicit ref: tags detach, branches fast-forward (install.sh's update_to).
    ref="$1"
    before="$(git -C "$root" describe --tags --always)"
    if git -C "$root" rev-parse --quiet --verify "refs/tags/$ref" >/dev/null; then
        git -C "$root" checkout --quiet --detach "refs/tags/$ref" || die "couldn't check out $ref"
    else
        git -C "$root" checkout --quiet "$ref" || die "couldn't check out $ref"
        git -C "$root" pull --ff-only --quiet origin "$ref" || die "couldn't fast-forward $ref — update it by hand"
    fi
    after="$(git -C "$root" describe --tags --always)"
    if [[ "$before" == "$after" ]]; then
        say "already at $after"
    else
        say "updated $before → $after"
    fi
elif git -C "$root" symbolic-ref --quiet HEAD >/dev/null 2>&1; then
    # On a branch: fast-forward it, like install.sh.
    before="$(git -C "$root" describe --tags --always)"
    git -C "$root" fetch --quiet --tags origin || die "couldn't fetch — not updating, staying on $before"
    git -C "$root" pull --ff-only --quiet || die "couldn't fast-forward — not updating, staying on $before"
    after="$(git -C "$root" describe --tags --always)"
    if [[ "$before" == "$after" ]]; then
        say "already at $after"
    else
        say "updated $before → $after"
    fi
else
    # Detached: only a tag equal to the newest release is "already at";
    # anything else is a pin (or a stray sha) that needs an explicit ref.
    git -C "$root" fetch --quiet --tags origin || die "couldn't fetch — not updating"
    newest="$(newest_tag)"
    [[ -n "$newest" ]] || die "no release tags found — staying put"
    current="$(git -C "$root" describe --tags --exact-match HEAD 2>/dev/null || true)"
    if [[ "$current" == "$newest" ]]; then
        say "already at $newest"
    elif [[ -n "$current" ]]; then
        die "$root is pinned to $current ($newest available) — pass a ref to move it"
    else
        die "$root is detached at $(git -C "$root" rev-parse --short HEAD) — pass a ref to move it"
    fi
fi
say "re-run setup.sh to pick up added, renamed or removed skills"
