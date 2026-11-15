#!/bin/sh
# Rehearse the project-level rollback of specs/039-release-3-contract (T051) in a SCRATCH
# GitHub repository: does `gh release edit <tag> --prerelease` followed by `gh release edit
# <tag> --latest` change which release `releases/latest` (what install.sh resolves) points to?
#
#   scripts/rehearse-rollback.sh [<owner/scratch-repo>]           dry run (default): prints the plan
#   scripts/rehearse-rollback.sh --run <owner/scratch-repo> [--good X.Y.Z] [--bad X.Y.Z]
#                                [--transcript FILE]
#
# The dry run prints the exact gh commands and what install.sh should resolve after each, and
# touches nothing (no network, gh is never started). --run executes them, only against the
# repository you name: there is no default and none is read from a git remote. It refuses
#   - a missing, malformed or unnamed repository,
#   - lucasvmigotto/ai-gent (any case) and any repository called ai-gent,
#   - a repository whose name does not contain "scratch" (name it so on purpose).
# It creates the two rehearsal releases when absent, edits their flags, and never deletes
# anything (no `gh release delete`, no tag deletion). Every command and answer goes to the
# transcript file (default ./rollback-rehearsal-<UTC time>.txt, never overwritten).
#
# install.sh resolves `latest` from the redirect of https://github.com/<repo>/releases/latest;
# this script asks the equivalent API (`gh api repos/<repo>/releases/latest`), which also works
# for a private scratch repository. Exit: 0 all expectations held, 1 an expectation failed or a
# command failed, 2 usage or refusal.
set -eu

REAL_SLUG="lucasvmigotto/ai-gent"
GOOD="9.9.0"
BAD="9.9.1"
MODE=dry
REPO=""
TRANSCRIPT=""

usage() { sed -n '2,/^set -eu/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//'; }
refuse() { printf 'rehearse-rollback: refused: %s\n' "$1" >&2; exit 2; }

lower() { printf '%s' "$1" | tr '[:upper:]' '[:lower:]'; }

is_version() { printf '%s' "$1" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; }

check_repo() {
  r="$1"
  printf '%s' "$r" | grep -Eq '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$' ||
    refuse "'$r' is not owner/repo"
  l="$(lower "$r")"
  [ "$l" != "$(lower "$REAL_SLUG")" ] || refuse "$REAL_SLUG is the real project; this rehearsal never touches it"
  [ "${l#*/}" != "ai-gent" ] || refuse "a repository called ai-gent is not a scratch repository"
  case "${l#*/}" in
    *scratch*) ;;
    *) refuse "the repository name must contain 'scratch' (got '$r'); create a throwaway repo named that way" ;;
  esac
}

while [ $# -gt 0 ]; do
  case "$1" in
    -h | --help) usage; exit 0 ;;
    --run)
      MODE=run
      [ $# -ge 2 ] || refuse "--run needs the scratch repository, owner/repo"
      case "$2" in -*) refuse "--run needs the scratch repository, owner/repo" ;; esac
      REPO="$2"; shift 2 ;;
    --good) [ $# -ge 2 ] || refuse "--good needs a version"; GOOD="$2"; shift 2 ;;
    --bad) [ $# -ge 2 ] || refuse "--bad needs a version"; BAD="$2"; shift 2 ;;
    --transcript) [ $# -ge 2 ] || refuse "--transcript needs a file"; TRANSCRIPT="$2"; shift 2 ;;
    -*) printf 'rehearse-rollback: unknown option %s\n' "$1" >&2; usage >&2; exit 2 ;;
    *)
      [ "$MODE" = dry ] && [ -z "$REPO" ] || refuse "unexpected argument '$1'"
      REPO="$1"; shift ;;
  esac
done

is_version "$GOOD" || refuse "--good '$GOOD' is not x.y.z"
is_version "$BAD" || refuse "--bad '$BAD' is not x.y.z"
[ "$GOOD" != "$BAD" ] || refuse "--good and --bad must differ"
if [ -n "$REPO" ]; then check_repo "$REPO"; fi

SHOW_REPO="${REPO:-<owner/scratch-repo>}"

# the plan, shared by the dry run and --run: "label|command shown|expected resolve (empty = no assertion)"
# The first four lines are the T051 sequence; "arm" makes the state independent of creation order.
plan() {
  cat <<EOF
create-good~gh release view $GOOD -R $SHOW_REPO || gh release create $GOOD -R $SHOW_REPO --title $GOOD --notes "rollback rehearsal: last good"~
create-bad~gh release view $BAD -R $SHOW_REPO || gh release create $BAD -R $SHOW_REPO --title $BAD --notes "rollback rehearsal: defective"~
arm~gh release edit $BAD -R $SHOW_REPO --prerelease=false --latest~$BAD
prerelease~gh release edit $BAD -R $SHOW_REPO --prerelease~NOT:$BAD
latest~gh release edit $GOOD -R $SHOW_REPO --latest~$GOOD
undo~gh release edit $BAD -R $SHOW_REPO --prerelease=false --latest~$BAD
EOF
}

dry_run() {
  printf 'DRY RUN (nothing is executed, no network). Repository: %s\n' "$SHOW_REPO"
  printf 'Last good release: %s. Defective release: %s.\n\n' "$GOOD" "$BAD"
  printf 'Before: gh auth status; gh repo view %s --json nameWithOwner,isPrivate (must exist; the repo needs one commit)\n' "$SHOW_REPO"
  printf 'After every step the probe is: gh api repos/%s/releases/latest --jq .tag_name\n' "$SHOW_REPO"
  printf '(install.sh gets the same tag from the redirect of https://github.com/%s/releases/latest)\n\n' "$SHOW_REPO"
  n=0
  plan | while IFS="~" read -r label cmd want; do
    n=$((n + 1))
    printf 'step %s (%s)\n  $ %s\n' "$n" "$label" "$cmd"
    case "$want" in
      "") printf '  install.sh should resolve: (unchanged; creation only)\n' ;;
      NOT:*) printf '  install.sh should resolve: anything but %s (the previous latest, %s, is expected)\n' "${want#NOT:}" "$GOOD" ;;
      *) printf '  install.sh should resolve: %s\n' "$want" ;;
    esac
  done
  printf '\nNothing is ever deleted. To execute: %s --run <owner/scratch-repo-name-with-scratch-in-it>\n' "$0"
}

if [ "$MODE" = dry ]; then dry_run; exit 0; fi

# ---- --run -------------------------------------------------------------------------------
command -v gh >/dev/null 2>&1 || refuse "gh is not installed"
if [ -z "$TRANSCRIPT" ]; then TRANSCRIPT="./rollback-rehearsal-$(date -u +%Y%m%dT%H%M%SZ).txt"; fi
[ ! -e "$TRANSCRIPT" ] || refuse "$TRANSCRIPT exists; the transcript is never overwritten"
: >"$TRANSCRIPT"
unset GH_REPO

log() { printf '%s\n' "$*" | tee -a "$TRANSCRIPT"; }
failures=0

# run_gh <args...>: echo the command, run it, record the output and the exit status
run_gh() {
  log "\$ gh $*"
  rc=0
  out="$(gh "$@" 2>&1)" || rc=$?
  [ -z "$out" ] || log "$out"
  log "  (exit $rc)"
  return "$rc"
}

probe() {
  rc=0
  got="$(gh api "repos/$REPO/releases/latest" --jq .tag_name 2>&1)" || rc=$?
  if [ "$rc" -ne 0 ]; then log "  probe failed (exit $rc): $got"; got="(none)"; fi
  printf '%s' "$got"
}

log "rollback rehearsal, started $(date -u +%Y-%m-%dT%H:%M:%SZ), repository $REPO, good $GOOD, bad $BAD"
run_gh auth status || { log "gh is not logged in"; exit 1; }
canon=""
rc=0
canon="$(gh repo view "$REPO" --json nameWithOwner --jq .nameWithOwner 2>&1)" || rc=$?
[ "$rc" -eq 0 ] || { log "gh repo view failed: $canon"; exit 1; }
log "\$ gh repo view $REPO --json nameWithOwner => $canon"
# the canonical name can differ from the typed one (renamed or transferred): check it again
check_repo "$canon"

n=0
plan_text="$(plan)"
while IFS="~" read -r label cmd want; do
  n=$((n + 1))
  log ""
  log "step $n ($label)"
  case "$label" in
    create-good | create-bad)
      tag="$GOOD"; [ "$label" = create-bad ] && tag="$BAD"
      if gh release view "$tag" -R "$REPO" >/dev/null 2>&1; then
        log "  release $tag already exists; left as it is"
      else
        run_gh release create "$tag" -R "$REPO" --title "$tag" --notes "rollback rehearsal" || failures=$((failures + 1))
      fi ;;
    arm | undo) run_gh release edit "$BAD" -R "$REPO" --prerelease=false --latest || failures=$((failures + 1)) ;;
    prerelease) run_gh release edit "$BAD" -R "$REPO" --prerelease || failures=$((failures + 1)) ;;
    latest) run_gh release edit "$GOOD" -R "$REPO" --latest || failures=$((failures + 1)) ;;
  esac
  [ -n "$want" ] || continue
  got="$(probe)"
  case "$want" in
    NOT:*) bad="${want#NOT:}"
      if [ "$got" != "$bad" ]; then log "  resolves: $got  [ok: not $bad]"; else log "  resolves: $got  [MISMATCH: expected anything but $bad]"; failures=$((failures + 1)); fi ;;
    *) if [ "$got" = "$want" ]; then log "  resolves: $got  [ok]"; else log "  resolves: $got  [MISMATCH: expected $want]"; failures=$((failures + 1)); fi ;;
  esac
done <<EOF
$plan_text
EOF

log ""
if [ "$failures" -eq 0 ]; then log "RESULT: every expectation held (nothing was deleted)"; else log "RESULT: $failures expectation(s) or command(s) failed"; fi
log "transcript: $TRANSCRIPT"
[ "$failures" -eq 0 ]
