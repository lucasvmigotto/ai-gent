#!/bin/sh
# PreToolUse guard for the Bash tool: database access goes through dbrun
# (plugins/db/scripts/dbrun.py), which classifies the target, refuses writes
# to anything but a proven local container, masks results and logs every
# statement.
#
#   deny  a direct database client (psql, mysql, sqlcmd, sqlplus, sqlite3,
#         mongosh, ...) whose command carries SQL that writes, or a restore
#         tool (pg_restore, impdp, ...)
#   ask   any other direct client use — the user decides (other plugins,
#         such as a private one, may still rely on a client)
#   deny  any tool touching the credentials file (connections.env or the
#         ai-gent config directory): only dbrun reads it
#
# Pattern matching on the raw command: it catches the commands an agent
# writes, not every possible obfuscation; dbrun's own checks are the
# primary guard.

set -u

input="$(tr '\n' ' ')"

decide() { # decide deny|ask <reason>
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"%s","permissionDecisionReason":"db guard: %s"}}\n' "$1" "$2"
  exit 0
}

# The credentials file: nothing but dbrun reads it, with any tool.
creds='connections\.env|\.config/ai-gent|XDG_CONFIG_HOME\}?/ai-gent|AI_GENT_DB_CONFIG'
if printf '%s' "$input" | grep -Eq "$creds"; then
  decide deny "that's the db credentials file; only dbrun reads it — use dbrun profiles / classify / test"
fi

cmd="$(printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p')"
[ -n "$cmd" ] || exit 0

clients='psql|pg_dump|pg_dumpall|pg_restore|pgcli|mysql|mariadb|mysqldump|mariadb-dump|mysqladmin|mycli|sqlcmd|mssql-cli|bcp|osql|isql|sqlplus|sql|sqlcl|rman|impdp|expdp|imp|exp|mongosh|mongo|mongodump|mongorestore|mongoimport|sqlite3|litecli|usql|cqlsh|clickhouse-client'
restore_tools='pg_restore|impdp|imp|mongorestore|mongoimport'

# One simple command per line, quoted strings blanked (so SQL text inside
# quotes can't look like a command), split on ; & | ( ) and newlines.
segments="$(printf '%s\n' "$cmd" | awk '{
  gsub(/\\"([^\\]|\\[^"])*\\"/, "\"\""); gsub(/\047[^\047]*\047/, "\047\047")
  gsub(/\\n/, "\n"); gsub(/[;&|()]/, "\n"); print }')"

# A client as the command word: at the start (after sudo, env assignments,
# timeout/nice/exec), or inside `docker|podman|nerdctl|kubectl exec|run ...`.
prefix='^[[:space:]]*((sudo([[:space:]]+-[^[:space:]]+([[:space:]]+[^-[:space:]][^[:space:]]*)?)*|env|exec|nice|timeout[[:space:]]+[0-9smh]+|[A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*)[[:space:]]+)*'
direct="$prefix([^[:space:]]*/)?($clients)([[:space:]]|\$)"
wrapped="(docker|podman|nerdctl|kubectl|oc)[[:space:]].*(exec|run)[[:space:]].*[[:space:]/]($clients)([[:space:]]|\$)"

# Every segment is checked on its own: a dbrun call elsewhere in the
# command line exempts nothing.
hit="$(printf '%s\n' "$segments" | grep -E -e "$direct" -e "$wrapped" | head -n1)"
[ -n "$hit" ] || exit 0

tool="$(printf '%s\n' "$hit" | grep -oE "(^|[[:space:]/])($clients)([[:space:]]|\$)" | head -n1 | tr -d ' /')"
if printf '%s\n' "$tool" | grep -Eqx "$restore_tools"; then
  decide deny "$tool restores into a database; local restores go through dbrun restore, anything else is a script for a person to run"
fi
# The SQL usually sits in the quotes or a heredoc: look at the whole command.
if printf '%s' "$cmd" | sed 's/\\n/ /g; s/\\t/ /g' | grep -Eiq '(^|[^A-Za-z_])(INSERT|UPDATE|DELETE|MERGE|UPSERT|TRUNCATE|DROP|ALTER|CREATE|GRANT|REVOKE|RENAME|REPLACE[[:space:]]+INTO)([^A-Za-z_]|$)'; then
  decide deny "a direct $tool call with SQL that writes; use dbrun (plan/apply on a proven local database, script for anything else)"
fi
decide ask "$tool bypasses dbrun (no classification, masking or command log); allow only if the user wants this direct call"
