// ai-gent guard — the OpenCode V2 side of the git and db guards.
//
// OpenCode does not run Claude Code's PreToolUse hooks, so this plugin
// enforces the same rules through the V2 permission API: for every shell,
// read or edit permission evaluation it ports the decision from
// plugins/git/hooks/guard.sh and plugins/db/hooks/guard.sh (via ./rules.mjs,
// kept in parity with them by test/rules.test.mjs) and can raise `ask` or
// `deny`.
//
// Configured permission rules in opencode.json remain the coarse, always-on
// first line; this plugin adds what those patterns cannot express, such as
// whether the current branch is main/master.
//
// If the plugin fails to load, OpenCode simply falls back to the configured
// rules — nothing here can widen access.
//
// The definition is exported as a plain object on purpose: a config-listed
// plugin directory cannot resolve the bare "@opencode/plugin" specifier that
// Plugin.define comes from, and the loader only needs a default object with an
// id and a setup (or effect) function.

import { execFileSync } from "node:child_process"
import { evaluate } from "./rules.mjs"

function branchOf(dir: string): string {
  try {
    return execFileSync("git", ["-C", dir, "symbolic-ref", "--quiet", "--short", "HEAD"], {
      encoding: "utf8",
    }).trim()
  } catch {
    return ""
  }
}

export default {
  id: "ai-gent.guard",
  async setup(ctx: any) {
    await ctx.permission.hook("evaluate", async (event: any) => {
      if (event.action !== "shell" && event.action !== "read" && event.action !== "edit") return

      let cwd = ctx.location.directory
      try {
        const session = await ctx.session.get({ sessionID: event.sessionID })
        if (session?.location?.directory) cwd = session.location.directory
      } catch {
        // Fall back to the plugin's own location.
      }

      const result = evaluate(event, { cwd, branchOf })
      if (result.decision === "none") return
      event.effect = result.decision
      event.message = result.reason
    })
  },
}
