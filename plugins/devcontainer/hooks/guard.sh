#!/bin/sh
# PreToolUse guard for the Bash tool: container resources are removed by ID,
# own prefix or this project's label, never by a broad sweep
# (shared/containers.md §9, §9a). A prune or an "everything" removal deletes
# other projects' images, caches and in-flight builds on a shared host.
#
#   deny  any `prune` through podman, docker, nerdctl, buildah or
#         $CONTAINER_ENGINE (system, image, volume, container, builder,
#         network, even filtered)
#   deny  rm / rmi / volume rm with -a/--all (buildah rmi --prune too)
#   deny  a removal fed by a quiet listing (`ps -q`, `images -q`,
#         `volume ls -q`) when the command names no label=, name= or id=
#         filter
#   none  everything else: removal by ID, by label, listings, builds
#
# Pattern matching on the raw command: it catches the commands an agent
# writes, not every possible obfuscation.
#
# In a package install (a VERSION file two levels above the plugin) the
# launcher below hands the event to `ai-gent guard --guard containers` and turns a
# missing, failing or slow binary into an ask (spec 033); in a clone it falls
# through to the shell logic, which is also the oracle the binary's fixtures
# are checked against.

# >>> guard launcher (shared/guard-launcher.sh, spec 033: package installs call the binary; a clone runs the shell logic below)
r="${CLAUDE_PLUGIN_ROOT:-${0%/*}/..}"
if [ -f "$r/../../VERSION" ]; then
  b="$r/../../bin/ai-gent"; [ -x "$b" ] || b="$(command -v ai-gent 2>/dev/null)" || b=
  rc=0 out=
  if [ -z "$b" ]; then rc=127; else
    set -- "$b" guard --guard containers --format claude
    command -v timeout >/dev/null 2>&1 && set -- timeout 2 "$@"
    out="$("$@" 2>/dev/null)" || rc=$?
  fi
  if [ "$rc" != 0 ]; then
    case "$rc" in 127) w="it is missing" ;; 124) w="it took over 2 s" ;; *) w="it failed with exit $rc" ;; esac
    printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"containers guard: the ai-gent binary is unavailable (%s); run: curl -fsSL https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.sh | sh. Allow only if you accept this command unguarded."}}\n' "$w"
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
  if [ "$(wc -l <"$_gw_f")" -gt 400 ]; then
    tail -n 200 "$_gw_f" >"$_gw_f.$$" 2>/dev/null && mv "$_gw_f.$$" "$_gw_f" 2>/dev/null
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
    ( printf '%s' "$input" | "$b" guard --guard containers --shadow --shell-decision none >/dev/null 2>&1 & ) >/dev/null 2>&1
  else
    ( printf '%s' "$input" | "$b" guard --guard containers --shadow --shell-decision "$sd" --shell-reason "$sr" >/dev/null 2>&1 & ) >/dev/null 2>&1
  fi
  return 0
}
trap shadow EXIT

decide() { # decide deny <reason>
  sd="$1" sr="containers guard: $2"
  guard_emit "$1" "containers guard: $2"
  exit 0
}

cmd="$(printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p')"
[ -n "$cmd" ] || exit 0

# "$CONTAINER_ENGINE" stays a word; other quoted strings are blanked (so text
# inside quotes can't look like a command); split on ; & | ( ) ` and newlines.
segments="$(printf '%s\n' "$cmd" | awk '{
  gsub(/\\"\$\{?CONTAINER_ENGINE\}?\\"/, "$CONTAINER_ENGINE")
  gsub(/\\"([^\\]|\\[^"])*\\"/, "\"\""); gsub(/\047[^\047]*\047/, "\047\047")
  gsub(/\\n/, "\n"); gsub(/[;&|()`]/, "\n"); print }')"

prefix='^[[:space:]]*((sudo([[:space:]]+-[^[:space:]]+([[:space:]]+[^-[:space:]][^[:space:]]*)?)*|env|exec|nice|do|then|else|xargs([[:space:]]+-[^[:space:]]+)*|timeout[[:space:]]+[0-9smh]+|[A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*)[[:space:]]+)*'
engine='(([^[:space:]]*/)?(podman|docker|nerdctl)|\$\{?CONTAINER_ENGINE\}?)'
buildah='([^[:space:]]*/)?buildah'
words='([[:space:]]+[^[:space:]]+)*'

has() { printf '%s\n' "$segments" | grep -Eq "$1"; }

if has "$prefix($engine|$buildah)${words}[[:space:]]+prune([[:space:]]|\$)"; then
  decide deny "prune removes other projects' images, volumes and in-flight builds; remove by ID or by this project's label (containers.md §9a: images -q --filter dangling=true --filter label=me.lucasvmigotto.ai-gent.project=<project>, then rmi each ID)"
fi

removal="$prefix(${engine}[[:space:]]+((container|image|volume)[[:space:]]+)?(rm|rmi)|${buildah}[[:space:]]+(rm|rmi))"
remove="$removal([[:space:]]|\$)"
if has "$removal${words}[[:space:]]+(--all|-[a-zA-Z]*a[a-zA-Z]*)([[:space:]]|\$)" ||
  has "$prefix${buildah}[[:space:]]+rmi${words}[[:space:]]+(--prune|-p)([[:space:]]|\$)"; then
  decide deny "removing everything takes other projects' containers, images or volumes; remove by ID or by this project's label (containers.md §9)"
fi

listing="$prefix${engine}[[:space:]]+((container|image|volume)[[:space:]]+)?(ps|ls|list|images)${words}[[:space:]]+(--quiet|-[a-zA-Z]*q[a-zA-Z]*)([[:space:]]|\$)"
if has "$remove" && has "$listing" && ! printf '%s' "$cmd" | grep -Eq '(label|name|id)='; then
  decide deny "that removes whatever an unfiltered listing returns, other projects' included; filter the listing by label=me.lucasvmigotto.ai-gent.project=<project> (or your own name= prefix) (containers.md §9a)"
fi

exit 0
