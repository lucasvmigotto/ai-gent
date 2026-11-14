# >>> guard words (generated from shared/guard-words.json by scripts/consistency.py --sync-shared-code)
# guard_emit deny|ask "<marker> <reason>": print the decision with the guard's own words (a
# systemMessage for the person; on a deny, additionalContext for the model) and append it to the
# session's guard log, which the project plugin's /guards reads (specs 015 and 024). The reason is
# JSON-ready text. The plugin options guardUx and guardLog (false) turn each off. The binary
# (rust/crates/ai-gent-core/src/guard/words.rs) and opencode/guard/rules.mjs do the same.
_gw_off() { # _gw_off <option> <OPTION>: the plugin option is set to false
  _gw_a='' _gw_b=''
  eval "_gw_a=\${CLAUDE_PLUGIN_OPTION_$1:-} _gw_b=\${CLAUDE_PLUGIN_OPTION_$2:-}"
  [ "$_gw_a" = false ] || [ "$_gw_b" = false ]
}
_gw_find() { # _gw_find <reason>: the guard and the entry a reason belongs to
  _gw_id='' _gw_match=generic _gw_say='' _gw_do='' _gw_if=''
  case "$1" in
    @GUARDS@
    *) return 1 ;;
  esac
  _gw_text="${1#"$_gw_marker"}"
  _gw_text="${_gw_text# }"
  case "$_gw_id:$_gw_text" in
    @ENTRIES@
  esac
  return 0
}
_gw_verb() { # _gw_verb <command, JSON-escaped>: its verb, or nothing (never a value or a path)
  _gw_c="$(printf '%s' "$1" | sed -e 's/\\[ntr]/ /g')"
  set -f
  # shellcheck disable=SC2086 # split into words on purpose
  set -- $_gw_c
  set +f
  _gw_w="${1:-}" _gw_2="${2:-}"
  case "$_gw_w" in *://*) return 0 ;; esac
  _gw_w="${_gw_w##*/}"
  printf '%s\n' "$_gw_w" | grep -Eq '^[a-z][a-z0-9._-]{0,23}$' || return 0
  _gw_n="$(printf '%s' "$_gw_w" | tr -cd '0-9')"
  [ "${#_gw_n}" -lt 7 ] || return 0
  case "$_gw_w" in
    git | gh | pass | podman | docker)
      if printf '%s\n' "$_gw_2" | grep -Eq '^[a-z][a-z-]{0,23}$'; then
        printf '%s %s' "$_gw_w" "$_gw_2"
        return 0
      fi
      ;;
  esac
  printf '%s' "$_gw_w"
}
_gw_log() { # _gw_log deny|ask: one row (entry and verb only) in <log dir>/<session>.jsonl
  _gw_off guardLog GUARDLOG && return 0
  _gw_sid="$(printf '%s' "$input" | sed -n 's/.*"session_id"[[:space:]]*:[[:space:]]*"\([A-Za-z0-9_-]\{1,128\}\)".*/\1/p')"
  [ -n "$_gw_sid" ] || return 0
  if [ -n "${AI_GENT_GUARD_LOG_DIR:-}" ]; then _gw_dir="$AI_GENT_GUARD_LOG_DIR"
  elif [ -n "${XDG_STATE_HOME:-}" ]; then _gw_dir="$XDG_STATE_HOME/ai-gent/guard-log"
  elif [ -n "${HOME:-}" ]; then _gw_dir="$HOME/.local/state/ai-gent/guard-log"
  elif [ -n "${LOCALAPPDATA:-}" ]; then _gw_dir="$LOCALAPPDATA/ai-gent/state/guard-log"
  else return 0; fi
  _gw_tool="$(printf '%s' "$input" | sed -n 's/.*"tool_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"
  printf '%s\n' "$_gw_tool" | grep -Eq '^[A-Za-z][A-Za-z0-9_.:-]{0,63}$' || _gw_tool=unknown
  _gw_v=''
  if [ "$_gw_tool" = Bash ]; then
    _gw_v="$(_gw_verb "$(printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p')")"
  fi
  if [ -n "$_gw_v" ]; then _gw_v="\"$_gw_v\""; else _gw_v=null; fi
  if [ "$1" = ask ]; then _gw_o='"asked"'; else _gw_o=null; fi
  _gw_f="$_gw_dir/$_gw_sid.jsonl"
  (umask 077 && mkdir -p "$_gw_dir" &&
    printf '{"at":%s000,"guard":"%s","decision":"%s","entry":"%s","tool":"%s","verb":%s,"outcome":%s}\n' \
      "$(date +%s)" "$_gw_id" "$1" "$_gw_match" "$_gw_tool" "$_gw_v" "$_gw_o" >>"$_gw_f") 2>/dev/null || return 0
  if [ "$(wc -l <"$_gw_f")" -gt @TRIM_AT@ ]; then
    tail -n @MAX_ROWS@ "$_gw_f" >"$_gw_f.$$" 2>/dev/null && mv "$_gw_f.$$" "$_gw_f" 2>/dev/null
  fi
  return 0
}
guard_emit() { # guard_emit deny|ask "<marker> <reason>"
  _gw_msg='' _gw_ctx=''
  if _gw_find "$2"; then
    _gw_log "$1" || :
    if ! _gw_off guardUx GUARDUX; then
      _gw_name="${_gw_marker%:}"
      if [ "$1" = deny ]; then
        if [ -n "$_gw_say" ]; then
          _gw_msg="$_gw_say"
          [ -z "$_gw_do" ] || _gw_msg="$_gw_msg Instead: $_gw_do"
        else
          _gw_msg="$_gw_text"
        fi
        _gw_msg="$_gw_name refused this. $_gw_msg ($_gw_skill)"
        [ -z "$_gw_do" ] || _gw_ctx="$_gw_name: instead, $_gw_do"
      elif [ -n "$_gw_say" ]; then
        _gw_msg="$_gw_name asks: $_gw_say"
        [ -z "$_gw_if" ] || _gw_msg="$_gw_msg $_gw_if"
        _gw_msg="$_gw_msg ($_gw_skill)"
      fi
    fi
  fi
  _gw_x='' _gw_y=''
  [ -z "$_gw_ctx" ] || _gw_x=',"additionalContext":"'"$_gw_ctx"'"'
  [ -z "$_gw_msg" ] || _gw_y=',"systemMessage":"'"$_gw_msg"'"'
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"%s","permissionDecisionReason":"%s"%s}%s}\n' "$1" "$2" "$_gw_x" "$_gw_y"
}
# <<< guard words
