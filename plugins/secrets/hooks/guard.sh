#!/bin/sh
# PreToolUse guard for the Bash tool: secret values in pass never reach the
# agent (shared/secrets.md). ai-gent's runners (dbrun, ...) read an entry
# for the command that needs it; the agent only sees names.
#
#   deny  a pass command that prints or copies a value: show, a bare
#         `pass <entry>`, -c/--clip, -q/--qrcode, edit, grep, generate, otp
#   deny  pass insert/add: the agent never writes a credential
#   ask   pass rm/mv/cp/init/git: changes the store, the user decides
#   deny  gpg on the password store (decrypting it directly)
#   deny  pass-run with an ai-gent/db/ entry (only dbrun reads those), or
#         handing a secret to a command that can print its environment
#   none  pass ls/find, help, version
#   none  `pass show <entry>` (no flags, one entry) when the user's own
#         allowlist names the entry (allowed_entry below); never under
#         ai-gent/db/, never with PASSWORD_STORE_*
#   ask   any command that mentions the allowlist file (pass-allow): the agent
#         doesn't widen its own reach
#
# Pattern matching on the raw command: it catches the commands an agent
# writes, not every possible obfuscation.
#
# In a package install (a VERSION file two levels above the plugin) the
# launcher below hands the event to `ai-gent guard --guard secrets` and turns a
# missing, failing or slow binary into an ask (spec 033); in a clone it falls
# through to the shell logic, which is also the oracle the binary's fixtures
# are checked against.

# >>> guard launcher (shared/guard-launcher.sh, spec 033: package installs call the binary; a clone runs the shell logic below)
r="${CLAUDE_PLUGIN_ROOT:-${0%/*}/..}"
if [ -f "$r/../../VERSION" ]; then
  b="$r/../../bin/ai-gent"; [ -x "$b" ] || b="$(command -v ai-gent 2>/dev/null)" || b=
  rc=0 out=
  if [ -z "$b" ]; then rc=127; else
    set -- "$b" guard --guard secrets --format claude
    command -v timeout >/dev/null 2>&1 && set -- timeout 2 "$@"
    out="$("$@" 2>/dev/null)" || rc=$?
  fi
  if [ "$rc" != 0 ]; then
    case "$rc" in 127) w="it is missing" ;; 124) w="it took over 2 s" ;; *) w="it failed with exit $rc" ;; esac
    printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"secrets guard: the ai-gent binary is unavailable (%s); run: curl -fsSL https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.sh | sh. Allow only if you accept this command unguarded."}}\n' "$w"
  elif [ -n "$out" ]; then printf '%s\n' "$out"; fi
  exit 0
fi
# <<< guard launcher

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
    'git:workflow guard:'*) _gw_id=git _gw_marker='git:workflow guard:' _gw_skill='git:workflow' ;;
    'db guard:'*) _gw_id=db _gw_marker='db guard:' _gw_skill='db:connect' ;;
    'secrets guard:'*) _gw_id=secrets _gw_marker='secrets guard:' _gw_skill='secrets:pass' ;;
    'repo guard:'*) _gw_id=repo _gw_marker='repo guard:' _gw_skill='repo:pr' ;;
    'containers guard:'*) _gw_id=containers _gw_marker='containers guard:' _gw_skill='devcontainer:workflow' ;;
    *) return 1 ;;
  esac
  _gw_text="${1#"$_gw_marker"}"
  _gw_text="${_gw_text# }"
  case "$_gw_id:$_gw_text" in
    git:*'hooks must not be bypassed'*) _gw_match='hooks must not be bypassed' _gw_say='Hooks are never bypassed (--no-verify, -n).' _gw_do='fix what the hook reports, then commit again.' ;;
    git:*'commits, issues, pull requests and comments carry no sign-off'*) _gw_match='commits, issues, pull requests and comments carry no sign-off' _gw_say='Commits, issues, pull requests and comments carry no mention of the agent.' _gw_do='remove the Co-Authored-By or \"generated with\" line and run the command again.' ;;
    git:*'force pushes use --force-with-lease'*) _gw_match='force pushes use --force-with-lease' _gw_say='A plain force push can overwrite other people'\''s commits.' _gw_do='use --force-with-lease, and only on the user'\''s explicit order.' ;;
    git:*'the autonomy grant'*) _gw_match='the autonomy grant' _gw_say='Only you write the autonomy grant; an agent that could write it could give itself the permissions.' _gw_do='create or change the grant file yourself (expires, branches and actions lines).' ;;
    git:*'git push publishes'*) _gw_match='git push publishes' _gw_say='This publishes commits to the remote.' _gw_if='Allow only if you asked for this push.' ;;
    git:*'opening a PR publishes the branch'*) _gw_match='opening a PR publishes the branch' _gw_say='This opens a pull request, which publishes the branch.' _gw_if='Allow only if you asked for it.' ;;
    git:*'git branch -D deletes unmerged work'*) _gw_match='git branch -D deletes unmerged work' _gw_say='This deletes a branch that may hold unmerged work.' _gw_if='Allow only if you asked for it; the workflow uses git branch -d.' ;;
    git:*'rewrites history or discards work'*) _gw_match='rewrites history or discards work' _gw_say='This rewrites history or discards work.' _gw_if='Allow only if you asked for it.' ;;
    git:*'the workflow works on a branch'*) _gw_match='the workflow works on a branch' _gw_say='This commits or merges on a long-lived branch.' _gw_if='Allow only if you asked for it.' ;;
    git:*'promotion up the branch chain'*) _gw_match='promotion up the branch chain' _gw_say='This commits or merges on a branch higher in the chain.' _gw_if='Allow only if you asked to promote.' ;;
    git:*'commits directly on'*) _gw_match='commits directly on' _gw_say='This commits straight onto the integration branch.' _gw_if='Work goes on a branch merged into it; allow only if you meant this (concluding a merge is fine).' ;;
    git:*'cannot follow the cd'*) _gw_match='cannot follow the cd' _gw_say='This commits or merges after a cd the guard cannot follow, so the branch is unknown.' _gw_if='Allow only if the target is not an integration branch; git -C with a plain path is understood.' ;;
    git:*'a git alias can run any command'*) _gw_match='a git alias can run any command' _gw_say='A git alias can run any command, so the branch a commit lands on is unknown.' _gw_if='Allow only if you asked for this alias.' ;;
    db:*'db credentials file'*) _gw_match='db credentials file' _gw_say='Only dbrun reads database credential files.' _gw_do='use dbrun profiles, classify or test.' ;;
    db:*'restores into a database'*) _gw_match='restores into a database' _gw_say='A direct restore changes a database.' _gw_do='local restores go through dbrun restore; anything else is a script for a person to run.' ;;
    db:*'with SQL that writes'*) _gw_match='with SQL that writes' _gw_say='A direct client call would write to a database.' _gw_do='use dbrun (plan and apply on a proven local database, a script for anything else).' ;;
    db:*'bypasses dbrun'*) _gw_match='bypasses dbrun' _gw_say='This calls a database client directly: no classification, masking or command log.' _gw_if='Allow only if you want this direct call.' ;;
    secrets:*'decrypting the password store directly'*) _gw_match='decrypting the password store directly' _gw_say='Decrypting the store would show secret values.' _gw_do='the runner that needs a secret reads it.' ;;
    secrets:*'ai-gent/db/ entries are read only by dbrun'*) _gw_match='ai-gent/db/ entries are read only by dbrun' _gw_say='Database passwords are read only by dbrun.' _gw_do='use dbrun test or query.' ;;
    secrets:*'hands the secret to a command that can print it'*) _gw_match='hands the secret to a command that can print it' _gw_say='This would hand a secret to a command that can print it.' _gw_do='run the tool that needs it after --.' ;;
    secrets:*'pass -c/-q exposes a secret value'*) _gw_match='pass -c/-q exposes a secret value' _gw_say='This would expose a secret value.' _gw_do='the runner that needs it reads it; the agent never does.' ;;
    secrets:*'the agent never writes a credential'*) _gw_match='the agent never writes a credential' _gw_say='The agent never types a credential.' _gw_do='ask the user to run `pass insert <entry>` in their own terminal.' ;;
    secrets:*'changes the password store'*) _gw_match='changes the password store' _gw_say='This changes the password store.' _gw_if='The user decides.' ;;
    secrets:*'mentions the pass allowlist'*) _gw_match='mentions the pass allowlist' _gw_say='This touches the list of pass entries the agent may read.' _gw_if='The user decides what the agent may read.' ;;
    secrets:*'that shows a secret value from pass'*) _gw_match='that shows a secret value from pass' _gw_say='This would show a secret value.' _gw_do='the agent sees names only (pass ls).' ;;
    repo:*'work items, pull requests and comments carry no sign-off'*) _gw_match='work items, pull requests and comments carry no sign-off' _gw_say='Issues, work items, pull requests and comments carry no mention of the agent.' _gw_do='remove the Co-Authored-By or \"generated with\" line and run the command again.' ;;
    repo:*'reporun --write changes an issue'*) _gw_match='reporun --write changes an issue' _gw_say='This changes an issue, work item or pull request on the platform.' _gw_if='Allow only if you asked for exactly this change.' ;;
    containers:*'prune removes other projects'*) _gw_match='prune removes other projects' _gw_say='A prune removes other projects'\'' images, volumes and builds.' _gw_do='remove by ID, or by this project'\''s label (containers.md §9a).' ;;
    containers:*'removing everything takes other projects'*) _gw_match='removing everything takes other projects' _gw_say='Removing everything takes other projects'\'' resources.' _gw_do='remove by ID or by this project'\''s label (containers.md §9).' ;;
    containers:*'removes whatever an unfiltered listing returns'*) _gw_match='removes whatever an unfiltered listing returns' _gw_say='That removes whatever an unfiltered listing returns.' _gw_do='filter the listing by this project'\''s label (containers.md §9a).' ;;
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
    git:add | git:am | git:apply | git:bisect | git:blame | git:branch | git:checkout | git:cherry-pick | git:clean | git:clone | git:commit | git:config | git:diff | git:fetch | git:filter-branch | git:filter-repo | git:gc | git:init | git:log | git:merge | git:mv | git:notes | git:pull | git:push | git:rebase | git:reflog | git:remote | git:replace | git:reset | git:restore | git:revert | git:rm | git:show | git:stash | git:status | git:submodule | git:switch | git:tag | git:update-ref | git:worktree)
      printf '%s %s' "$_gw_w" "$_gw_2"
      return 0
      ;;
    gh:api | gh:auth | gh:gist | gh:issue | gh:pr | gh:release | gh:repo | gh:run | gh:secret | gh:ssh-key | gh:variable | gh:workflow)
      printf '%s %s' "$_gw_w" "$_gw_2"
      return 0
      ;;
    pass:cp | pass:edit | pass:find | pass:generate | pass:git | pass:grep | pass:init | pass:insert | pass:ls | pass:mv | pass:otp | pass:rm | pass:show)
      printf '%s %s' "$_gw_w" "$_gw_2"
      return 0
      ;;
    podman:build | podman:builder | podman:buildx | podman:compose | podman:container | podman:cp | podman:create | podman:exec | podman:image | podman:images | podman:inspect | podman:kill | podman:load | podman:login | podman:logout | podman:logs | podman:network | podman:pause | podman:pod | podman:ps | podman:pull | podman:push | podman:rm | podman:rmi | podman:run | podman:save | podman:start | podman:stop | podman:system | podman:tag | podman:volume)
      printf '%s %s' "$_gw_w" "$_gw_2"
      return 0
      ;;
    docker:build | docker:builder | docker:buildx | docker:compose | docker:container | docker:cp | docker:create | docker:exec | docker:image | docker:images | docker:inspect | docker:kill | docker:load | docker:login | docker:logout | docker:logs | docker:network | docker:pause | docker:pod | docker:ps | docker:pull | docker:push | docker:rm | docker:rmi | docker:run | docker:save | docker:start | docker:stop | docker:system | docker:tag | docker:volume)
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
  # most the last 262144 bytes are read. The cut runs under flock on <file>.lock, skipped when
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
    [ "$_gw_size" -gt 262144 ] || [ $(($(tail -c 262144 "$_gw_f" | wc -l))) -gt 400 ] || exit 0
    command -v flock >/dev/null 2>&1 && command -v mktemp >/dev/null 2>&1 || exit 0
    if [ -e "$_gw_f.lock" ] || [ -L "$_gw_f.lock" ]; then
      [ -f "$_gw_f.lock" ] && [ ! -L "$_gw_f.lock" ] || exit 0
    fi
    (
      flock -n 9 || exit 0
      _gw_t="$(mktemp "$_gw_dir/.$_gw_sid.jsonl.XXXXXXXX")" || exit 0
      # a cut from the middle of a large file starts on a partial line: drop it. The copy replaces
      # the file only when it is not empty and ends on the file's last row: a read that failed
      # upstream (the pipe still exits 0) or a row appended meanwhile leaves the file whole.
      if tail -c 262144 "$_gw_f" | if [ "$_gw_size" -gt 262144 ]; then sed 1d; else cat; fi |
        tail -n 200 >"$_gw_t" && [ -s "$_gw_t" ] &&
        _gw_l="$(tail -n 1 "$_gw_f")" && [ -n "$_gw_l" ] && [ "$(tail -n 1 "$_gw_t")" = "$_gw_l" ] &&
        mv -f "$_gw_t" "$_gw_f"; then :; else rm -f "$_gw_t"; fi
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

set -u

input="$(tr '\n' ' ')"

# Shadow mode (spec 033 FR-009): once the decision is printed, the installed
# ai-gent binary, when there is one, gets the same event detached and only
# records whether it agrees. It never changes the decision, never writes to
# stdout and is silent without the binary; AI_GENT_NO_SHADOW=1 turns it off.
sd=none sr=
# shellcheck disable=SC2329 # run by the EXIT trap
shadow() {
  [ "${AI_GENT_NO_SHADOW:-0}" = 0 ] || return 0
  b="${CLAUDE_PLUGIN_ROOT:-${0%/*}/..}/../../bin/ai-gent"
  [ -x "$b" ] || b="$(command -v ai-gent 2>/dev/null)" || return 0
  [ -x "$b" ] || return 0
  if [ "$sd" = none ]; then
    ( printf '%s' "$input" | "$b" guard --guard secrets --shadow --shell-decision none >/dev/null 2>&1 & ) >/dev/null 2>&1
  else
    ( printf '%s' "$input" | "$b" guard --guard secrets --shadow --shell-decision "$sd" --shell-reason "$sr" >/dev/null 2>&1 & ) >/dev/null 2>&1
  fi
  return 0
}
trap shadow EXIT

decide() { # decide deny|ask <reason>
  sd="$1" sr="secrets guard: $2"
  guard_emit "$1" "secrets guard: $2"
  exit 0
}

cmd="$(printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p')"
[ -n "$cmd" ] || exit 0

# One simple command per line, quoted strings blanked (so text inside quotes
# can't look like a command), split on ; & | ( ) and newlines.
segments="$(printf '%s\n' "$cmd" | awk '{
  gsub(/\\"([^\\]|\\[^"])*\\"/, "\"\""); gsub(/\047[^\047]*\047/, "\047\047")
  gsub(/\\n/, "\n"); gsub(/[;&|()]/, "\n"); print }')"

prefix='^[[:space:]]*((sudo([[:space:]]+-[^[:space:]]+([[:space:]]+[^-[:space:]][^[:space:]]*)?)*|env|exec|nice|timeout[[:space:]]+[0-9smh]+|[A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*)[[:space:]]+)*'

# gpg anywhere in a command that names the password store.
if printf '%s\n' "$segments" | grep -Eq "$prefix([^[:space:]]*/)?gpg2?([[:space:]]|\$)" &&
  printf '%s' "$cmd" | grep -Eq '\.password-store|PASSWORD_STORE_DIR'; then
  decide deny "decrypting the password store directly shows secret values; the runner that needs a secret reads it"
fi

# pass-run (dottod) and similar runners: a secret for one command.
runner="$(printf '%s\n' "$segments" | grep -E "$prefix([^[:space:]]*/)?pass-run([[:space:]]|\$)" | head -n1)"
if [ -n "$runner" ]; then
  if printf '%s' "$cmd" | grep -Eq '(^|[=:[:space:]])([A-Za-z0-9_.@+-]+/)?ai-gent/db/'; then
    decide deny "ai-gent/db/ entries are read only by dbrun; use dbrun test / query"
  fi
  after="$(printf '%s\n' "$runner" | awk '{ for (i = 1; i < NF; i++) if ($i == "--") { print $(i + 1); exit } }')"
  case "${after##*/}" in
    env | printenv | set | export | declare | typeset | echo | printf | cat | sh | bash | dash | zsh | ksh | fish | python* | node | perl | ruby | eval)
      decide deny "that hands the secret to a command that can print it; run the tool that needs it after --"
      ;;
  esac
fi

# The user's personal allowlist of pass entries the agent may `pass show`:
# $AI_GENT_PASS_ALLOW, else ${XDG_CONFIG_HOME:-~/.config}/ai-gent/pass-allow.
# One rule per line: `entry` (exact) or `prefix/` (everything under it); blank
# lines and # comments ignored; rules that are empty, start with / - or ~, or
# have a . or .. path part never allow. Missing or unreadable file: no rule.
allow_file="${AI_GENT_PASS_ALLOW:-${XDG_CONFIG_HOME:-${HOME:-}/.config}/ai-gent/pass-allow}"

# shellcheck disable=SC2329 # called below
allowed_entry() { # allowed_entry <entry>: 0 when a rule names it
  [ -f "$allow_file" ] && [ -r "$allow_file" ] || return 1
  awk -v e="$1" '
    { sub(/\r$/, ""); sub(/^[ \t]+/, ""); sub(/[ \t]+$/, "") }
    $0 == "" || substr($0, 1, 1) == "#" { next }
    index("/-~", substr($0, 1, 1)) > 0 { next }
    {
      n = split($0, p, "/"); bad = 0
      for (i = 1; i <= n; i++) if (p[i] == ".." || p[i] == ".") bad = 1
      if (bad) next
      if (substr($0, length($0)) == "/") {
        if (index(e, $0) == 1 && length(e) > length($0)) { found = 1; exit }
      } else if ($0 == e) { found = 1; exit }
    }
    END { exit !found }
  ' "$allow_file" 2>/dev/null
}

# Whatever names the allowlist file is the user's to approve: the agent must
# not widen its own reach through Bash. (Write/Edit are outside this hook.)
if printf '%s' "$cmd" | grep -Fq -e pass-allow -e AI_GENT_PASS_ALLOW ||
  { [ -n "${AI_GENT_PASS_ALLOW:-}" ] && printf '%s' "$cmd" | grep -Fq -e "$AI_GENT_PASS_ALLOW"; }; then
  decide ask "that command mentions the pass allowlist (pass-allow); the user decides what the agent may read"
fi

hit="$(printf '%s\n' "$segments" | grep -E "$prefix([^[:space:]]*/)?pass([[:space:]]|\$)" | head -n1)"
[ -n "$hit" ] || exit 0
npass="$(printf '%s\n' "$segments" | grep -Ec "$prefix([^[:space:]]*/)?pass([[:space:]]|\$)")"

rest="$(printf '%s\n' "$hit" | sed -E "s#$prefix([^[:space:]]*/)?pass([[:space:]]|\$)##")"
set -f
# shellcheck disable=SC2086 # split the arguments on purpose
set -- $rest
set +f
sub="" entry="" nflags=0 nargs=0
for arg in "$@"; do
  case "$arg" in
    -c | --clip | -c[0-9]* | --clip=* | -q | --qrcode | -q[0-9]* | --qrcode=*)
      decide deny "pass -c/-q exposes a secret value; the agent never reads one — the runner that needs it does"
      ;;
    -*) nflags=$((nflags + 1)) ;;
    *)
      nargs=$((nargs + 1))
      if [ -z "$sub" ]; then sub="$arg"; elif [ -z "$entry" ]; then entry="$arg"; fi
      ;;
  esac
done

# Could the allowlist open this command? Only `pass show <entry>`: no flags,
# one clean entry name, the only pass command in the line, no store or config
# override in the text, and never an ai-gent/db/ entry (dbrun reads those).
show_eligible() {
  [ "$sub" = show ] && [ "$nflags" -eq 0 ] && [ "$nargs" -eq 2 ] && [ "$npass" -eq 1 ] || return 1
  case "$entry" in
    "" | /* | -* | */ | *//* | *[!A-Za-z0-9._@+/-]*) return 1 ;;
    . | .. | ./* | ../* | */. | */.. | */./* | */../*) return 1 ;;
  esac
  lower="$(printf '%s' "$entry" | tr '[:upper:]' '[:lower:]')"
  case "$lower" in ai-gent/db | ai-gent/db/*) return 1 ;; esac
  ! printf '%s' "$cmd" | grep -Eq 'PASSWORD_STORE_|XDG_CONFIG_HOME|(^|[^A-Za-z0-9_])HOME='
}

case "$sub" in
  "" | ls | list | find | search | help | --help | version | --version) exit 0 ;;
  insert | add)
    decide deny "the agent never writes a credential; ask the user to run \`pass insert <entry>\` in their own terminal"
    ;;
  rm | remove | delete | mv | rename | cp | copy | init | git)
    decide ask "pass $sub changes the password store; the user decides"
    ;;
  *)
    if show_eligible; then
      allowed_entry "$entry" && exit 0
      decide deny "that shows a secret value from pass; the agent sees names only (pass ls) — the runner that needs a secret reads it; list the entry in ~/.config/ai-gent/pass-allow to let the agent read it"
    fi
    decide deny "that shows a secret value from pass; the agent sees names only (pass ls) — the runner that needs a secret reads it"
    ;;
esac
