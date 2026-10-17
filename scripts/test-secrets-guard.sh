#!/usr/bin/env bash
# Tests plugins/secrets/hooks/guard.sh with hook-event JSON for commands that must
# be denied, confirmed, or left alone. Called by scripts/check.sh.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GUARD="$REPO_DIR/plugins/secrets/hooks/guard.sh"

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


# Values: printed, copied, shown as a QR code, edited or searched.
expect deny 'pass show ai-gent/db/shop-dev'
expect deny 'pass ai-gent/github/pat'
expect deny 'pass -c ai-gent/github/pat'
expect deny 'pass show --clip=2 ai-gent/github/pat'
expect deny 'pass show -q work/vpn'
expect deny 'pass edit ai-gent/azure-devops/pat'
expect deny 'pass grep token'
expect deny 'pass generate ai-gent/x/y 32'
expect deny 'pass otp ai-gent/x/totp'
expect deny 'PASSWORD_STORE_DIR=/tmp/s pass show x | head -1'
expect deny 'echo ok && pass show "ai-gent/db/shop dev"'
expect deny 'sudo -u me pass show x'
expect deny 'gpg -d ~/.password-store/ai-gent/github/pat.gpg'
expect deny 'cat "$PASSWORD_STORE_DIR/x.gpg" | gpg --decrypt'
# Writing a credential is the user's job.
expect deny 'pass insert ai-gent/db/shop-dev'
expect deny 'printf "%s\n" "$TOKEN" | pass insert -m ai-gent/github/pat'
# Store changes: the user decides.
expect ask 'pass rm ai-gent/db/old'
expect ask 'pass mv ai-gent/db/a ai-gent/db/b'
expect ask 'pass init 0xDEADBEEF'
# pass-run: db entries belong to dbrun; no printing the environment.
expect deny 'pass-run PGPASSWORD=ai-gent/db/shop-dev -- psql -h db -l'
expect deny 'pass-run GH_TOKEN=ai-gent/github/pat -- env'
expect deny 'pass-run GH_TOKEN=ai-gent/github/pat -- sh -c "echo $GH_TOKEN"'
expect deny 'pass-run X=ai-gent/github/pat -- /usr/bin/printenv X'
expect none 'pass-run GH_TOKEN=ai-gent/github/pat -- gh pr list'
expect none 'pass-run GH_TOKEN=ai-gent/github/pat -- gh api -- repos/x/y'
# Names only, and text that just mentions pass.
expect none 'pass'
expect none 'pass ls'
expect none 'pass ls ai-gent'
expect none 'pass find github'
expect none 'pass --version'
expect none 'command -v pass'
expect none 'which pass gpg'
expect none 'git commit -m "docs: pass show prints the secret"'
expect none 'echo "use pass show" > notes.txt'
expect none 'grep -rn "pass show" docs/'
expect none 'gpg --list-secret-keys'
expect none 'python3 /x/plugins/db/scripts/dbrun.py test SHOP_DEV --reason check'
expect none 'bypass --flag'
expect none 'ls -la'

# Team stores sync through git: reads and fast-forwards are routine.
expect none 'pass git pull'
expect none 'pass git fetch origin'
expect none 'pass git status'
expect none 'PASSWORD_STORE_DIR=/srv/team pass git log --oneline -5'
expect ask 'pass git push'
expect ask 'pass git reset --hard origin/main'
expect ask 'pass init -p db/team AAAA BBBB'

exit $((failures > 0))
