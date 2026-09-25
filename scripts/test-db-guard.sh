#!/usr/bin/env bash
# Tests plugins/db/hooks/guard.sh with hook-event JSON for commands that must
# be denied, confirmed, or left alone. Called by scripts/check.sh.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GUARD="$REPO_DIR/plugins/db/hooks/guard.sh"

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

expect deny 'psql -h db.internal -U app -c "DELETE FROM orders WHERE id = 1"'
expect deny "mysql -u root shop -e 'update customer set status = 1'"
expect deny 'docker exec -i shop-db-1 psql -U app -c "DROP TABLE t"'
expect deny 'podman exec db sqlplus -s system/x@FREEPDB1 <<EOF
TRUNCATE TABLE audit;
EOF'
expect deny 'sqlcmd -S prod -Q "ALTER TABLE t ADD c int"'
expect deny 'pg_restore -d shop backup.dump'
expect deny 'kubectl exec -it pg-0 -- psql -c "insert into t values (1)"'
expect deny 'PGPASSWORD=x psql -h 10.0.0.5 -c "grant all on t to u"'
expect ask 'psql -h db.internal -U app -c "SELECT id FROM orders LIMIT 5"'
expect ask 'sudo -u postgres psql'
expect ask '/usr/lib/postgresql/17/bin/psql -l'
expect ask 'docker compose exec db psql -U app -d shop'
expect ask 'sqlite3 app.db ".tables"'
expect ask 'mongosh "mongodb://localhost/test" --eval "db.stats()"'
expect ask 'timeout 10s mysql -e "SELECT 1"'
expect none 'python3 /home/me/ai-gent/plugins/db/scripts/dbrun.py query DEVDB --sql q.sql --reason "check"'
expect none 'echo "psql is installed" && ls'
expect none 'grep -rn "DELETE FROM" src/'
expect none 'which psql'
expect none 'command -v sqlcmd'
expect none 'npm run sql:lint'
expect none 'cat queries/report.sql'
expect none 'git commit -m "fix: guard mysql client calls"'
expect none 'ls -la'

exit $((failures > 0))
