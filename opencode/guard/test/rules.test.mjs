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

function shellDecision(guard, tool, cwd, toolInput) {
  const event = JSON.stringify({
    hook_event_name: "PreToolUse",
    tool_name: tool,
    cwd,
    tool_input: toolInput,
  })
  const out = execFileSync("sh", [guard], { input: event, encoding: "utf8" })
  const m = /"permissionDecision":"([a-z]+)"/.exec(out)
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

// ------------------------------------------------------------- adapter

console.log("    evaluate(): OpenCode permission event adapter")
for (const c of load("adapter").cases) {
  check(JSON.stringify(c.event), c.want, evaluate(c.event, { cwd: cwdOf(c.cwd), branchOf }).decision)
}

console.log(`\n    ${count} checks, ${failures} failed`)
process.exit(failures ? 1 : 0)
