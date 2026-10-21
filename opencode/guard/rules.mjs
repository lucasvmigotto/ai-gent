// ai-gent guard rules — the OpenCode side of the git, db, secrets and repo guards.
//
// These are a faithful TypeScript/JavaScript port of plugins/git/hooks/guard.sh,
// plugins/db/hooks/guard.sh, plugins/secrets/hooks/guard.sh and
// plugins/repo/hooks/guard.sh, which stay the Claude Code enforcement. The
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

// Secret values in pass never reach the agent (shared/secrets.md).
export function secretsGuard(cmd) {
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

  const hit = segments.find((s) => SECRETS_PASS_RE.test(s));
  if (hit === undefined) return { decision: "none" };
  const rest = hit.replace(SECRETS_PASS_RE, "");
  let sub = "";
  for (const arg of rest.split(/[ \t]+/).filter(Boolean)) {
    if (/^(-c|--clip|-c[0-9].*|--clip=.*|-q|--qrcode|-q[0-9].*|--qrcode=.*)$/.test(arg)) {
      return SECRETS_DENY("pass -c/-q exposes a secret value; the agent never reads one — the runner that needs it does");
    }
    if (arg.startsWith("-")) continue;
    if (!sub) sub = arg;
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
  return SECRETS_DENY(
    "that shows a secret value from pass; the agent sees names only (pass ls) — the runner that needs a secret reads it",
  );
}

// ----------------------------------------------------------------- repo

const REPO_RUNNER_RE =
  /^[ \t]*([A-Za-z_][A-Za-z0-9_]*=[^ \t]*[ \t]+)*((python3?|uv[ \t]+run([ \t]+python3?)?)[ \t]+)?([^ \t]*\/)?reporun(\.py)?[ \t]/;

// Every change reporun makes on a platform is confirmed by the user; a
// --write call carrying agent attribution is denied outright, the same rule
// git:workflow's guard enforces for commits and PRs/issues opened with `gh`.
export function repoGuard(cmd) {
  const hit = dbSegments(cmd).some((s) => REPO_RUNNER_RE.test(s) && /[ \t]--write([ \t]|$)/.test(s));
  if (!hit) return { decision: "none" };
  if (/co-authored-by[ \t]*:|generated[ \t]+(with|by)[ \t]{0,3}\[?claude/i.test(cmd)) {
    return {
      decision: "deny",
      reason:
        "repo guard: this user's issues, work items, pull requests and comments carry no sign-off or mention of the agent that wrote them; remove it and run the command again",
    };
  }
  return {
    decision: "ask",
    reason:
      "repo guard: reporun --write changes an issue, work item or pull request on the platform; confirm this exact change",
  };
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
      candidates.push(dbCredsShell(cmd));
      candidates.push(secretsGuard(cmd));
      candidates.push(repoGuard(cmd));
    }
  } else if (event.action === "read" || event.action === "edit") {
    for (const r of resources) candidates.push(dbCredsPath(r));
  }

  if (candidates.some((c) => c.decision === "deny")) {
    return candidates.find((c) => c.decision === "deny");
  }
  if (candidates.some((c) => c.decision === "ask")) {
    return candidates.find((c) => c.decision === "ask");
  }
  return { decision: "none" };
}
