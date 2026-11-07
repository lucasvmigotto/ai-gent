// Binary-backed decisions for the OpenCode guard plugin (spec 033, US4, FR-014).
//
// From a package install the plugin asks `ai-gent guard --guard all --format
// opencode` instead of running rules.mjs: the event goes in on stdin as
// {"action","resources","cwd"} and {"decision":"deny|ask|none","reason":"..."}
// comes back on stdout, exit 0. Any failure (binary missing or not
// executable, non-zero exit, unparseable or unknown answer, timeout) becomes
// an `ask` that names the install command; a failure never allows and never
// denies. The binary resolves branches itself, so no branchOf is passed.

import { execFileSync } from "node:child_process"
import { existsSync } from "node:fs"
import { join } from "node:path"

export const INSTALL_COMMAND =
  "curl -fsSL https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.sh | sh"
export const MARKER = "ai-gent guard:"
export const TIMEOUT_MS = 2000

// A package install has VERSION at its root, next to bin/ and opencode/guard/.
// A git clone has no VERSION and keeps deciding with rules.mjs.
export function packageInstall(root, platform = process.platform) {
  if (!existsSync(join(root, "VERSION"))) return null
  const exe = platform === "win32" ? "ai-gent.exe" : "ai-gent"
  const local = join(root, "bin", exe)
  return { bin: existsSync(local) ? local : exe }
}

export function failure(why) {
  return {
    decision: "ask",
    reason: `${MARKER} the ai-gent binary is unavailable (${why}); run: ${INSTALL_COMMAND}. Allow only if you accept this command unguarded.`,
  }
}

export function decideWithBinary(event, { cwd, bin, timeoutMs = TIMEOUT_MS }) {
  const input = JSON.stringify({
    action: event.action,
    resources: Array.isArray(event.resources) ? event.resources : [],
    cwd,
  })
  let out
  try {
    out = execFileSync(bin, ["guard", "--guard", "all", "--format", "opencode"], {
      input,
      encoding: "utf8",
      timeout: timeoutMs,
      killSignal: "SIGKILL",
      stdio: ["pipe", "pipe", "ignore"],
      windowsHide: true,
    })
  } catch (e) {
    if (e?.code === "ENOENT") return failure("not found")
    if (e?.code === "EACCES") return failure("not executable")
    if (e?.code === "ETIMEDOUT" || e?.killed) return failure("guard timed out")
    if (typeof e?.status === "number") return failure(`guard failed, exit ${e.status}`)
    return failure("guard failed")
  }
  let answer
  try {
    answer = JSON.parse(out)
  } catch {
    return failure("guard failed, unreadable answer")
  }
  if (answer?.decision === "none") return { decision: "none" }
  if ((answer?.decision === "deny" || answer?.decision === "ask") && typeof answer.reason === "string") {
    return { decision: answer.decision, reason: answer.reason }
  }
  return failure("guard failed, unknown answer")
}
