# >>> guard launcher (shared/guard-launcher.sh, spec 033: package installs call the binary; a clone runs the shell logic below)
r="${CLAUDE_PLUGIN_ROOT:-${0%/*}/..}"
if [ -f "$r/../../VERSION" ]; then
  b="$r/../../bin/ai-gent"; [ -x "$b" ] || b="$(command -v ai-gent 2>/dev/null)" || b=
  rc=0 out=
  if [ -z "$b" ]; then rc=127; else
    set -- "$b" guard --guard @ID@ --format claude
    command -v timeout >/dev/null 2>&1 && set -- timeout 2 "$@"
    out="$("$@" 2>/dev/null)" || rc=$?
  fi
  if [ "$rc" != 0 ]; then
    case "$rc" in 127) w="it is missing" ;; 124) w="it took over 2 s" ;; *) w="it failed with exit $rc" ;; esac
    printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"@MARKER@ the ai-gent binary is unavailable (%s); run: curl -fsSL https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.sh | sh. Allow only if you accept this command unguarded."}}\n' "$w"
  elif [ -n "$out" ]; then printf '%s\n' "$out"; fi
  exit 0
fi
# <<< guard launcher
