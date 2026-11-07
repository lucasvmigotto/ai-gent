// Adapter test for the OpenCode guard's binary path (spec 033 T060): a fake
// `ai-gent` stands in for the real one. Run with `node` or `bun`.

import assert from "node:assert/strict"
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { decideWithBinary, INSTALL_COMMAND, packageInstall } from "../binary.mjs"

if (process.platform === "win32") {
  console.log("adapter: skipped on Windows (fake binaries are POSIX sh)")
  process.exit(0)
}

const dir = mkdtempSync(join(tmpdir(), "ai-gent-adapter-"))
let checks = 0
const check = (fn) => {
  fn()
  checks++
}

function fake(name, body) {
  const p = join(dir, name)
  writeFileSync(p, `#!/bin/sh\n${body}\n`)
  chmodSync(p, 0o755)
  return p
}

const event = { action: "shell", resources: ["git push"], sessionID: "s1" }
const opts = (bin, extra = {}) => ({ cwd: "/work/repo", bin, ...extra })

function assertAsksUnavailable(r, why) {
  assert.equal(r.decision, "ask")
  assert.match(r.reason, /^ai-gent guard: the ai-gent binary is unavailable \(/)
  assert.ok(r.reason.includes(why), `${r.reason} should mention ${why}`)
  assert.ok(r.reason.includes(`run: ${INSTALL_COMMAND}.`), r.reason)
  assert.ok(r.reason.endsWith("Allow only if you accept this command unguarded."), r.reason)
}

try {
  // Decisions are mapped, and the call is the documented one.
  const seen = join(dir, "seen")
  const rec = fake(
    "record",
    `echo "$@" > "${seen}.args"; cat > "${seen}.stdin"; printf '{"decision":"deny","reason":"git:workflow guard: no"}'`,
  )
  check(() => {
    assert.deepEqual(decideWithBinary(event, opts(rec)), {
      decision: "deny",
      reason: "git:workflow guard: no",
    })
    assert.equal(readFileSync(`${seen}.args`, "utf8").trim(), "guard --guard all --format opencode")
    assert.deepEqual(JSON.parse(readFileSync(`${seen}.stdin`, "utf8")), {
      action: "shell",
      resources: ["git push"],
      cwd: "/work/repo",
    })
  })
  check(() =>
    assert.deepEqual(
      decideWithBinary(event, opts(fake("ask", `cat >/dev/null; printf '{"decision":"ask","reason":"r"}'`))),
      { decision: "ask", reason: "r" },
    ),
  )
  check(() =>
    assert.deepEqual(
      decideWithBinary(event, opts(fake("allow", `cat >/dev/null; printf '{"decision":"none"}'`))),
      { decision: "none" },
    ),
  )

  // FR-014: every failure is an ask with the install command, never none or deny.
  check(() => assertAsksUnavailable(decideWithBinary(event, opts(join(dir, "absent"))), "not found"))
  const noexec = join(dir, "noexec")
  writeFileSync(noexec, "#!/bin/sh\nexit 0\n")
  chmodSync(noexec, 0o644)
  check(() => assertAsksUnavailable(decideWithBinary(event, opts(noexec)), "not executable"))
  check(() =>
    assertAsksUnavailable(decideWithBinary(event, opts(fake("exit1", "cat >/dev/null; exit 1"))), "exit 1"),
  )
  check(() =>
    assertAsksUnavailable(
      decideWithBinary(event, opts(fake("exit1out", `cat >/dev/null; printf '{"decision":"none"}'; exit 1`))),
      "exit 1",
    ),
  )
  check(() =>
    assertAsksUnavailable(decideWithBinary(event, opts(fake("crash", "kill -SEGV $$"))), "guard failed"),
  )
  check(() => {
    const t0 = Date.now()
    const r = decideWithBinary(event, opts(fake("slow", "sleep 5"), { timeoutMs: 300 }))
    assertAsksUnavailable(r, "timed out")
    assert.ok(Date.now() - t0 < 3000, "the slow binary was cut short")
  })
  check(() =>
    assertAsksUnavailable(
      decideWithBinary(event, opts(fake("empty", "cat >/dev/null"))),
      "unreadable answer",
    ),
  )
  check(() =>
    assertAsksUnavailable(
      decideWithBinary(event, opts(fake("junk", `cat >/dev/null; printf '{"decision":"allow"}'`))),
      "unknown answer",
    ),
  )
  check(() =>
    assertAsksUnavailable(
      decideWithBinary(event, opts(fake("noreason", `cat >/dev/null; printf '{"decision":"deny"}'`))),
      "unknown answer",
    ),
  )

  // Package detection: VERSION marks a package install; a clone has none.
  const root = join(dir, "pkg")
  mkdirSync(join(root, "bin"), { recursive: true })
  check(() => assert.equal(packageInstall(root, "linux"), null))
  writeFileSync(join(root, "VERSION"), "2.28.0\n")
  check(() => assert.deepEqual(packageInstall(root, "linux"), { bin: "ai-gent" }))
  writeFileSync(join(root, "bin", "ai-gent"), "")
  check(() => assert.deepEqual(packageInstall(root, "linux"), { bin: join(root, "bin", "ai-gent") }))
  writeFileSync(join(root, "bin", "ai-gent.exe"), "")
  check(() => assert.deepEqual(packageInstall(root, "win32"), { bin: join(root, "bin", "ai-gent.exe") }))
} finally {
  rmSync(dir, { recursive: true, force: true })
}

console.log(`opencode guard adapter: ${checks} checks passed`)
