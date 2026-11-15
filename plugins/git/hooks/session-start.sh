#!/bin/sh
# SessionStart hook: ai-gent's update notice (`ai-gent update --check` in a package
# install, scripts/update-check.sh otherwise; silent when there is nothing to say).
#
# The notice reaches the model as additionalContext and the person as a systemMessage
# the engine shows itself, so no hooks module has to relay it (a session.append hook
# on the hook-context door is passed over where cc-plugin-sec-default is seated,
# decided 2026-11-14). The plugin option sessionToasts=false keeps it for the model
# only, printed as plain text as before. Always exits 0.

r="${CLAUDE_PLUGIN_ROOT:-${0%/*}/..}"
b="$r/../../bin/ai-gent"
f="$r/../../scripts/update-check.sh"
out=
if [ -x "$b" ]; then
  out="$("$b" update --check 2>/dev/null)"
elif [ -f "$f" ]; then
  out="$(bash "$f" 2>/dev/null)"
fi
[ -n "$out" ] || exit 0

case "${CLAUDE_PLUGIN_OPTION_sessionToasts:-}${CLAUDE_PLUGIN_OPTION_SESSIONTOASTS:-}" in
  *false*)
    printf '%s\n' "$out"
    exit 0
    ;;
esac

# JSON string text: control characters dropped, backslash, quote and tab escaped, lines joined by \n.
esc() {
  printf '%s' "$1" | tr -d '\000-\010\013-\037' | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' -e 's/	/\\t/g' |
    awk 'NR > 1 { printf "\\n" } { printf "%s", $0 }'
}
line="$(printf '%s\n' "$out" | grep -m1 '^\[ai-gent\] update available')"
if [ -n "$line" ]; then
  printf '{"systemMessage":"%s","hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}\n' "$(esc "$line")" "$(esc "$out")"
else
  printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}\n' "$(esc "$out")"
fi
exit 0
