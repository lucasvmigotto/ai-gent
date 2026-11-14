#!/bin/sh
# SessionStart hook: this project's container leftovers and a low disk, as
# hooks/leftovers.sh reports them (report only; silent with nothing to say).
#
# The report reaches the model as additionalContext and the person as one
# systemMessage line the engine shows itself (the header and the findings joined
# by " · "), so no hooks module has to relay it (a session.append hook on the
# hook-context door is passed over where cc-plugin-sec-default is seated, decided
# 2026-11-14). The plugin option leftoversToast=false keeps it for the model only,
# printed as plain text as before. Always exits 0.

here="${0%/*}"
out="$(sh "$here/leftovers.sh" 2>/dev/null)"
[ -n "$out" ] || exit 0

case "${CLAUDE_PLUGIN_OPTION_leftoversToast:-}${CLAUDE_PLUGIN_OPTION_LEFTOVERSTOAST:-}" in
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
# The header, then each finding, on one line.
line="$(printf '%s\n' "$out" | awk 'NF { sub(/^[ \t]+/, ""); sub(/[ \t]+$/, ""); printf "%s%s", (n++ == 0 ? "" : (n == 2 ? " " : " · ")), $0 }')"
printf '{"systemMessage":"%s","hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}\n' "$(esc "$line")" "$(esc "$out")"
exit 0
