#!/usr/bin/env bash
# Tests plugins/repo/hooks/guard.sh with hook-event JSON for commands that must
# be denied, confirmed, or left alone. Called by scripts/check.sh.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GUARD="$REPO_DIR/plugins/repo/hooks/guard.sh"

failures=0
expect() { # expect deny|ask|none <command>
  local want="$1" cmd="$2" out got
  out="$(python3 -c 'import json,sys; print(json.dumps({"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":"/tmp","tool_input":{"command":sys.argv[1]}}))' "$cmd" | sh "$GUARD")"
  got="$(printf '%s' "$out" | sed -n 's/.*"permissionDecision":"\([a-z]*\)".*/\1/p')"
  got="${got:-none}"
  if [[ "$got" == "$want" ]]; then
    printf '      ok    %-5s %s\n' "$want" "${cmd//$'\n'/⏎}"
  else
    printf '      FAIL  want %s, got %s: %s\n' "$want" "$got" "${cmd//$'\n'/⏎}"
    failures=$((failures + 1))
  fi
}


R=/home/me/ai-gent/plugins/repo/scripts/reporun.py
expect ask "python3 $R cli github --write -- pr merge 3 --squash"
expect ask "python3 $R api github POST /repos/o/r/issues --data body.json --write"
expect ask "python3 $R cli azure-devops --write -- boards work-item update --id 12 --state Done"
expect ask "cd /x && python3 $R api jira PUT /rest/api/3/issue/X-1 --data - --write < f.json"
expect ask "reporun cli gitlab --write -- mr note 4 -m ok"
expect deny "python3 $R cli github --write -- issue comment 3 --body 'ok

Co-Authored-By: Someone <a@b.c>'"
expect deny "python3 $R cli github --write -- pr comment 3 --body 'looks good

🤖 Generated with [Claude Code](https://claude.com/claude-code)'"
expect ask "python3 $R cli github --write -- issue comment 3 --body 'denies Co-Authored-By trailers'"
expect none "python3 $R detect"
expect none "python3 $R cli github -- pr list"
expect none "python3 $R api github GET /repos/o/r/issues --paginate"
expect none "python3 $R api github GET /search/issues --query 'q=--write'"
expect none 'git commit -m "docs: reporun --write asks first"'
expect none 'echo reporun --write'
expect none 'ls -la'

exit $((failures > 0))
