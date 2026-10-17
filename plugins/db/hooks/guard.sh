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
#   deny  any tool whose target is a credentials file (connections.env or
#         .toml, the ai-gent db config directory, its variables): only dbrun
#         reads them. Text that merely names them is fine
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

# The credentials files: nothing but dbrun reads them, with any tool. Only a
# path is checked — a tool's target, or a word on a command line — never the
# text a Write or Edit puts into a file, so a document may name them; and a
# name with more after it (an .example file) isn't the real file.
creds='(^|[^A-Za-z0-9_.-])connections\.(env|toml)($|[^A-Za-z0-9_.-])|\.config/ai-gent($|[^A-Za-z0-9_.-])|XDG_CONFIG_HOME\}?/ai-gent($|[^A-Za-z0-9_.-])|AI_GENT_DB_(CONFIG|CONNECTIONS)'
creds_reason="that's a db credentials file; only dbrun reads it — use dbrun profiles / classify / test"
tool="$(printf '%s' "$input" | sed -nE 's/.*"tool_name"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p')"
if [ "$tool" != Bash ]; then
  keys='file_path|notebook_path|path|glob'
  [ "$tool" = Glob ] && keys="$keys|pattern"
  if printf '%s' "$input" | grep -oE "\"($keys)\"[[:space:]]*:[[:space:]]*\"([^\"\\\\]|\\\\.)*\"" | grep -Eq "$creds"; then
    decide deny "$creds_reason"
  fi
  exit 0
fi

cmd="$(printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p')"
[ -n "$cmd" ] || exit 0

# Quoted text with spaces in it (a commit message, an echo) is prose, not a
# path: blank it, scanning quotes left to right so each pairs with its own
# closer; a quoted word without spaces keeps its text.
words="$(printf '%s\n' "$cmd" | awk '{
  out = ""; n = length($0); i = 1
  while (i <= n) {
    c = substr($0, i, 1)
    if (c == "\\" && substr($0, i + 1, 1) == "\"") {
      j = i + 2; t = ""
      while (j <= n && !(substr($0, j, 1) == "\\" && substr($0, j + 1, 1) == "\"")) {
        if (substr($0, j, 1) == "\\") { t = t substr($0, j, 2); j += 2 } else { t = t substr($0, j, 1); j++ }
      }
      out = out (t ~ /[[:space:]]/ ? "\"\"" : t); i = j + 2; continue
    }
    if (c == "\047") {
      j = i + 1; t = ""
      while (j <= n && substr($0, j, 1) != "\047") { t = t substr($0, j, 1); j++ }
      out = out (t ~ /[[:space:]]/ ? "\047\047" : t); i = j + 1; continue
    }
    out = out c; i++
  }
  print out }')"
if printf '%s' "$words" | grep -Eq "$creds"; then
  decide deny "$creds_reason"
fi

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
