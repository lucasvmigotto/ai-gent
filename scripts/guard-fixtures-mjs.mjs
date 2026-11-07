// Runs guard fixture cases through opencode/guard/rules.mjs (spec 033).
// Stdin: a JSON array of {guard, tool, cwd, command, tool_input} (paths already
// substituted). Stdout: a JSON array of {decision, reason} in the same order.
// Used by scripts/guard-fixtures.py --impl mjs.
import { execFileSync } from "node:child_process"
import { readFileSync } from "node:fs"
import { gitGuard, gitGrantPath, dbGuard, dbCredsShell, dbCredsTool, secretsGuard, repoGuard, containersGuard } from "../opencode/guard/rules.mjs"

const branchOf = (dir) => {
  try {
    return execFileSync("git", ["-C", dir, "symbolic-ref", "--quiet", "--short", "HEAD"], { encoding: "utf8" }).trim()
  } catch {
    return ""
  }
}

function run(c) {
  if (c.tool !== "Bash") {
    // The git guard reads an Edit or Write target (the autonomy grant is the user's to write).
    if (c.guard === "git") return gitGrantPath(c.tool_input?.file_path ?? c.tool_input?.notebook_path)
    return dbCredsTool(c.tool, c.tool_input ?? {})
  }
  switch (c.guard) {
    case "git":
      return gitGuard(c.command, { cwd: c.cwd, branchOf })
    case "db": {
      const creds = dbCredsShell(c.command)
      return creds.decision !== "none" ? creds : dbGuard(c.command)
    }
    case "secrets":
      return secretsGuard(c.command)
    case "repo":
      return repoGuard(c.command, { cwd: c.cwd })
    case "containers":
      return containersGuard(c.command)
    default:
      throw new Error(`unknown guard ${c.guard}`)
  }
}

const cases = JSON.parse(readFileSync(0, "utf8"))
process.stdout.write(JSON.stringify(cases.map((c) => {
  const r = run(c)
  return { decision: r.decision, reason: r.reason ?? null }
})))
