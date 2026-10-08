#!/usr/bin/env bash
#
# update-check.sh — ai-gent update notice for agent-session start.
#
# Notify-only: prints one line when a newer release tag is known, nothing
# otherwise. Reads a timestamped cache file; refreshes it detached when
# stale, so the foreground path never touches the network. Always exits 0.
#
# Sourced by tests, executed by session-start hooks (Claude Code
# SessionStart; see plugins/git/hooks/hooks.json). Pinned checkouts
# (detached HEAD at a tag) stay silent — pinning is deliberate.
#
# Config (environment variables):
#   AI_GENT_UPDATE_DAYS      days between remote checks (default 7)
#   AI_GENT_NO_UPDATE_CHECK  set (any value) to disable the check entirely
#   AI_GENT_UPDATE_REPO      owner/repo slug (default: from origin, else
#                            lucasvmigotto/ai-gent)
#   AI_GENT_UPDATE_ROOT      ai-gent checkout root (default: detected from
#                            this file's location)

# _ai_gent_ver_gt <a> <b>: true when SemVer a > b (tolerates a leading v
# and missing parts; non-numeric suffixes are ignored).
_ai_gent_ver_gt() {
    local a=${1#v} b=${2#v}
    local va vb i an bn
    IFS='.' read -r -a va <<<"${a}" || true
    IFS='.' read -r -a vb <<<"${b}" || true
    for i in 0 1 2; do
        an="${va[$i]:-0}"
        bn="${vb[$i]:-0}"
        an="${an%%[^0-9]*}"
        bn="${bn%%[^0-9]*}"
        [[ -z "${an}" ]] && an=0
        [[ -z "${bn}" ]] && bn=0
        if ((10#${an} > 10#${bn})); then
            return 0
        fi
        if ((10#${an} < 10#${bn})); then
            return 1
        fi
    done
    return 1
}

# _ai_gent_update_root: prints the ai-gent checkout root, or fails.
# AI_GENT_UPDATE_ROOT overrides detection (tests, exotic layouts).
_ai_gent_update_root() {
    if [[ -n "${AI_GENT_UPDATE_ROOT:-}" ]]; then
        printf '%s' "${AI_GENT_UPDATE_ROOT}"
        return 0
    fi
    [[ -n "${BASH_SOURCE[0]:-}" ]] || return 1
    command -v readlink >/dev/null 2>&1 || return 1
    dirname "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")"
}

# _ai_gent_local_tag [root]: newest local SemVer tag, or fails.
_ai_gent_local_tag() {
    local root="${1:-$(_ai_gent_update_root)}" tag
    [[ -n "${root}" ]] || return 1
    tag="$(git -C "${root}" tag --list '[0-9]*.[0-9]*.[0-9]*' --sort=-v:refname 2>/dev/null | head -n 1)"
    [[ -n "${tag}" ]] || return 1
    printf '%s' "${tag}"
}

# _ai_gent_pinned [root]: true when HEAD is detached exactly at a tag.
_ai_gent_pinned() {
    local root="${1:-$(_ai_gent_update_root)}"
    [[ -n "${root}" ]] || return 1
    git -C "${root}" symbolic-ref --quiet HEAD >/dev/null 2>&1 && return 1
    git -C "${root}" describe --tags --exact-match HEAD >/dev/null 2>&1
}

# _ai_gent_repo_slug [root]: owner/repo for the remote release lookup.
_ai_gent_repo_slug() {
    if [[ -n "${AI_GENT_UPDATE_REPO:-}" ]]; then
        printf '%s' "${AI_GENT_UPDATE_REPO}"
        return 0
    fi
    local root="${1:-}" url slug
    url=""
    [[ -n "${root}" ]] && url="$(git -C "${root}" remote get-url origin 2>/dev/null)"
    if [[ "${url}" =~ github\.com[:/]([^/]+/[^/]+)(\.git)?$ ]]; then
        slug="${BASH_REMATCH[1]}"
        printf '%s' "${slug%.git}"
        return 0
    fi
    printf 'lucasvmigotto/ai-gent'
}

# _ai_gent_update_state_file: path of the timestamped cache file.
_ai_gent_update_state_file() {
    printf '%s' "${XDG_STATE_HOME:-${HOME}/.local/state}/ai-gent/update-check"
}

# _ai_gent_update_refresh: fetch the newest release tag into the cache.
# Silent and infallible by design (it runs detached in the background).
_ai_gent_update_refresh() {
    local root slug json tag now state tmp
    root="$(_ai_gent_update_root)" || return 0
    command -v curl >/dev/null 2>&1 || return 0
    slug="$(_ai_gent_repo_slug "${root}")"
    json="$(curl -fsSL --max-time 8 "https://api.github.com/repos/${slug}/releases/latest" 2>/dev/null)" || return 0
    tag="$(printf '%s' "${json}" | grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' | head -n 1 | cut -d'"' -f4)" || return 0
    [[ "${tag}" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+$ ]] || return 0
    now="$(date +%s 2>/dev/null)" || return 0
    state="$(_ai_gent_update_state_file)"
    mkdir -p -- "${state%/*}" 2>/dev/null || return 0
    tmp="$(mktemp "${state%/*}/update-check.XXXXXX" 2>/dev/null)" || return 0
    printf '%s %s\n' "${now}" "${tag}" >"${tmp}" || {
        rm -f -- "${tmp}"
        return 0
    }
    mv -- "${tmp}" "${state}" 2>/dev/null || rm -f -- "${tmp}"
    return 0
}

# _ai_gent_update_check: notify once per AI_GENT_UPDATE_DAYS when a newer
# release tag is known. Reads the cache; refreshes it detached when stale.
# Always silent except for the notice itself; never fails the caller.
_ai_gent_update_check() {
    [[ -z "${AI_GENT_NO_UPDATE_CHECK:-}" ]] || return 0
    local root local_tag state checked remote now maxage
    root="$(_ai_gent_update_root)" || return 0
    _ai_gent_pinned "${root}" && return 0
    state="$(_ai_gent_update_state_file)"
    now="$(date +%s 2>/dev/null)" || return 0
    maxage=$(( ${AI_GENT_UPDATE_DAYS:-7} * 86400 ))
    if [[ -s "${state}" ]]; then
        read -r checked remote <"${state}" || return 0
        if [[ ! "${checked}" =~ ^[0-9]+$ ]] || ((now - checked >= maxage)); then
            ( _ai_gent_update_refresh </dev/null >/dev/null 2>&1 & )
        fi
    else
        ( _ai_gent_update_refresh </dev/null >/dev/null 2>&1 & )
        return 0
    fi
    [[ -n "${remote:-}" ]] || return 0
    local_tag="$(_ai_gent_local_tag "${root}")" || return 0
    [[ -n "${local_tag}" ]] || return 0
    if _ai_gent_ver_gt "${remote}" "${local_tag}"; then
        printf '[ai-gent] update available: %s → %s — run scripts/ai-gent-update.sh in the ai-gent checkout (propose first, never self-apply in-session)\n' "${local_tag}" "${remote}"
    fi
    return 0
}

# Executed (not sourced): run the check once.
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    _ai_gent_update_check
fi
