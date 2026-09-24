#!/usr/bin/env bash
# Tests plugins/git/hooks/guard.sh with hook-event JSON for commands that
# must be denied, confirmed, or left alone. Called by scripts/check.sh.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GUARD="$REPO_DIR/plugins/git/hooks/guard.sh"
SANDBOX="$(mktemp -d)"
trap 'rm -rf "$SANDBOX"' EXIT

# A repo on main and one on a feature branch, to test the branch check.
for b in main feat/x; do
  d="$SANDBOX/${b//\//-}"
  git init -q -b "$b" "$d"
done

failures=0
expect() { # expect deny|ask|none <cwd> <command>
  local want="$1" cwd="$2" cmd="$3" out got
  out="$(python3 -c 'import json,sys; print(json.dumps({"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":sys.argv[1],"tool_input":{"command":sys.argv[2],"description":"x"}}))' "$cwd" "$cmd" | sh "$GUARD")"
  got="$(printf '%s' "$out" | sed -n 's/.*"permissionDecision":"\([a-z]*\)".*/\1/p')"
  got="${got:-none}"
  if [[ "$got" == "$want" ]]; then
    printf '      ok    %-5s %s\n' "$want" "${cmd//$'\n'/⏎}"
  else
    printf '      FAIL  want %s, got %s: %s\n' "$want" "$got" "${cmd//$'\n'/⏎}"
    failures=$((failures + 1))
  fi
}

main="$SANDBOX/main"
feat="$SANDBOX/feat-x"

expect deny "$feat" 'git commit --no-verify -m "fix: x"'
expect deny "$feat" 'git commit -n -m "fix: x"'
expect deny "$feat" 'git -C /tmp/repo merge --no-verify feat/x'
expect deny "$feat" "git commit -m \"\$(cat <<'EOF'
feat: x

Co-Authored-By: Someone <a@b.c>
EOF
)\""
expect deny "$feat" 'git push --force origin feat/x'
expect deny "$feat" 'git push -f'
expect ask "$feat" 'git push --force-with-lease origin feat/x'
expect ask "$feat" 'git push origin feat/x'
expect ask "$feat" 'cd /tmp && git push'
expect ask "$feat" 'git fetch && git push origin --delete feat/old'
expect ask "$feat" 'gh pr create --fill'
expect ask "$feat" 'git branch -D feat/old'
expect ask "$feat" 'git reset --hard HEAD~1'
expect ask "$feat" 'git clean -fd'
expect ask "$feat" 'git commit --amend --no-edit'
expect ask "$feat" 'git filter-repo --mailmap m'
expect ask "$main" 'git commit -m "fix: x"'
expect ask "$main" 'git merge --ff-only feat/x'
expect ask "$feat" "git -C $main commit -m \"fix: x\""
expect none "$feat" 'git commit -m "fix: x"'
expect none "$feat" 'git commit -m "feat(git): block --no-verify and -n in the guard"'
expect none "$feat" 'git commit -m "docs: explain git push --force"'
expect none "$feat" 'git merge --no-ff feat/x-db'
expect none "$feat" 'git branch -d feat/x-db'
expect none "$feat" 'git status && git log --oneline -5'
expect none "$feat" 'echo "git push" > notes.txt'
expect none "$feat" 'grep -rn -- --no-verify docs/'
expect none "$feat" 'git push-notes-helper'
expect none "$main" 'git switch -c feat/new'
expect none "$feat" 'ls -la'

exit $((failures > 0))
