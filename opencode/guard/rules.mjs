// ai-gent guard rules — the OpenCode side of the git and db guards.
//
// These are a faithful TypeScript/JavaScript port of plugins/git/hooks/guard.sh
// and plugins/db/hooks/guard.sh, which stay the Claude Code enforcement. The
// two are kept from drifting by opencode/guard/test/rules.test.mjs, which runs
// both against the same vectors and asserts identical decisions.
//
// The input here is the *raw* command string OpenCode gives the permission
// hook (not JSON-escaped, as the shell guards receive from Claude Code). Each
// function returns { decision: "deny" | "ask" | "none", reason?: string }.

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

// branchOf(dir) returns the current branch name ("" when unknown). The plugin
// passes a git-backed resolver; tests do too.
export function gitGuard(cmd, { cwd = ".", branchOf = () => "" } = {}) {
  const segments = gitSegments(cmd);
  const deny = (reason) => ({ decision: "deny", reason: `git:workflow guard: ${reason}` });
  const ask = (reason) => ({ decision: "ask", reason: `git:workflow guard: ${reason}` });

  if (
    gitHas(segments, "(commit|merge|push|am|rebase|cherry-pick|revert)", "(^|[ \\t])--no-verify([ \\t]|$)") ||
    gitHas(segments, "commit", "(^|[ \\t])-n([ \\t]|$)")
  ) {
    return deny("hooks must not be bypassed (--no-verify / -n); fix what the hook reports and try again");
  }

  if (
    gitHas(segments, "(commit|merge|revert|cherry-pick|tag|notes)") &&
    /co-authored-by/i.test(cmd)
  ) {
    return deny("this user's commits carry no Co-Authored-By trailers; remove it and commit again");
  }

  if (
    gitHas(segments, "push", "(^|[ \\t])(--force|-[a-zA-Z]*f[a-zA-Z]*)([ \\t]|$)") &&
    !gitHas(segments, "push", "--force-with-lease")
  ) {
    return deny("force pushes use --force-with-lease, and only on the user's explicit order");
  }

  if (gitHas(segments, "push")) {
    return ask("git push publishes; allow only if the user explicitly asked for this push");
  }
  if (segments.some((s) => /^[ \t]*gh[ \t]+pr[ \t]+create/.test(s))) {
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
  }

  return { decision: "none" };
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
const DB_CREDS_RE = /connections\.env|\.config\/ai-gent|XDG_CONFIG_HOME\}?\/ai-gent|AI_GENT_DB_CONFIG/;
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

// Anything (any tool) touching the credentials file is denied.
export function dbCredsGuard(text) {
  if (DB_CREDS_RE.test(text)) {
    return DB_DENY("that's the db credentials file; only dbrun reads it — use dbrun profiles / classify / test");
  }
  return { decision: "none" };
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

// ----------------------------------------------------------- adapter

// Map one OpenCode permission evaluation to a guard decision. The strictest
// decision wins (deny beats ask beats none).
export function evaluate(event, { cwd = ".", branchOf = () => "" } = {}) {
  const resources = Array.isArray(event.resources) ? event.resources : [];
  const candidates = [];

  if (event.action === "shell") {
    for (const cmd of resources) {
      candidates.push(gitGuard(cmd, { branchOf, cwd }));
      candidates.push(dbGuard(cmd));
      candidates.push(dbCredsGuard(cmd));
    }
  } else if (event.action === "read" || event.action === "edit") {
    for (const r of resources) candidates.push(dbCredsGuard(r));
  }

  if (candidates.some((c) => c.decision === "deny")) {
    return candidates.find((c) => c.decision === "deny");
  }
  if (candidates.some((c) => c.decision === "ask")) {
    return candidates.find((c) => c.decision === "ask");
  }
  return { decision: "none" };
}
