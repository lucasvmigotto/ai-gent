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

expect_tool() { # expect_tool deny|none <tool> <tool_input JSON>
  local want="$1" tool="$2" tinput="$3" out got
  out="$(python3 -c 'import json,sys; print(json.dumps({"hook_event_name":"PreToolUse","tool_name":sys.argv[1],"cwd":"/tmp","tool_input":json.loads(sys.argv[2])}))' "$tool" "$tinput" | sh "$GUARD")"
  got="$(printf '%s' "$out" | sed -n 's/.*"permissionDecision":"\([a-z]*\)".*/\1/p')"
  got="${got:-none}"
  if [[ "$got" == "$want" ]]; then
    printf '      ok    %-5s %s %s\n' "$want" "$tool" "$tinput"
  else
    printf '      FAIL  want %s, got %s: %s %s\n' "$want" "$got" "$tool" "$tinput"
    failures=$((failures + 1))
  fi
}

# The credentials file, with any tool.
expect deny 'cat ~/.config/ai-gent/db/connections.env'
expect deny 'grep -i password "$HOME/.config/ai-gent/db/connections.env"'
expect deny 'ls ${XDG_CONFIG_HOME}/ai-gent/db'
expect deny 'echo $AI_GENT_DB_CONFIG'
expect deny 'python3 /x/plugins/db/scripts/dbrun.py profiles && cat connections.env'
expect_tool deny Read '{"file_path": "/home/me/.config/ai-gent/db/connections.env"}'
expect_tool deny Grep '{"pattern": "PASSWORD", "path": "/home/me/.config/ai-gent"}'
expect_tool deny Glob '{"pattern": "**/connections.env"}'
expect_tool deny Edit '{"file_path": "/home/me/.config/ai-gent/db/connections.env", "old_string": "a", "new_string": "b"}'
expect_tool none Read '{"file_path": "/home/me/project/.env.example"}'
expect_tool none Grep '{"pattern": "DATABASE_URL", "path": "src/"}'
# A dbrun call elsewhere on the line exempts nothing.
expect deny 'python3 /x/plugins/db/scripts/dbrun.py profiles; psql -c "DELETE FROM orders"'
expect ask 'python3 /x/plugins/db/scripts/dbrun.py profiles && psql -l'

exit $((failures > 0))
