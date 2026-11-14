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
  # a second word only when the table lists it as a subcommand of the first (never an entry or a branch)
  case "$_gw_w:$_gw_2" in
    @VERBS@
      printf '%s %s' "$_gw_w" "$_gw_2"
      return 0
      ;;
  esac
  printf '%s' "$_gw_w"
}
_gw_mine() { # _gw_mine <path>: it belongs to the current user (POSIX has no test -O)
  [ -n "$(find "$1" -prune -user "$(id -u)" -print 2>/dev/null)" ]
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
  # A subshell with umask 077: the directory 0700, the file and the cut copy 0600 (one we own is
  # tightened). Only a regular file is written or read, never a link, a FIFO or a device, and at
  # most the last @MAX_BYTES@ bytes are read. The cut runs under flock on <file>.lock, skipped when
  # flock is missing or another writer holds it (the file then grows until the next cut).
  (
    umask 077
    mkdir -p "$_gw_dir" || exit 0
    if _gw_mine "$_gw_dir"; then chmod 700 "$_gw_dir" || :; fi
    if [ -e "$_gw_f" ] || [ -L "$_gw_f" ]; then
      [ -f "$_gw_f" ] && [ ! -L "$_gw_f" ] || exit 0
    fi
    printf '{"at":%s000,"guard":"%s","decision":"%s","entry":"%s","tool":"%s","verb":%s,"outcome":%s}\n' \
      "$(date +%s)" "$_gw_id" "$1" "$_gw_match" "$_gw_tool" "$_gw_v" "$_gw_o" >>"$_gw_f" || exit 0
    if _gw_mine "$_gw_f"; then chmod 600 "$_gw_f" || :; fi
    _gw_size=$(($(wc -c <"$_gw_f")))
    [ "$_gw_size" -gt @MAX_BYTES@ ] || [ $(($(tail -c @MAX_BYTES@ "$_gw_f" | wc -l))) -gt @TRIM_AT@ ] || exit 0
    command -v flock >/dev/null 2>&1 && command -v mktemp >/dev/null 2>&1 || exit 0
    if [ -e "$_gw_f.lock" ] || [ -L "$_gw_f.lock" ]; then
      [ -f "$_gw_f.lock" ] && [ ! -L "$_gw_f.lock" ] || exit 0
    fi
    (
      flock -n 9 || exit 0
      _gw_t="$(mktemp "$_gw_dir/.$_gw_sid.jsonl.XXXXXXXX")" || exit 0
      # a cut from the middle of a large file starts on a partial line: drop it
      if tail -c @MAX_BYTES@ "$_gw_f" | if [ "$_gw_size" -gt @MAX_BYTES@ ]; then sed 1d; else cat; fi |
        tail -n @MAX_ROWS@ >"$_gw_t" && mv -f "$_gw_t" "$_gw_f"; then :; else rm -f "$_gw_t"; fi
    ) 9>>"$_gw_f.lock"
  ) 2>/dev/null
  return 0
}
guard_emit() { # guard_emit deny|ask "<marker> <reason>"
  _gw_msg='' _gw_ctx=''
  _gw_own=''
  if _gw_find "$2"; then
    _gw_own=1
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
  # the row after the decision is out: a log that fails loses the row, never the decision
  [ -z "$_gw_own" ] || _gw_log "$1" || :
}
# <<< guard words
