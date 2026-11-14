// ai-gent guard rules — the OpenCode side of the git, db, secrets, repo and containers guards.
//
// These are a faithful TypeScript/JavaScript port of plugins/git/hooks/guard.sh,
// plugins/db/hooks/guard.sh, plugins/secrets/hooks/guard.sh,
// plugins/repo/hooks/guard.sh and plugins/devcontainer/hooks/guard.sh, which stay the Claude Code enforcement. The
// two are kept from drifting by opencode/guard/test/rules.test.mjs, which runs
// both against the same vectors and asserts identical decisions.
//
// The input here is the *raw* command string OpenCode gives the permission
// hook (not JSON-escaped, as the shell guards receive from Claude Code). Each
// function returns { decision: "deny" | "ask" | "none", reason?: string }.

import { execFileSync } from "node:child_process";
import { chmodSync, closeSync, constants as FS, fstatSync, lstatSync, mkdirSync, openSync, readFileSync, readSync, renameSync, statSync, unlinkSync, writeSync } from "node:fs";
import { randomBytes } from "node:crypto";
import { join } from "node:path";

// The autonomy grant: .ai-gent/autonomy at the repository root, key: value lines the user writes
// (expires: 2026-11-06T18:00:00Z, branches: develop dev, actions: push pr merge issues). It counts
// only while expires lies ahead; anything unreadable or malformed is no grant. The git and repo
// guards stop asking for the forms it covers and deny the agent writing the file.
export const GRANT_FILE = ".ai-gent/autonomy";
const GRANT_REASON =
  "the autonomy grant (.ai-gent/autonomy) is written by the user, never by the agent; ask the user to create or change it";

export function parseGrant(text, nowIso) {
  const value = (key) => {
    const line = text.split("\n").find((l) => l.startsWith(`${key}:`));
    return line === undefined ? "" : line.slice(key.length + 1).replace(/^[ \t]+/, "").replace(/\r$/, "");
  };
  const expires = value("expires");
  if (!/^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$/.test(expires)) return null;
  if (nowIso >= expires) return null;
  const words = (v) => v.split(/[ \t]+/).filter(Boolean);
  const branches = words(value("branches"));
  return { actions: words(value("actions")), branches: branches.length ? branches : ["develop", "dev"] };
}

// The active grant of the repository that contains dir, as of now (null when there is none).
export function grantAt(dir, now = new Date()) {
  const git = (...args) =>
    execFileSync("git", ["-C", dir || ".", ...args], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).replace(/\n$/, "");
  try {
    let text;
    try {
      text = readFileSync(join(git("rev-parse", "--show-toplevel"), GRANT_FILE), "utf8");
    } catch {
      // A linked worktree has no copy of the git-excluded file: use the main checkout's
      const common = git("rev-parse", "--path-format=absolute", "--git-common-dir");
      if (!common.endsWith("/.git")) return null;
      text = readFileSync(join(common.slice(0, -5), GRANT_FILE), "utf8");
    }
    return parseGrant(text, now.toISOString().slice(0, 19) + "Z");
  } catch {
    return null;
  }
}

const GIT_RE_SRC =
  "^[ \\t]*(sudo[ \\t]+)?git(?:[ \\t]+(?:-[Cc][ \\t]+[^ \\t]+|--[a-z-]+(?:=[^ \\t]+)?))*[ \\t]+";

// Split a raw command the way the shell guards do: quoted strings blanked so
// their text cannot look like options, then split on ; & | ( ) and newlines.
function gitSegments(cmd) {
  return cmd
    .replace(/"(?:[^"\\]|\\.)*"/g, '""')
    .replace(/'[^']*'/g, "''")
    .replace(/\\n/g, "\n")
    .replace(/[;&|()]/g, "\n")
    .split("\n");
}

function gitHas(segments, sub, arg) {
  const re = new RegExp(GIT_RE_SRC + sub + "(?:[ \\t].*)?$");
  const matched = segments.filter((s) => re.test(s));
  if (!matched.length) return false;
  if (arg === undefined) return true;
  const argRe = new RegExp(arg);
  return matched.some((s) => argRe.test(s));
}

// `ai-gent autonomy` in any form but show/status/--help/-h, and `autonomy [flags] grant|revoke`
// even without the word ai-gent.
function autonomyDenied(text) {
  const t = text.replace(/\n/g, " ");
  const w = "(^|[^A-Za-z0-9_-])";
  if (new RegExp(w + "autonomy(\\s+-\\S*)*\\s+(grant|revoke)([^A-Za-z0-9_-]|$)").test(t)) return true;
  if (!new RegExp(w + "ai-gent(\\.exe)?([^A-Za-z0-9_/.-]|$)").test(t)) return false;
  const rest = t.replace(
    new RegExp(w + "autonomy\\s+(show|status|--help|-h)([^A-Za-z0-9_-]|$)", "g"),
    " ",
  );
  return new RegExp(w + "autonomy([^A-Za-z0-9_-]|$)").test(rest);
}

// branchOf(dir) returns the current branch name ("" when unknown). The plugin
// passes a git-backed resolver; tests do too.
export function gitGuard(cmd, { cwd = ".", branchOf = () => "", grantOf = grantAt } = {}) {
  const segments = gitSegments(cmd);
  const deny = (reason) => ({ decision: "deny", reason: `git:workflow guard: ${reason}` });
  const ask = (reason) => ({ decision: "ask", reason: `git:workflow guard: ${reason}` });

  if (
    gitHas(segments, "(commit|merge|push|am|rebase|cherry-pick|revert)", "(^|[ \\t])--no-verify([ \\t]|$)") ||
    gitHas(segments, "commit", "(^|[ \\t])-n([ \\t]|$)")
  ) {
    return deny("hooks must not be bypassed (--no-verify / -n); fix what the hook reports and try again");
  }

  // A real trailer or signature, not just prose that mentions one.
  const attributed = () =>
    /co-authored-by[ \t]*:|generated[ \t]+(with|by)[ \t]{0,3}\[?claude/i.test(cmd);
  if (
    (gitHas(segments, "(commit|merge|revert|cherry-pick|tag|notes)") ||
      segments.some((s) => /^[ \t]*gh[ \t]+(issue|pr)([ \t]|$)/.test(s))) &&
    attributed()
  ) {
    return deny(
      "this user's commits, issues, pull requests and comments carry no sign-off or mention of the agent that wrote them; remove it and run the command again",
    );
  }

  if (
    gitHas(segments, "push", "(^|[ \\t])(--force|-[a-zA-Z]*f[a-zA-Z]*)([ \\t]|$)") &&
    !gitHas(segments, "push", "--force-with-lease")
  ) {
    return deny("force pushes use --force-with-lease, and only on the user's explicit order");
  }

  // Nothing but the user writes the grant: a command that names it and is not a read.
  if (
    cmd.includes(GRANT_FILE) &&
    (cmd.includes(">") ||
      segments.some((s) => s.includes(GRANT_FILE) && !/^[ \t]*(cat|ls|head|tail|grep|stat|wc|test|\[)[ \t]/.test(s)))
  ) {
    return deny(GRANT_REASON);
  }

  // Nor does the agent run the grant CLI (ai-gent autonomy ...), in any spelling. The real
  // defence is structural (grant and revoke need a real terminal); this is defence in depth.
  // Tested on the raw command and on a copy with quotes, backslashes and $( ) stripped.
  const plain = cmd.replace(/['"`\\]/g, "").replace(/\$\(/g, "").replace(/\)/g, "");
  if (autonomyDenied(cmd) || autonomyDenied(plain)) {
    return deny(
      "the autonomy grant is created and removed by the user, never by the agent (ai-gent autonomy grant / revoke); ask the user to run it in their terminal, or with ! in the prompt",
    );
  }

  const cDir = new RegExp("git[ \\t]+-C[ \\t]+([^ \\t]*)").exec(segments.join("\n"));
  const workDir = cDir ? cDir[1] : cwd;

  // A push the active grant lifts: it names no protected branch and no bulk or forced form, and
  // runs from a branch that is not a protected one either.
  const pushLifted = () => {
    if (!grantOf(workDir)?.actions.includes("push")) return false;
    if (["", "main", "master", "homolog", "staging"].includes(branchOf(workDir))) return false;
    const sel = new RegExp(GIT_RE_SRC + "push(?:[ \\t].*)?$");
    const bad =
      /(^|[ \t])(--all|--mirror|--tags|--delete|--prune|-d)([ \t]|$)|(^|[ \t:/])(main|master|homolog|staging)([ \t]|$)|(^|[ \t])\+/;
    return !segments.filter((s) => sel.test(s)).some((s) => bad.test(s));
  };
  // A gh pr create the active grant lifts: it names a --base the grant covers.
  const prCreateLifted = () => {
    const g = grantOf(workDir);
    if (!g?.actions.includes("pr")) return false;
    const base = segments
      .filter((s) => /^[ \t]*gh[ \t]+pr[ \t]+create/.test(s))
      .map((s) => /^.*[ \t](--base|-B)[ \t=]+([^ \t]+)/.exec(s))
      .find(Boolean);
    return base !== undefined && g.branches.includes(base[2]);
  };

  if (gitHas(segments, "push") && !pushLifted()) {
    return ask("git push publishes; allow only if the user explicitly asked for this push");
  }
  if (segments.some((s) => /^[ \t]*gh[ \t]+pr[ \t]+create/.test(s)) && !prCreateLifted()) {
    return ask("opening a PR publishes the branch; allow only on the user's explicit request");
  }
  if (gitHas(segments, "branch", "(^|[ \\t])(-D|--delete[ \\t]+--force|--force[ \\t]+--delete)([ \\t]|$)")) {
    return ask("git branch -D deletes unmerged work; the workflow uses git branch -d");
  }
  if (
    gitHas(segments, "reset", "--hard") ||
    gitHas(segments, "clean", "(^|[ \\t])-[a-zA-Z]*f") ||
    gitHas(segments, "commit", "--amend") ||
    gitHas(segments, "(filter-branch|filter-repo)")
  ) {
    return ask("this rewrites history or discards work; allow only on the user's explicit request");
  }
  if (gitHas(segments, "commit") || gitHas(segments, "merge")) {
    const cMatch = new RegExp("git[ \\t]+-C[ \\t]+([^ \\t]*)").exec(segments.join("\n"));
    const dir = cMatch ? cMatch[1] : cwd;
    const branch = branchOf(dir);
    if (branch === "main" || branch === "master") {
      return ask(`this commits or merges on ${branch}; the workflow works on a branch (allow only if the user asked for it)`);
    }
    if (branch === "homolog" || branch === "staging") {
      return ask(`this commits or merges on ${branch}; promotion up the branch chain needs the user's explicit request`);
    }
    if ((branch === "dev" || branch === "develop") && gitHas(segments, "commit")) {
      return ask(
        `this commits directly on ${branch}; work goes on a branch merged into it (conclude a merge with git merge --continue)`,
      );
    }
  }

  return { decision: "none" };
}

// An Edit/Write target that is the grant file.
export function gitGrantPath(path) {
  return typeof path === "string" && (path === GRANT_FILE || path.endsWith("/" + GRANT_FILE))
    ? { decision: "deny", reason: `git:workflow guard: ${GRANT_REASON}` }
    : { decision: "none" };
}

// ------------------------------------------------------------------- db

const DB_CLIENTS = [
  "psql", "pg_dump", "pg_dumpall", "pg_restore", "pgcli",
  "mysql", "mariadb", "mysqldump", "mariadb-dump", "mysqladmin", "mycli",
  "sqlcmd", "mssql-cli", "bcp", "osql", "isql",
  "sqlplus", "sql", "sqlcl", "rman", "impdp", "expdp", "imp", "exp",
  "mongosh", "mongo", "mongodump", "mongorestore", "mongoimport",
  "sqlite3", "litecli", "usql", "cqlsh", "clickhouse-client",
];
const DB_RESTORE = ["pg_restore", "impdp", "imp", "mongorestore", "mongoimport"];
const DB_CLIENTS_RE = DB_CLIENTS.join("|");
const DB_CREDS_RE =
  /(^|[^A-Za-z0-9_.-])connections\.(env|toml)($|[^A-Za-z0-9_.-])|\.config\/ai-gent($|[^A-Za-z0-9_.-])|XDG_CONFIG_HOME\}?\/ai-gent($|[^A-Za-z0-9_.-])|AI_GENT_DB_(CONFIG|CONNECTIONS)/;
const DB_SQL_WRITE_RE =
  /(^|[^A-Za-z_])(INSERT|UPDATE|DELETE|MERGE|UPSERT|TRUNCATE|DROP|ALTER|CREATE|GRANT|REVOKE|RENAME|REPLACE[ \t]+INTO)([^A-Za-z_]|$)/i;

function dbSegments(cmd) {
  return cmd
    .replace(/"(?:[^"\\]|\\.)*"/g, '""')
    .replace(/'[^']*'/g, "''")
    .replace(/\\n/g, "\n")
    .replace(/[;&|()]/g, "\n")
    .split("\n");
}

const DB_PREFIX =
  "^[ \\t]*((sudo([ \\t]+-[^ \\t]+([ \\t]+[^- \\t][^ \\t]*)?)*|env|exec|nice|timeout[ \\t]+[0-9smh]+|[A-Za-z_][A-Za-z0-9_]*=[^ \\t]*)[ \\t]+)*";
const DB_DIRECT_RE = new RegExp(DB_PREFIX + "([^ \\t]*/)?(" + DB_CLIENTS_RE + ")([ \\t]|$)");
const DB_WRAPPED_RE = new RegExp(
  "(docker|podman|nerdctl|kubectl|oc)[ \\t].*(exec|run)[ \\t].*[ \\t/](" + DB_CLIENTS_RE + ")([ \\t]|$)",
);
const DB_TOOL_RE = new RegExp("(^|[ \\t/])(" + DB_CLIENTS_RE + ")([ \\t]|$)");

const DB_DENY = (reason) => ({ decision: "deny", reason: `db guard: ${reason}` });
const DB_ASK = (reason) => ({ decision: "ask", reason: `db guard: ${reason}` });

// A credentials file as a tool's target or a word on a command line is
// denied; text that merely names one (a document, a commit message) is not.
const DB_CREDS_DENY = () =>
  DB_DENY("that's a db credentials file; only dbrun reads it — use dbrun profiles / classify / test");

export function dbCredsShell(cmd) {
  // Quoted text with spaces in it (a commit message, an echo) is prose, not a
  // path: blank it, pairing each quote with its own closer; a quoted word
  // without spaces keeps its text.
  const words = cmd.replace(/"((?:[^"\\]|\\.)*)"|'([^']*)'/g, (m, dq, sq) => {
    const t = dq ?? sq;
    return /\s/.test(t) ? (dq !== undefined ? '""' : "''") : t;
  });
  return DB_CREDS_RE.test(words) ? DB_CREDS_DENY() : { decision: "none" };
}

export function dbCredsPath(path) {
  return typeof path === "string" && DB_CREDS_RE.test(path) ? DB_CREDS_DENY() : { decision: "none" };
}

// Claude Code's non-shell tools, as the shell guard reads them: only the target fields.
export function dbCredsTool(tool, input) {
  const keys = ["file_path", "notebook_path", "path", "glob", ...(tool === "Glob" ? ["pattern"] : [])];
  const hit = keys.some((k) => dbCredsPath(input?.[k]).decision === "deny");
  return hit ? DB_CREDS_DENY() : { decision: "none" };
}

export function dbGuard(cmd) {
  const segments = dbSegments(cmd);
  const hit = segments.find((s) => DB_DIRECT_RE.test(s) || DB_WRAPPED_RE.test(s));
  if (!hit) return { decision: "none" };

  const toolMatch = DB_TOOL_RE.exec(hit);
  const tool = toolMatch ? toolMatch[2] : "";
  if (DB_RESTORE.includes(tool)) {
    return DB_DENY(
      `${tool} restores into a database; local restores go through dbrun restore, anything else is a script for a person to run`,
    );
  }
  const flattened = cmd.replace(/\\n/g, " ").replace(/\\t/g, " ");
  if (DB_SQL_WRITE_RE.test(flattened)) {
    return DB_DENY(
      `a direct ${tool} call with SQL that writes; use dbrun (plan/apply on a proven local database, script for anything else)`,
    );
  }
  return DB_ASK(
    `${tool} bypasses dbrun (no classification, masking or command log); allow only if the user wants this direct call`,
  );
}

// -------------------------------------------------------------- secrets

const SECRETS_PREFIX = DB_PREFIX;
const SECRETS_GPG_RE = new RegExp(SECRETS_PREFIX + "([^ \\t]*/)?gpg2?([ \\t]|$)");
const SECRETS_RUNNER_RE = new RegExp(SECRETS_PREFIX + "([^ \\t]*/)?pass-run([ \\t]|$)");
const SECRETS_PASS_RE = new RegExp(SECRETS_PREFIX + "([^ \\t]*/)?pass([ \\t]|$)");
const SECRETS_PRINTERS = new Set([
  "env", "printenv", "set", "export", "declare", "typeset", "echo", "printf", "cat",
  "sh", "bash", "dash", "zsh", "ksh", "fish", "node", "perl", "ruby", "eval",
]);
const SECRETS_DENY = (reason) => ({ decision: "deny", reason: `secrets guard: ${reason}` });
const SECRETS_ASK = (reason) => ({ decision: "ask", reason: `secrets guard: ${reason}` });

const SECRETS_SHOW_HINT = "; list the entry in ~/.config/ai-gent/pass-allow to let the agent read it";

// The user's personal allowlist of pass entries the agent may `pass show`
// (same file and rules as plugins/secrets/hooks/guard.sh): AI_GENT_PASS_ALLOW,
// else ${XDG_CONFIG_HOME:-~/.config}/ai-gent/pass-allow. One rule per line:
// `entry` (exact) or `prefix/` (everything under it); blank lines and # comments
// ignored; rules that are empty, start with / - or ~, or have a . or .. path
// part never allow. A missing or unreadable file allows nothing.
function secretsAllowFile(env) {
  if (env.AI_GENT_PASS_ALLOW) return env.AI_GENT_PASS_ALLOW;
  const base = env.XDG_CONFIG_HOME || `${env.HOME ?? ""}/.config`;
  return `${base}/ai-gent/pass-allow`;
}

function secretsAllowed(entry, env, readFile) {
  let text;
  try {
    text = readFile(secretsAllowFile(env));
  } catch {
    return false;
  }
  for (const raw of text.split("\n")) {
    const rule = raw.replace(/\r$/, "").replace(/^[ \t]+/, "").replace(/[ \t]+$/, "");
    if (rule === "" || rule[0] === "#" || "/-~".includes(rule[0])) continue;
    if (rule.split("/").some((p) => p === ".." || p === ".")) continue;
    if (rule.endsWith("/")) {
      if (entry.startsWith(rule) && entry.length > rule.length) return true;
    } else if (rule === entry) {
      return true;
    }
  }
  return false;
}

function secretsShowEligible(sub, entry, flags, args, passCount, cmd) {
  if (sub !== "show" || flags !== 0 || args !== 2 || passCount !== 1) return false;
  if (entry === "" || /[^A-Za-z0-9._@+/-]/.test(entry)) return false;
  if (entry.startsWith("/") || entry.endsWith("/") || entry.includes("//")) return false;
  if (entry.split("/").some((p) => p === "." || p === "..")) return false;
  const lower = entry.toLowerCase();
  if (lower === "ai-gent/db" || lower.startsWith("ai-gent/db/")) return false;
  return !/PASSWORD_STORE_|XDG_CONFIG_HOME|(^|[^A-Za-z0-9_])HOME=/.test(cmd);
}

// Secret values in pass never reach the agent (shared/secrets.md).
export function secretsGuard(cmd, { env = process.env, readFile = (p) => readFileSync(p, "utf8") } = {}) {
  const segments = dbSegments(cmd);

  if (segments.some((s) => SECRETS_GPG_RE.test(s)) && /\.password-store|PASSWORD_STORE_DIR/.test(cmd)) {
    return SECRETS_DENY("decrypting the password store directly shows secret values; the runner that needs a secret reads it");
  }

  const runner = segments.find((s) => SECRETS_RUNNER_RE.test(s));
  if (runner !== undefined) {
    if (/(^|[=: \t])([A-Za-z0-9_.@+-]+\/)?ai-gent\/db\//.test(cmd)) {
      return SECRETS_DENY("ai-gent/db/ entries are read only by dbrun; use dbrun test / query");
    }
    const words = runner.trim().split(/[ \t]+/);
    const i = words.indexOf("--");
    const after = i >= 0 && i < words.length - 1 ? words[i + 1].split("/").pop() : "";
    if (SECRETS_PRINTERS.has(after) || after.startsWith("python")) {
      return SECRETS_DENY("that hands the secret to a command that can print it; run the tool that needs it after --");
    }
  }

  // Whatever names the allowlist file is the user's to approve: the agent must
  // not widen its own reach through Bash. (Write/Edit are outside this hook.)
  const allowVar = env.AI_GENT_PASS_ALLOW;
  if (cmd.includes("pass-allow") || cmd.includes("AI_GENT_PASS_ALLOW") || (allowVar && cmd.includes(allowVar))) {
    return SECRETS_ASK("that command mentions the pass allowlist (pass-allow); the user decides what the agent may read");
  }

  const hit = segments.find((s) => SECRETS_PASS_RE.test(s));
  if (hit === undefined) return { decision: "none" };
  const passCount = segments.filter((s) => SECRETS_PASS_RE.test(s)).length;
  const rest = hit.replace(SECRETS_PASS_RE, "");
  let sub = "";
  let entry = "";
  let flags = 0;
  let args = 0;
  for (const arg of rest.split(/[ \t]+/).filter(Boolean)) {
    if (/^(-c|--clip|-c[0-9].*|--clip=.*|-q|--qrcode|-q[0-9].*|--qrcode=.*)$/.test(arg)) {
      return SECRETS_DENY("pass -c/-q exposes a secret value; the agent never reads one — the runner that needs it does");
    }
    if (arg.startsWith("-")) {
      flags++;
      continue;
    }
    args++;
    if (!sub) sub = arg;
    else if (!entry) entry = arg;
  }
  if (["", "ls", "list", "find", "search", "help", "--help", "version", "--version"].includes(sub)) {
    return { decision: "none" };
  }
  if (sub === "insert" || sub === "add") {
    return SECRETS_DENY("the agent never writes a credential; ask the user to run `pass insert <entry>` in their own terminal");
  }
  if (["rm", "remove", "delete", "mv", "rename", "cp", "copy", "init", "git"].includes(sub)) {
    return SECRETS_ASK(`pass ${sub} changes the password store; the user decides`);
  }
  let hint = "";
  if (secretsShowEligible(sub, entry, flags, args, passCount, cmd)) {
    if (secretsAllowed(entry, env, readFile)) return { decision: "none" };
    hint = SECRETS_SHOW_HINT;
  }
  return SECRETS_DENY(
    `that shows a secret value from pass; the agent sees names only (pass ls) — the runner that needs a secret reads it${hint}`,
  );
}

// ----------------------------------------------------------------- repo

const REPO_RUNNER_RE =
  /^[ \t]*([A-Za-z_][A-Za-z0-9_]*=[^ \t]*[ \t]+)*((python3?|uv[ \t]+run([ \t]+python3?)?)[ \t]+)?([^ \t]*\/)?reporun(\.py)?[ \t]/;

// Every change reporun makes on a platform is confirmed by the user; a
// --write call carrying agent attribution is denied outright, the same rule
// git:workflow's guard enforces for commits and PRs/issues opened with `gh`.
// Whether the active grant lifts one reporun write segment: reporun cli github, then the gh words
// (pr create --base <covered>, pr merge, issue create|comment|edit).
function repoLifted(segment, grant) {
  const head = /^.*reporun(\.py)?[ \t]+cli[ \t]+github[ \t]*/;
  if (!head.test(segment) || !grant) return false;
  const words = segment
    .replace(head, "")
    .replace(/--write([ \t]|$)/g, "")
    .replace(/--entry[ \t]+[^ \t]+/, "")
    .replace(/^[ \t]*(--[ \t]+)?(gh[ \t]+)?/, "");
  const [noun, verb] = words.split(/[ \t]+/).filter(Boolean);
  if (noun === "pr" && verb === "create") {
    const base = /^.*[ \t](--base|-B)[ \t=]+([^ \t]+)/.exec(words);
    return grant.actions.includes("pr") && base !== null && grant.branches.includes(base[2]);
  }
  if (noun === "pr" && verb === "merge") return grant.actions.includes("merge");
  if (noun === "issue" && ["create", "comment", "edit"].includes(verb)) return grant.actions.includes("issues");
  return false;
}

export function repoGuard(cmd, { cwd = ".", grantOf = grantAt } = {}) {
  const writes = dbSegments(cmd).filter((s) => REPO_RUNNER_RE.test(s) && /[ \t]--write([ \t]|$)/.test(s));
  if (!writes.length) return { decision: "none" };
  if (/co-authored-by[ \t]*:|generated[ \t]+(with|by)[ \t]{0,3}\[?claude/i.test(cmd)) {
    return {
      decision: "deny",
      reason:
        "repo guard: this user's issues, work items, pull requests and comments carry no sign-off or mention of the agent that wrote them; remove it and run the command again",
    };
  }
  const grant = grantOf(cwd);
  if (writes.every((w) => repoLifted(w, grant))) return { decision: "none" };
  return {
    decision: "ask",
    reason:
      "repo guard: reporun --write changes an issue, work item or pull request on the platform; confirm this exact change",
  };
}

// ----------------------------------------------------------- containers

const CTR_PREFIX =
  "^[ \\t]*((sudo([ \\t]+-[^ \\t]+([ \\t]+[^- \\t][^ \\t]*)?)*|env|exec|nice|do|then|else|xargs([ \\t]+-[^ \\t]+)*|timeout[ \\t]+[0-9smh]+|[A-Za-z_][A-Za-z0-9_]*=[^ \\t]*)[ \\t]+)*";
const CTR_ENGINE = "(([^ \\t]*/)?(podman|docker|nerdctl)|\\$\\{?CONTAINER_ENGINE\\}?)";
const CTR_BUILDAH = "([^ \\t]*/)?buildah";
const CTR_WORDS = "([ \\t]+[^ \\t]+)*";
const CTR_PRUNE_RE = new RegExp(CTR_PREFIX + "(" + CTR_ENGINE + "|" + CTR_BUILDAH + ")" + CTR_WORDS + "[ \\t]+prune([ \\t]|$)");
const CTR_REMOVAL =
  CTR_PREFIX + "(" + CTR_ENGINE + "[ \\t]+((container|image|volume)[ \\t]+)?(rm|rmi)|" + CTR_BUILDAH + "[ \\t]+(rm|rmi))";
const CTR_REMOVE_RE = new RegExp(CTR_REMOVAL + "([ \\t]|$)");
const CTR_ALL_RE = new RegExp(CTR_REMOVAL + CTR_WORDS + "[ \\t]+(--all|-[a-zA-Z]*a[a-zA-Z]*)([ \\t]|$)");
const CTR_BUILDAH_PRUNE_RE = new RegExp(CTR_PREFIX + CTR_BUILDAH + "[ \\t]+rmi" + CTR_WORDS + "[ \\t]+(--prune|-p)([ \\t]|$)");
const CTR_LISTING_RE = new RegExp(
  CTR_PREFIX + CTR_ENGINE + "[ \\t]+((container|image|volume)[ \\t]+)?(ps|ls|list|images)" + CTR_WORDS +
    "[ \\t]+(--quiet|-[a-zA-Z]*q[a-zA-Z]*)([ \\t]|$)",
);
const CTR_DENY = (reason) => ({ decision: "deny", reason: `containers guard: ${reason}` });

function ctrSegments(cmd) {
  return cmd
    .replace(/"\$\{?CONTAINER_ENGINE\}?"/g, "$CONTAINER_ENGINE")
    .replace(/"(?:[^"\\]|\\.)*"/g, '""')
    .replace(/'[^']*'/g, "''")
    .replace(/\\n/g, "\n")
    .replace(/[;&|()`]/g, "\n")
    .split("\n");
}

// Container resources go by ID, own prefix or this project's label, never by
// a broad sweep (shared/containers.md §9, §9a).
export function containersGuard(cmd) {
  const segments = ctrSegments(cmd);
  const has = (re) => segments.some((s) => re.test(s));
  if (has(CTR_PRUNE_RE)) {
    return CTR_DENY(
      "prune removes other projects' images, volumes and in-flight builds; remove by ID or by this project's label (containers.md §9a: images -q --filter dangling=true --filter label=me.lucasvmigotto.ai-gent.project=<project>, then rmi each ID)",
    );
  }
  if (has(CTR_ALL_RE) || has(CTR_BUILDAH_PRUNE_RE)) {
    return CTR_DENY(
      "removing everything takes other projects' containers, images or volumes; remove by ID or by this project's label (containers.md §9)",
    );
  }
  if (has(CTR_REMOVE_RE) && has(CTR_LISTING_RE) && !/(label|name|id)=/.test(cmd)) {
    return CTR_DENY(
      "that removes whatever an unfiltered listing returns, other projects' included; filter the listing by label=me.lucasvmigotto.ai-gent.project=<project> (or your own name= prefix) (containers.md §9a)",
    );
  }
  return { decision: "none" };
}

// ----------------------------------------------------------- adapter

// Map one OpenCode permission evaluation to a guard decision. The strictest
// decision wins (deny beats ask beats none).
export function evaluate(event, { cwd = ".", branchOf = () => "", grantOf = grantAt } = {}) {
  const resources = Array.isArray(event.resources) ? event.resources : [];
  const candidates = [];

  if (event.action === "shell") {
    for (const cmd of resources) {
      candidates.push(gitGuard(cmd, { branchOf, cwd, grantOf }));
      candidates.push(dbGuard(cmd));
      candidates.push(dbCredsShell(cmd));
      candidates.push(secretsGuard(cmd));
      candidates.push(repoGuard(cmd, { cwd, grantOf }));
      candidates.push(containersGuard(cmd));
    }
  } else if (event.action === "read" || event.action === "edit") {
    for (const r of resources) {
      candidates.push(dbCredsPath(r));
      if (event.action === "edit") candidates.push(gitGrantPath(r));
    }
  }

  if (candidates.some((c) => c.decision === "deny")) {
    return candidates.find((c) => c.decision === "deny");
  }
  if (candidates.some((c) => c.decision === "ask")) {
    return candidates.find((c) => c.decision === "ask");
  }
  return { decision: "none" };
}

// ----------------------------------------------------------- words and log

// The guards' own words and log rows (specs 015 and 024, decided 2026-11-14): a deny or ask is
// shown with the plain words of its table entry, and one row per decision (the entry and the
// command's verb, never the reason, the command or a path) goes to the session's guard log.
// words.json is a copy of shared/guard-words.json; the shell guards (the generated `guard words`
// block of each guard.sh) and the binary (guard/words.rs) do the same, checked by
// scripts/guard-fixtures.py run.
export const WORDS = JSON.parse(readFileSync(new URL("./words.json", import.meta.url), "utf8"));

function owner(reason) {
  if (typeof reason !== "string") return null;
  for (const [id, g] of Object.entries(WORDS.guards)) {
    const at = reason.indexOf(g.marker);
    if (at < 0) continue;
    const text = reason.slice(at + g.marker.length).trim();
    return { id, guard: g, text, entry: g.entries.find((e) => text.includes(e.match)) ?? null };
  }
  return null;
}

// { message, context }: for the person, and on a deny one line for the model; nulls when none.
export function guardWords(decision, reason) {
  const o = owner(reason);
  if (!o || (decision !== "deny" && decision !== "ask")) return { message: null, context: null };
  const name = o.guard.marker.replace(/:$/, "");
  const e = o.entry;
  if (decision === "deny") {
    const why = e ? (e.instead ? `${e.say} Instead: ${e.instead}` : e.say) : o.text;
    return { message: `${name} refused this. ${why} (${o.guard.skill})`, context: e?.instead ? `${name}: instead, ${e.instead}` : null };
  }
  if (!e) return { message: null, context: null };
  return { message: `${name} asks: ${e.say}${e.allowIf ? ` ${e.allowIf}` : ""} (${o.guard.skill})`, context: null };
}

// The command's verb: the first word and, for a first word the table's verbs lists, the second
// word when it is a listed subcommand. Never a value, an entry, a branch or a path.
export function guardVerb(command, tool) {
  if (typeof command !== "string" || tool !== "Bash") return null;
  const words = command.trim().split(/[ \t\n\r]+/);
  const raw = words[0] ?? "";
  if (raw.includes("://")) return null;
  const first = raw.slice(raw.lastIndexOf("/") + 1);
  if (!/^[a-z][a-z0-9._-]{0,23}$/.test(first) || first.replace(/[^0-9]/g, "").length >= 7) return null;
  const second = words[1] ?? "";
  return Object.hasOwn(WORDS.verbs, first) && WORDS.verbs[first].includes(second) ? `${first} ${second}` : first;
}

// The log row of a deny or ask a guard owns, or null.
export function guardRow(decision, reason, tool, command, at) {
  const o = owner(reason);
  if (!o || (decision !== "deny" && decision !== "ask")) return null;
  const t = typeof tool === "string" && /^[A-Za-z][A-Za-z0-9_.:-]{0,63}$/.test(tool) ? tool : "unknown";
  return { at, guard: o.id, decision, entry: o.entry?.match ?? "generic", tool: t, verb: guardVerb(command, t), outcome: decision === "ask" ? "asked" : null };
}

// AI_GENT_GUARD_LOG_DIR, else the state directory's ai-gent/guard-log (as the binary and guard.sh).
export function guardLogDir(env = process.env) {
  if (env.AI_GENT_GUARD_LOG_DIR) return env.AI_GENT_GUARD_LOG_DIR;
  if (env.XDG_STATE_HOME) return join(env.XDG_STATE_HOME, "ai-gent", "guard-log");
  if (env.HOME) return join(env.HOME, ".local", "state", "ai-gent", "guard-log");
  if (env.LOCALAPPDATA) return join(env.LOCALAPPDATA, "ai-gent", "state", "guard-log");
  return null;
}

// Whether a plugin option is set to false (CLAUDE_PLUGIN_OPTION_<key>, as written or upper-cased),
// read the way the binary and guard.sh read it.
export function optionOff(key, env = process.env) {
  return env[`CLAUDE_PLUGIN_OPTION_${key}`] === "false" || env[`CLAUDE_PLUGIN_OPTION_${key.toUpperCase()}`] === "false";
}

// What OpenCode shows for a decision: the person's line, the guard's reason and, on a deny, the
// line for the agent (OpenCode has one message for both, where Claude Code has systemMessage and
// additionalContext), and whether to log it. guardUx false leaves the reason alone; guardLog
// false writes no row.
export function guardPresent(decision, reason, env = process.env) {
  const { message, context } = optionOff("guardUx", env) ? { message: null, context: null } : guardWords(decision, reason);
  return { message: [message, reason, context].filter(Boolean).join("\n\n"), log: !optionOff("guardLog", env) };
}

const OWN = typeof process.getuid === "function" ? process.getuid() : null;
const NOFOLLOW = FS.O_NOFOLLOW ?? 0;
const NONBLOCK = FS.O_NONBLOCK ?? 0;

// Open the log without following a symlink or blocking on a FIFO; anything but a regular file is
// refused (throws).
function openLog(file, flags) {
  if (!NOFOLLOW) {
    try {
      if (!lstatSync(file).isFile()) throw new Error("not a regular file");
    } catch (e) {
      if (e.code !== "ENOENT") throw e;
    }
  }
  const fd = openSync(file, flags | NOFOLLOW | NONBLOCK, 0o600);
  const st = fstatSync(fd);
  if (!st.isFile()) {
    closeSync(fd);
    throw new Error("not a regular file");
  }
  return { fd, st };
}

// The last `cap` bytes of an open file.
function readTail(fd, size, cap) {
  const n = Math.min(size, cap);
  const buf = Buffer.alloc(n);
  let got = 0;
  while (got < n) {
    const r = readSync(fd, buf, got, n - got, size - n + got);
    if (r <= 0) break;
    got += r;
  }
  return buf.subarray(0, got).toString("utf8");
}

// Append a row to <dir>/<session>.jsonl (directory 0700, file 0600; one we own is tightened), only
// to a regular file, never through a symlink. Past twice the row limit (or maxBytes) the file is
// cut to its newest rows, read from its last maxBytes only, under <session>.jsonl.trim (created
// exclusively; held by another writer, the cut waits for the next row). Never throws.
export function appendGuardRow(dir, session, row) {
  if (!dir || !row || typeof session !== "string" || !/^[A-Za-z0-9_-]{1,128}$/.test(session)) return;
  try {
    mkdirSync(dir, { recursive: true, mode: 0o700 });
    const d = statSync(dir);
    if (OWN !== null && d.uid === OWN && (d.mode & 0o077) !== 0) chmodSync(dir, 0o700);
    const file = join(dir, `${session}.jsonl`);
    const { fd, st } = openLog(file, FS.O_WRONLY | FS.O_APPEND | FS.O_CREAT);
    let size;
    try {
      if (OWN !== null && st.uid === OWN && (st.mode & 0o077) !== 0) chmodSync(file, 0o600);
      writeSync(fd, JSON.stringify(row) + "\n");
      size = fstatSync(fd).size;
    } finally {
      closeSync(fd);
    }
    const cap = WORDS.maxBytes;
    let text = null;
    if (size <= cap) {
      const r = openLog(file, FS.O_RDONLY);
      try {
        text = readTail(r.fd, r.st.size, cap);
      } finally {
        closeSync(r.fd);
      }
      if (text.split("\n").filter(Boolean).length <= 2 * WORDS.maxRows) return;
    }
    trimGuardLog(dir, session, file, cap);
  } catch {
    // a row that cannot be written is lost, never the decision
  }
}

function trimGuardLog(dir, session, file, cap) {
  const lock = join(dir, `${session}.jsonl.trim`);
  let lfd;
  for (let tries = 0; ; tries++) {
    try {
      lfd = openSync(lock, FS.O_WRONLY | FS.O_CREAT | FS.O_EXCL | NOFOLLOW, 0o600);
      break;
    } catch (e) {
      if (e.code !== "EEXIST" || tries >= 1) return;
      // a lock left by a writer that died is stale after 30 s
      try {
        if (Date.now() - lstatSync(lock).mtimeMs < 30_000) return;
        unlinkSync(lock);
      } catch {
        return;
      }
    }
  }
  let tmp = null;
  try {
    const r = openLog(file, FS.O_RDONLY);
    let text;
    try {
      text = readTail(r.fd, r.st.size, cap);
    } finally {
      closeSync(r.fd);
    }
    const lines = text.split("\n").filter(Boolean);
    if (r.st.size > cap) lines.shift(); // a cut from the middle starts on a partial line
    tmp = join(dir, `.${session}.jsonl.${randomBytes(8).toString("hex")}`);
    const tfd = openSync(tmp, FS.O_WRONLY | FS.O_CREAT | FS.O_EXCL | NOFOLLOW, 0o600);
    try {
      writeSync(tfd, lines.slice(-WORDS.maxRows).join("\n") + "\n");
    } finally {
      closeSync(tfd);
    }
    renameSync(tmp, file);
    tmp = null;
  } finally {
    if (tmp) try { unlinkSync(tmp); } catch {}
    closeSync(lfd);
    try { unlinkSync(lock); } catch {}
  }
}
