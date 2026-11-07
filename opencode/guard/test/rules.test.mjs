import { execFileSync } from "node:child_process"
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { gitGuard, gitGrantPath, dbGuard, dbCredsShell, dbCredsTool, secretsGuard, repoGuard, containersGuard, evaluate } from "../rules.mjs"

// Parity test: the OpenCode rules must decide exactly what the Claude Code
// guards (plugins/{git,db,secrets,repo,devcontainer}/hooks/guard.sh) decide for
// the same cases. guard.sh is the oracle; the cases are the shared fixtures
// (shared/guard-fixtures/*.json, spec 033), which the shell test suites and
// scripts/guard-fixtures.py read too, so the implementations cannot drift.

const REPO = new URL("../../..", import.meta.url).pathname.replace(/\/$/, "")
const GIT_GUARD = join(REPO, "plugins/git/hooks/guard.sh")
const DB_GUARD = join(REPO, "plugins/db/hooks/guard.sh")
const SECRETS_GUARD = join(REPO, "plugins/secrets/hooks/guard.sh")
const REPO_GUARD = join(REPO, "plugins/repo/hooks/guard.sh")
const CONTAINERS_GUARD = join(REPO, "plugins/devcontainer/hooks/guard.sh")

const FIXTURES = join(REPO, "shared/guard-fixtures")
const load = (name) => JSON.parse(readFileSync(join(FIXTURES, `${name}.json`), "utf8"))

const sandbox = mkdtempSync(join(tmpdir(), "ai-gent-guard-"))
process.on("exit", () => rmSync(sandbox, { recursive: true, force: true }))

const dirs = {}
for (const b of ["main", "staging", "dev", "feat/x"]) {
  const d = join(sandbox, b.replace("/", "-"))
  execFileSync("git", ["init", "-q", "-b", b, d])
  dirs[b] = d
}
// The autonomy grant a sandbox repository carries, as the user would write it.
const GRANTS = {
  granted: "expires: 2099-12-31T23:59:59Z\nbranches: develop dev\nactions: push pr merge issues\n",
  expired: "expires: 2000-01-01T00:00:00Z\nbranches: develop dev\nactions: push pr merge issues\n",
  partial: "expires: 2099-12-31T23:59:59Z\nactions: issues\n",
}
for (const [k, text] of Object.entries(GRANTS)) {
  const d = join(sandbox, k)
  execFileSync("git", ["init", "-q", "-b", "feat/x", d])
  mkdirSync(join(d, ".ai-gent"))
  writeFileSync(join(d, ".ai-gent", "autonomy"), text)
  dirs[k] = d
}
const main = dirs["main"]
const staging = dirs["staging"]
const dev = dirs["dev"]
const feat = dirs["feat/x"]

const branchOf = (dir) => {
  try {
    return execFileSync("git", ["-C", dir, "symbolic-ref", "--quiet", "--short", "HEAD"], {
      encoding: "utf8",
    }).trim()
  } catch {
    return ""
  }
}

let failures = 0
let count = 0

function check(label, want, got) {
  count++
  const ok = want === got
  if (!ok) failures++
  console.log(`      ${ok ? "ok   " : "FAIL "} want ${want}, got ${got}: ${label}`)
}

// The user's real allowlist must never leak into these runs: every shell guard run sees an
// allowlist path that does not exist unless the case sets its own environment.
const NO_ALLOWLIST = join(sandbox, "no-such-allowlist")
process.env.AI_GENT_PASS_ALLOW = NO_ALLOWLIST

// env: null runs with this process's environment; an object runs with only PATH and those variables.
function shellRaw(guard, tool, cwd, toolInput, env = null) {
  const event = JSON.stringify({
    hook_event_name: "PreToolUse",
    tool_name: tool,
    cwd,
    tool_input: toolInput,
  })
  const opts = { input: event, encoding: "utf8" }
  if (env) opts.env = { PATH: process.env.PATH, AI_GENT_NO_SHADOW: "1", ...env }
  return execFileSync("sh", [guard], opts)
}

function shellDecision(guard, tool, cwd, toolInput, env = null) {
  const m = /"permissionDecision":"([a-z]+)"/.exec(shellRaw(guard, tool, cwd, toolInput, env))
  return m ? m[1] : "none"
}

// ------------------------------------------------------- fixtures

const GUARD_FILES = { git: GIT_GUARD, db: DB_GUARD, secrets: SECRETS_GUARD, repo: REPO_GUARD, containers: CONTAINERS_GUARD }
const branchDirs = { main, staging, dev, feat, granted: dirs.granted, expired: dirs.expired, partial: dirs.partial }
const subst = (s) => s.replace(/\{(main|staging|dev|feat|granted|expired|partial)\}/g, (_, k) => branchDirs[k])
const cwdOf = (c) => (c.startsWith("@") ? branchDirs[c.slice(1)] : c)
const sample = (c) => (c.command ?? `${c.tool} ${JSON.stringify(c.tool_input)}`).replace(/\n/g, "⏎")

function ocDecision(guard, c, cwd, command) {
  if (c.tool !== "Bash") {
    // The git guard reads an Edit or Write target (the autonomy grant is the user's to write).
    if (guard === "git") return gitGrantPath(c.tool_input?.file_path ?? c.tool_input?.notebook_path).decision
    return dbCredsTool(c.tool, c.tool_input ?? {}).decision
  }
  switch (guard) {
    case "git":
      return gitGuard(command, { cwd, branchOf }).decision
    case "db": {
      const creds = dbCredsShell(command)
      return creds.decision !== "none" ? creds.decision : dbGuard(command).decision
    }
    case "secrets":
      return secretsGuard(command).decision
    case "repo":
      return repoGuard(command, { cwd }).decision
    case "containers":
      return containersGuard(command).decision
  }
  throw new Error(`unknown guard ${guard}`)
}

for (const guard of Object.keys(GUARD_FILES)) {
  console.log(`    ${guard}: TypeScript rules vs ${GUARD_FILES[guard].replace(REPO + "/", "")}`)
  for (const c of load(guard).cases) {
    // POSIX shell cases only: the PowerShell and cmd variants belong to the Rust evaluator
    if ((c.shell ?? "sh") !== "sh" || (c.tags ?? []).includes("new")) continue
    const command = c.command === undefined ? undefined : subst(c.command)
    const cwd = cwdOf(c.cwd)
    const toolInput = command === undefined ? c.tool_input : { command }
    if (!(c.skip_in ?? []).includes("mjs")) check(sample(c), c.want, ocDecision(guard, c, cwd, command))
    check(`shell ${sample(c)}`, c.want, shellDecision(GUARD_FILES[guard], c.tool, cwd, toolInput))
  }
}

// ------------------------------------------------- pass allowlist

// The secrets guard's personal allowlist, through a temp file named by AI_GENT_PASS_ALLOW (never
// the real ~/.config). The shell guard and the TypeScript rules must agree on every case.
console.log("    pass allowlist: TypeScript rules vs plugins/secrets/hooks/guard.sh")
{
  const allow = join(sandbox, "pass-allow")
  writeFileSync(
    allow,
    [
      "# a comment",
      "",
      "  work/vpn  ",
      "team/",
      "ai-gent/",
      "ai-gent/db/shop-dev",
      "dos\r",
      "/abs/entry",
      "-dash",
      "~home/x",
      "../up",
      "a/../b",
      "a/./b",
      "mal/../",
      "bad entry",
    ].join("\n") + "\n",
  )
  const cases = [
    // allowed: exact and prefix rules, comments and blanks skipped, CRLF and padding trimmed
    ["pass show work/vpn", "none"],
    ["pass show team/a/b", "none"],
    ["pass show ai-gent/github/pat", "none"],
    ["pass show work/vpn | head -1", "none"],
    ["echo ok && pass show team/x", "none"],
    ["sudo -u me pass show team/x", "none"],
    ["pass show dos", "none"],
    // no rule matches
    ["pass show work/vpn2", "deny"],
    ["pass show work", "deny"],
    ["pass show team/", "deny"],
    ["pass show other/entry", "deny"],
    ["pass show a/b", "deny"],
    // malformed rules never allow
    ["pass show /abs/entry", "deny"],
    ["pass show -dash", "deny"],
    ["pass show ~home/x", "deny"],
    ["pass show ../up", "deny"],
    ["pass show a/../b", "deny"],
    ["pass show a/./b", "deny"],
    ["pass show bad entry", "deny"],
    // only `pass show <entry>`, no flags: never the bare form, other verbs or any flag
    ["pass work/vpn", "deny"],
    ["pass -c work/vpn", "deny"],
    ["pass show -c work/vpn", "deny"],
    ["pass show --clip work/vpn", "deny"],
    ["pass show -q work/vpn", "deny"],
    ["pass show --qrcode=2 work/vpn", "deny"],
    ["pass show -n work/vpn", "deny"],
    ["pass show work/vpn extra", "deny"],
    ["pass show work/vpn > /tmp/x", "deny"],
    ["pass show", "deny"],
    ["pass edit work/vpn", "deny"],
    ["pass grep work/vpn", "deny"],
    ["pass generate team/x 20", "deny"],
    ["pass otp team/x", "deny"],
    ["pass insert team/x", "deny"],
    ["pass add team/x", "deny"],
    // every other pass command in the line makes it a deny
    ["pass show work/vpn; pass show other/x", "deny"],
    ["pass show work/vpn | pass show other/x", "deny"],
    // ai-gent/db/ stays dbrun-only even when a rule names it
    ["pass show ai-gent/db/shop-dev", "deny"],
    ["pass show ai-gent/db/other", "deny"],
    ["pass show AI-Gent/DB/shop-dev", "deny"],
    ["pass show ai-gent/db", "deny"],
    // shell metacharacters, substitutions and odd shapes in the name
    ["pass show team/$(id)", "deny"],
    ["pass show team/`id`", "deny"],
    ["pass show team/$HOME", "deny"],
    ["pass show team/*", "deny"],
    ["pass show team/a?b", "deny"],
    ["pass show team/{a,b}", "deny"],
    ["pass show team/a\\ b", "deny"],
    ["pass show 'team/x'", "deny"],
    ['pass show "team/x"', "deny"],
    ["pass show team//x", "deny"],
    ["pass show team/../work/vpn", "deny"],
    ["pass show ./team/x", "deny"],
    // a store or config override in the command
    ["PASSWORD_STORE_DIR=/tmp/s pass show team/x", "deny"],
    ["env PASSWORD_STORE_GPG_OPTS=x pass show team/x", "deny"],
    ["XDG_CONFIG_HOME=/tmp/x pass show team/x", "deny"],
    ["HOME=/tmp/x pass show team/x", "deny"],
    // the command mentions the allowlist file: the user decides
    ["echo 'team/' >> ~/.config/ai-gent/pass-allow", "ask"],
    ["cat ~/.config/ai-gent/pass-allow", "ask"],
    ["sed -i s/x/y/ $XDG_CONFIG_HOME/ai-gent/pass-allow", "ask"],
    ["AI_GENT_PASS_ALLOW=/tmp/mine pass show other/x", "ask"],
    [`echo x >> ${allow}`, "ask"],
    [`pass show team/x ; echo ${allow}`, "ask"],
    ["ls ~/.config/ai-gent", "none"],
  ]
  for (const [command, want] of cases) {
    check(`allow: ${command}`, want, secretsGuard(command, { env: { AI_GENT_PASS_ALLOW: allow } }).decision)
    check(`allow shell: ${command}`, want, shellDecision(SECRETS_GUARD, "Bash", "/tmp", { command }, { AI_GENT_PASS_ALLOW: allow }))
  }

  // the default location: $XDG_CONFIG_HOME, then ~/.config; AI_GENT_PASS_ALLOW wins over both
  const xdg = join(sandbox, "xdg")
  mkdirSync(join(xdg, "ai-gent"), { recursive: true })
  writeFileSync(join(xdg, "ai-gent", "pass-allow"), "xdg/entry\n")
  const home = join(sandbox, "home")
  mkdirSync(join(home, ".config", "ai-gent"), { recursive: true })
  writeFileSync(join(home, ".config", "ai-gent", "pass-allow"), "home/entry\n")
  const both = { XDG_CONFIG_HOME: xdg, HOME: home }
  for (const [env, command, want] of [
    [both, "pass show xdg/entry", "none"],
    [both, "pass show home/entry", "deny"],
    [{ HOME: home }, "pass show home/entry", "none"],
    [{ HOME: home }, "pass show xdg/entry", "deny"],
    [{ ...both, AI_GENT_PASS_ALLOW: allow }, "pass show xdg/entry", "deny"],
    [{ ...both, AI_GENT_PASS_ALLOW: allow }, "pass show work/vpn", "none"],
  ]) {
    const label = `${Object.keys(env).join("+")}: ${command}`
    check(`allow env ${label}`, want, secretsGuard(command, { env }).decision)
    check(`allow env shell ${label}`, want, shellDecision(SECRETS_GUARD, "Bash", "/tmp", { command }, env))
  }

  // a missing, empty or unreadable file allows nothing: deny, exactly as without the feature
  const empty = join(sandbox, "empty-allow")
  writeFileSync(empty, "")
  for (const [label, env] of [
    ["missing file", { AI_GENT_PASS_ALLOW: NO_ALLOWLIST }],
    ["empty file", { AI_GENT_PASS_ALLOW: empty }],
    ["a directory", { AI_GENT_PASS_ALLOW: sandbox }],
    ["no HOME", {}],
  ]) {
    check(`allow ${label}`, "deny", secretsGuard("pass show team/x", { env }).decision)
    check(`allow ${label} shell`, "deny", shellDecision(SECRETS_GUARD, "Bash", "/tmp", { command: "pass show team/x" }, env))
  }

  // the reasons: today's text, plus the hint only where a rule could have opened the command
  const BASE = "secrets guard: that shows a secret value from pass; the agent sees names only (pass ls) — the runner that needs a secret reads it"
  const HINT = "; list the entry in ~/.config/ai-gent/pass-allow to let the agent read it"
  const shellReason = (command, env) =>
    /"permissionDecisionReason":"((?:[^"\\]|\\.)*)"/.exec(shellRaw(SECRETS_GUARD, "Bash", "/tmp", { command }, env))?.[1]
  const env = { AI_GENT_PASS_ALLOW: allow }
  for (const [command, reason] of [
    ["pass show other/entry", BASE + HINT],
    ["pass show ai-gent/db/shop-dev", BASE],
    ["pass work/vpn", BASE],
    ["pass show -n work/vpn", BASE],
    ["cat ~/.config/ai-gent/pass-allow", "secrets guard: that command mentions the pass allowlist (pass-allow); the user decides what the agent may read"],
  ]) {
    check(`reason: ${command}`, reason, secretsGuard(command, { env }).reason)
    check(`reason shell: ${command}`, reason, shellReason(command, env))
  }
}

// ------------------------------------------------------------- adapter

console.log("    evaluate(): OpenCode permission event adapter")
for (const c of load("adapter").cases) {
  check(JSON.stringify(c.event), c.want, evaluate(c.event, { cwd: cwdOf(c.cwd), branchOf }).decision)
}

console.log(`\n    ${count} checks, ${failures} failed`)
process.exit(failures ? 1 : 0)
