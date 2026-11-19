// Checks the rendered `qa:uat` page (spec 055, T016 and T021). Not part of CI (Decision D8).
//
//   scripts/uat-page-check.mjs [page.html]
//
// Without an argument it renders rust/fixtures/uat/clean.json with `ai-gent uat render` (bin/ai-gent, else
// ai-gent on PATH). jsdom comes from docs/site/node_modules (`cd docs/site && bun install`).
//
// What it checks, and what it cannot:
//   1. Roster and role proposals (T016): the page against a fake Artifact runtime, owner and non-owner.
//   2. Accessibility (T021): the contrast of every color token pair in both themes, computed from the page's
//      own CSS (WCAG 2.2 AA, 4.5:1); the CSS rules that keep a 375 px screen from scrolling sideways; and the
//      structure axe would look at (names, labels, ids, headings, roles, aria-pressed) on the page with data.
//   Not checked here: axe-core itself and real rendering at 375 and 1280 px. Neither runs under jsdom on Bun
//   (jsdom cannot execute scripts there), and this machine's Chromium lacks a system library. Run axe in a
//   browser on the rendered page (the pilot, T050) before the first real session.
//
// jsdom under Bun cannot report script errors or click natively, so the page script is evaluated outside
// the parser and events are dispatched by hand.
import { execFileSync } from "node:child_process"
import { createRequire } from "node:module"
import { existsSync, readFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const require = createRequire(join(root, "docs/site/package.json"))
const { JSDOM } = require("jsdom")

function pageHtml() {
  if (process.argv[2]) return readFileSync(process.argv[2], "utf8")
  const bin = existsSync(join(root, "bin/ai-gent")) ? join(root, "bin/ai-gent") : "ai-gent"
  return execFileSync(bin, ["uat", "render", join(root, "rust/fixtures/uat/clean.json")], { encoding: "utf8" })
}
const html = pageHtml()
const script = html.slice(html.indexOf("<script>\n") + 9, html.lastIndexOf("</script>"))
// jsdom under Bun runs the JSON data island as JavaScript and dies reporting the error, so the harness
// turns that one element into a div with the same id and text.
const htmlWithoutScript = html
  .replace(script, "")
  .replace(/<script>\s*<\/script>/, "")
  .replace(/<script type="application\/json" id="uat-data">([\s\S]*?)<\/script>/, (_, json) => `<div id="uat-data">${json}</div>`)

let fails = 0
const ok = (cond, msg) => {
  if (!cond) fails++
  console.log(`${cond ? "ok  " : "FAIL"} ${msg}`)
}
const wait = (ms) => new Promise((r) => setTimeout(r, ms))

// ---- a fake Artifact runtime ----------------------------------------------------------------------------
function runtime({ uid = "u1", owner = true, canWrite = true, reject = null } = {}) {
  const cols = {}
  const subs = {}
  const writes = []
  const emit = (name) => {
    const docs = Object.entries(cols[name] || {}).map(([id, d]) => ({ id, exists: true, data: () => d }))
    ;(subs[name] ||= []).forEach((f) => f({ docs, size: docs.length, empty: !docs.length }))
  }
  const db = {
    collection: (name) => ({
      onSnapshot: (next) => {
        ;(subs[name] ||= []).push(next)
        setTimeout(() => emit(name), 0)
        return () => {}
      },
    }),
    doc: (path) => ({
      set: async (data) => {
        writes.push([path, JSON.parse(JSON.stringify(data))])
        if (reject) throw { code: reject, message: "x" }
        const [c, id] = path.split("/")
        ;(cols[c] ||= {})[id] = data
        emit(c)
      },
    }),
  }
  const names = { u1: "Ana", u2: "Bia", u9: "Zed" }
  const user = {
    id: async () => uid,
    isOwner: async () => owner,
    can: async () => canWrite,
    profiles: async (ids) => Object.fromEntries(ids.map((i) => [i, { name: names[i] || "" }])),
  }
  const claude = { use: async (n) => (n === "db" ? db : n === "user" ? user : null) }
  return { claude, cols, writes }
}

async function open(rt, theme) {
  const dom = new JSDOM(htmlWithoutScript, { runScripts: "dangerously", pretendToBeVisual: true })
  if (theme) dom.window.document.documentElement.setAttribute("data-theme", theme)
  if (rt) dom.window.claude = rt.claude
  dom.window.eval(script)
  await wait(50)
  return dom
}
const $ = (d, id) => d.window.document.getElementById(id)
const fire = (d, el, type = "click") => el.dispatchEvent(new d.window.MouseEvent(type, { bubbles: true }))
const typeInto = (d, el, value) => {
  el.value = value
  el.dispatchEvent(new d.window.Event("input", { bubbles: true }))
}
const byText = (d, root, text) => [...root.querySelectorAll("button")].find((b) => b.textContent === text)

// ---- 1. roster and role proposals (T016) ----------------------------------------------------------------
console.log("\n# roster and role proposals")
{
  // A validator proposes another role in R0: a proposal is written, the roster is not.
  const rt = runtime({ uid: "u2", owner: false })
  rt.cols.roster = { u2: { validator: "v2", role: "clerk" } }
  const d = await open(rt)
  ok($(d, "prop-role") && $(d, "prop-note") && $(d, "prop-save"), "R0 role-confirmation step carries the proposal controls")
  ok([...$(d, "prop-role").options].map((o) => o.textContent).join("|") === "Keep my role|Administrator|Clerk", "the proposal select lists the roles by label")
  typeInto(d, $(d, "prop-note"), "I also approve orders.")
  $(d, "prop-role").value = "admin"
  fire(d, $(d, "prop-save"))
  await wait(30)
  const w = rt.writes.find(([p]) => p === "proposals/u2")
  ok(w && w[1].role === "admin" && w[1].decision === "pending" && w[1].note === "I also approve orders.", "a proposal is written as pending, with its note")
  ok(!rt.writes.some(([p]) => p.startsWith("roster/")), "proposing writes no roster entry")
  ok(!$(d, "roster-body").textContent.includes("I also approve"), "a non-owner does not see the proposal note")
  ok(/proposal pending/.test($(d, "roster-body").textContent), "a non-owner sees that a proposal is pending")
}
{
  // The owner assigns, approves and rejects.
  const rt = runtime()
  rt.cols.marks = {
    u2: { steps: { "r1-1": { r: "OK", at: "t", rev: 1 } }, improvements: {} },
    u9: { steps: { "r1-1": { r: "OK", at: "t", rev: 1 } }, improvements: {} },
  }
  rt.cols.roster = { u2: { validator: "v2", role: "clerk" } }
  rt.cols.proposals = {
    u2: { role: "admin", note: "wants admin", decision: "pending", at: "t" },
    u9: { role: "clerk", note: "n", decision: "pending", at: "t" },
  }
  const d = await open(rt)
  await wait(60)
  const body = $(d, "roster-body")
  ok(/wants admin/.test(body.textContent), "the owner sees the pending note")
  ok(/Zed/.test(body.textContent) && /role not assigned/.test(body.textContent), "an unlisted account shows under 'role not assigned'")
  const cov = $(d, "results-body").textContent
  ok(/Clerk: 1 steps covered/.test(cov), "coverage counts the clerk with a roster entry")
  ok(!/Administrator: [1-9]/.test(cov), "a pending proposal changes no coverage")
  const sel = $(d, "assign-u9")
  sel.value = "clerk"
  sel.dispatchEvent(new d.window.Event("change", { bubbles: true }))
  await wait(30)
  ok(rt.cols.roster.u9 && rt.cols.roster.u9.role === "clerk", "the owner assigns a role to an unlisted account")
  const approve = byText(d, $(d, "roster-body"), "Approve")
  fire(d, approve)
  await wait(30)
  ok(rt.cols.roster.u2.role === "admin" && rt.cols.proposals.u2.decision === "approved", "approve writes the roster and the decision")
  ok(/Administrator: [1-9]|Administrator: 0/.test($(d, "results-body").textContent), "coverage is recomputed after the approval")
  const reject = byText(d, $(d, "roster-body"), "Reject")
  if (reject) {
    fire(d, reject)
    await wait(30)
  }
  ok(rt.cols.proposals.u9.decision === "rejected", "reject records the decision")
  ok(rt.cols.roster.u9.role === "clerk" && rt.cols.roster.u2.role === "admin", "a rejection changes no roster entry")
}
{
  // Coverage comes from the roster only: a mark by an account with no roster entry covers nothing.
  const rt = runtime({ owner: false })
  rt.cols.marks = { u9: { steps: { "r1-1": { r: "OK", at: "t", rev: 1 } }, improvements: {} } }
  const d = await open(rt)
  await wait(60)
  ok(/Not covered by the target role: r1-1/.test($(d, "results-body").textContent), "a mark from an unlisted account leaves the step uncovered")
}

// ---- 2. accessibility (T021) -----------------------------------------------------------------------------
console.log("\n# accessibility")
const full = (h) => (h.length === 4 ? `#${h[1]}${h[1]}${h[2]}${h[2]}${h[3]}${h[3]}` : h)
const tokens = (block) => Object.fromEntries([...block.matchAll(/--([a-z-]+):\s*(#[0-9a-fA-F]{3,6})\b/g)].map((m) => [m[1], full(m[2])]))
const css = html.slice(html.indexOf("<style>"), html.indexOf("</style>"))
const lightBlock = css.match(/:root\{([^}]*)\}/)[1]
const darkBlock = css.match(/:root\[data-theme="dark"\]\{([^}]*)\}/)[1]
const light = tokens(lightBlock)
const dark = { ...light, ...tokens(darkBlock) }
const lum = (hex) => {
  const c = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4))
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}
const ratio = (a, b) => {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p)
  return (x + 0.05) / (y + 0.05)
}
const pairs = [
  ["fg", "bg"], ["fg", "card"], ["muted", "bg"], ["muted", "card"], ["accent", "bg"], ["accent", "card"],
  ["ok-fg", "ok-bg"], ["nok-fg", "nok-bg"], ["stale-fg", "stale-bg"], ["imp-fg", "imp-bg"], ["pend-fg", "pend-bg"],
]
for (const [name, theme] of [["light", light], ["dark", dark]]) {
  const worst = pairs.map(([f, b]) => [f, b, ratio(theme[f], theme[b])]).sort((p, q) => p[2] - q[2])[0]
  ok(worst[2] >= 4.5, `${name} theme: every text pair reaches 4.5:1 (lowest ${worst[0]} on ${worst[1]} = ${worst[2].toFixed(2)})`)
}
ok(/@media \(prefers-color-scheme:dark\)\{:root:not\(\[data-theme="light"\]\)/.test(css) && /:root\[data-theme="dark"\]/.test(css), "dark values exist for the media query and the explicit data-theme")
ok(/body\{margin:0;background:var\(--bg\)/.test(css), "body has an explicit background")
ok(/main\{[^}]*padding:16px/.test(css), "16 px side gutter")
ok(/\.scroll\{overflow-x:auto/.test(css), "tables scroll inside their own container")
ok(/table\{[^}]*min-width:560px/.test(css) && html.split('class="scroll"').length > 1, "wide tables sit inside .scroll, not on the page")
ok(!/(^|[;{])width:\s*\d+px/.test(css.replace(/min-width:\d+px/g, "")), "no fixed pixel width outside the tables' min-width")
ok(/button\{[^}]*min-height:44px/.test(css), "controls are at least 44 px high")
ok(/:focus-visible\{outline:3px/.test(css), "focus is visible")
ok(/prefers-reduced-motion:reduce/.test(css), "reduced motion is respected")
ok(!/(src|href)=["']https?:/.test(html.replace(/<a [^>]*>/g, "")) && !/<script[^>]+src=/.test(html), "no external scripts or assets")

for (const [label, rt] of [["signed out", null], ["owner with data", (() => {
  const r = runtime()
  r.cols.marks = { u2: { steps: { "r1-1": { r: "NOK", obs: "Blank screen", at: "2026-11-20T10:00:00Z", rev: 1, links: ["https://x.example/a"] } }, improvements: { "r1-1": { txt: "Show a message", at: "t", rev: 1 } } } }
  r.cols.roster = { u2: { validator: "v2", role: "clerk" } }
  r.cols.proposals = { u2: { role: "admin", note: "n", decision: "pending", at: "t" } }
  return r
})()]]) {
  const d = await open(rt, "dark")
  await wait(60)
  const doc = d.window.document
  const all = [...doc.querySelectorAll("[id]")].map((e) => e.id)
  ok(new Set(all).size === all.length, `${label}: every id is unique`)
  const name = (e) => (e.getAttribute("aria-label") || e.textContent || "").trim()
  ok([...doc.querySelectorAll("button")].every((b) => name(b).length > 0), `${label}: every button has a name`)
  ok([...doc.querySelectorAll("input, textarea, select")].every((e) => e.labels && e.labels.length > 0 || e.getAttribute("aria-label")), `${label}: every field has a label`)
  ok($(d, "status").getAttribute("role") === "status" && $(d, "notice").getAttribute("role") === "alert", `${label}: status and error lines are live regions`)
  const pressed = [...doc.querySelectorAll("button[aria-pressed]")]
  ok(pressed.length > 0 && pressed.every((b) => ["true", "false"].includes(b.getAttribute("aria-pressed"))), `${label}: OK and NOK are toggle buttons with aria-pressed`)
  const levels = [...doc.querySelectorAll("h1, h2, h3")].map((h) => Number(h.tagName[1]))
  ok(levels[0] === 1 && levels.every((l, i) => i === 0 || l - levels[i - 1] <= 1), `${label}: headings never skip a level`)
  ok([...doc.querySelectorAll("table")].every((t) => t.querySelectorAll("thead th").length > 0), `${label}: every table has header cells`)
  ok([...doc.querySelectorAll("a[target=_blank]")].every((a) => /noopener/.test(a.rel)), `${label}: links that open a new tab carry rel=noopener`)
  ok([...doc.querySelectorAll("a[href]")].every((a) => !/^javascript:/i.test(a.getAttribute("href"))), `${label}: no javascript: links`)
  ok([...doc.querySelectorAll("nav")].every((n) => n.getAttribute("aria-label")), `${label}: navigation is labelled`)
  const tab = [...doc.querySelectorAll("[tabindex]")].filter((e) => Number(e.getAttribute("tabindex")) > 0)
  ok(tab.length === 0, `${label}: no positive tabindex`)
}

console.log(fails ? `\n${fails} FAILED` : "\nALL OK")
process.exit(fails ? 1 : 0)
