import { execFileSync } from "node:child_process"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { gitGuard, dbGuard, dbCredsShell, dbCredsTool, secretsGuard, repoGuard, evaluate } from "../rules.mjs"

// Parity test: the OpenCode rules must decide exactly what the Claude Code
// guards (plugins/{git,db,secrets,repo}/hooks/guard.sh) decide for
// the same vectors. guard.sh is the oracle; the shell test suites and this
// one share the same expectations, so the two implementations cannot drift.

const REPO = new URL("../../..", import.meta.url).pathname.replace(/\/$/, "")
const GIT_GUARD = join(REPO, "plugins/git/hooks/guard.sh")
const DB_GUARD = join(REPO, "plugins/db/hooks/guard.sh")
const SECRETS_GUARD = join(REPO, "plugins/secrets/hooks/guard.sh")
const REPO_GUARD = join(REPO, "plugins/repo/hooks/guard.sh")

const sandbox = mkdtempSync(join(tmpdir(), "ai-gent-guard-"))
process.on("exit", () => rmSync(sandbox, { recursive: true, force: true }))

const dirs = {}
for (const b of ["main", "staging", "dev", "feat/x"]) {
  const d = join(sandbox, b.replace("/", "-"))
  execFileSync("git", ["init", "-q", "-b", b, d])
  dirs[b] = d
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

// ---------------------------------------------------------------- git

const gitVectors = [
  ["deny", feat, 'git commit --no-verify -m "fix: x"'],
  ["deny", feat, 'git commit -n -m "fix: x"'],
  ["deny", feat, "git -C /tmp/repo merge --no-verify feat/x"],
  ["deny", feat, "git commit -m \"$(cat <<'EOF'\nfeat: x\n\nCo-Authored-By: Someone <a@b.c>\nEOF\n)\""],
  ["deny", feat, "git push --force origin feat/x"],
  ["deny", feat, "git push -f"],
  ["ask", feat, "git push --force-with-lease origin feat/x"],
  ["ask", feat, "git push origin feat/x"],
  ["ask", feat, "cd /tmp && git push"],
  ["ask", feat, "git fetch && git push origin --delete feat/old"],
  ["ask", feat, "gh pr create --fill"],
  ["ask", feat, "git branch -D feat/old"],
  ["ask", feat, "git reset --hard HEAD~1"],
  ["ask", feat, "git clean -fd"],
  ["ask", feat, "git commit --amend --no-edit"],
  ["ask", feat, "git filter-repo --mailmap m"],
  ["ask", main, 'git commit -m "fix: x"'],
  ["ask", main, "git merge --ff-only feat/x"],
  ["ask", feat, `git -C ${main} commit -m "fix: x"`],
  ["ask", staging, "git merge --no-ff dev"],
  ["ask", staging, 'git commit -m "fix: x"'],
  ["ask", dev, 'git commit -m "fix: x"'],
  ["ask", dev, 'git merge --no-ff feat/x && git commit -m "fix: x"'],
  ["ask", feat, `git -C ${dev} commit -m "fix: x"`],
  ["none", dev, "git merge --no-ff feat/x"],
  ["none", dev, "git merge --ff-only feat/x"],
  ["none", dev, "git merge --continue"],
  ["none", feat, 'git commit -m "fix: x"'],
  ["none", feat, 'git commit -m "feat(git): block --no-verify and -n in the guard"'],
  ["none", feat, 'git commit -m "docs: explain git push --force"'],
  ["none", feat, "git merge --no-ff feat/x-db"],
  ["none", feat, "git branch -d feat/x-db"],
  ["none", feat, "git status && git log --oneline -5"],
  ["none", feat, 'echo "git push" > notes.txt'],
  ["none", feat, "grep -rn -- --no-verify docs/"],
  ["none", feat, "git push-notes-helper"],
  ["none", main, "git switch -c feat/new"],
  ["none", feat, "ls -la"],
]

console.log("    git: TypeScript rules vs plugins/git/hooks/guard.sh")
for (const [want, cwd, cmd] of gitVectors) {
  const ocDecision = gitGuard(cmd, { cwd, branchOf }).decision
  check(cmd.replace(/\n/g, "⏎"), want, ocDecision)
  check(`shell ${cmd.replace(/\n/g, "⏎")}`, want, shellDecision(GIT_GUARD, "Bash", cwd, { command: cmd, description: "x" }))
}

// ----------------------------------------------------------------- db

const dbVectors = [
  ["deny", 'psql -h db.internal -U app -c "DELETE FROM orders WHERE id = 1"'],
  ["deny", "mysql -u root shop -e 'update customer set status = 1'"],
  ["deny", 'docker exec -i shop-db-1 psql -U app -c "DROP TABLE t"'],
  ["deny", "podman exec db sqlplus -s system/x@FREEPDB1 <<EOF\nTRUNCATE TABLE audit;\nEOF"],
  ["deny", 'sqlcmd -S prod -Q "ALTER TABLE t ADD c int"'],
  ["deny", "pg_restore -d shop backup.dump"],
  ["deny", 'kubectl exec -it pg-0 -- psql -c "insert into t values (1)"'],
  ["deny", 'PGPASSWORD=x psql -h 10.0.0.5 -c "grant all on t to u"'],
  ["ask", 'psql -h db.internal -U app -c "SELECT id FROM orders LIMIT 5"'],
  ["ask", "sudo -u postgres psql"],
  ["ask", "/usr/lib/postgresql/17/bin/psql -l"],
  ["ask", "docker compose exec db psql -U app -d shop"],
  ["ask", 'sqlite3 app.db ".tables"'],
  ["ask", 'mongosh "mongodb://localhost/test" --eval "db.stats()"'],
  ["ask", 'timeout 10s mysql -e "SELECT 1"'],
  ["none", 'python3 /home/me/ai-gent/plugins/db/scripts/dbrun.py query DEVDB --sql q.sql --reason "check"'],
  ["none", 'echo "psql is installed" && ls'],
  ["none", 'grep -rn "DELETE FROM" src/'],
  ["none", "which psql"],
  ["none", "command -v sqlcmd"],
  ["none", "npm run sql:lint"],
  ["none", "cat queries/report.sql"],
  ["none", 'git commit -m "fix: guard mysql client calls"'],
  ["none", "ls -la"],
  // credentials file, any tool
  ["deny", 'cat ~/.config/ai-gent/db/connections.env'],
  ["deny", 'grep -i password "$HOME/.config/ai-gent/db/connections.env"'],
  ["deny", "ls ${XDG_CONFIG_HOME}/ai-gent/db"],
  ["deny", "echo $AI_GENT_DB_CONFIG"],
  ["deny", 'python3 /x/plugins/db/scripts/dbrun.py profiles && cat connections.env'],
  ["deny", 'python3 /x/plugins/db/scripts/dbrun.py profiles; psql -c "DELETE FROM orders"'],
  ["ask", 'python3 /x/plugins/db/scripts/dbrun.py profiles && psql -l'],
  ["none", "grep -E \"^FAIL|passed\" out.log; git commit -m \"fix: a tool's AI_GENT_DB_CONNECTIONS note, and a Glob's path\""],
  ["deny", "cat ~/.config/ai-gent/db/connections.toml"],
  ["deny", "cp connections.toml /tmp/x"],
  ["deny", "echo $AI_GENT_DB_CONNECTIONS"],
  ["none", "cat connections.env.example"],
  ["none", "git commit -m \"docs: explain the connections.env format\""],
  ["none", "echo 'profiles live in connections.env' > notes.md"],
]

console.log("    db: TypeScript rules vs plugins/db/hooks/guard.sh")
for (const [want, cmd] of dbVectors) {
  const creds = dbCredsShell(cmd)
  const ocDecision = creds.decision !== "none" ? creds.decision : dbGuard(cmd).decision
  check(cmd.replace(/\n/g, "⏎"), want, ocDecision)
  check(`shell ${cmd.replace(/\n/g, "⏎")}`, want, shellDecision(DB_GUARD, "Bash", "/tmp", { command: cmd }))
}

const toolVectors = [
  ["deny", "Read", { file_path: "/home/me/.config/ai-gent/db/connections.env" }],
  ["deny", "Grep", { pattern: "PASSWORD", path: "/home/me/.config/ai-gent" }],
  ["deny", "Glob", { pattern: "**/connections.env" }],
  ["deny", "Edit", { file_path: "/home/me/.config/ai-gent/db/connections.env", old_string: "a", new_string: "b" }],
  ["none", "Read", { file_path: "/home/me/project/.env.example" }],
  ["none", "Grep", { pattern: "DATABASE_URL", path: "src/" }],
  ["none", "Write", {"file_path": "docs/product/brief.md", "content": "Profiles live in ~/.config/ai-gent/db/connections.env"}],
  ["none", "Edit", {"file_path": "src/app.py", "old_string": "a", "new_string": "AI_GENT_DB_CONFIG"}],
  ["deny", "Read", {"file_path": "/home/me/.config/ai-gent/db/connections.toml"}],
  ["none", "Read", {"file_path": "/home/me/project/connections.env.example"}],
  ["none", "Grep", {"pattern": "connections.env", "path": "docs/"}],
  ["deny", "Glob", {"pattern": "**/connections.toml"}],
]
console.log("    db: credentials file, non-shell tools")
for (const [want, tool, input] of toolVectors) {
  const ocDecision = dbCredsTool(tool, input).decision
  check(`${tool} ${JSON.stringify(input)}`, want, ocDecision)
  check(`shell ${tool}`, want, shellDecision(DB_GUARD, tool, "/tmp", input))
}

// ------------------------------------------------------------ secrets

const secretsVectors = [
  ["none", "pass git pull"],
  ["none", "pass git fetch origin"],
  ["none", "pass git status"],
  ["none", "PASSWORD_STORE_DIR=/srv/team pass git log --oneline -5"],
  ["ask", "pass git push"],
  ["ask", "pass git reset --hard origin/main"],
  ["ask", "pass init -p db/team AAAA BBBB"],
  ["deny", "pass show ai-gent/db/shop-dev"],
  ["deny", "pass ai-gent/github/pat"],
  ["deny", "pass -c ai-gent/github/pat"],
  ["deny", "pass show --clip=2 ai-gent/github/pat"],
  ["deny", "pass show -q work/vpn"],
  ["deny", "pass edit ai-gent/azure-devops/pat"],
  ["deny", "pass grep token"],
  ["deny", "pass generate ai-gent/x/y 32"],
  ["deny", "pass otp ai-gent/x/totp"],
  ["deny", "PASSWORD_STORE_DIR=/tmp/s pass show x | head -1"],
  ["deny", "echo ok && pass show \"ai-gent/db/shop dev\""],
  ["deny", "sudo -u me pass show x"],
  ["deny", "gpg -d ~/.password-store/ai-gent/github/pat.gpg"],
  ["deny", "cat \"$PASSWORD_STORE_DIR/x.gpg\" | gpg --decrypt"],
  ["deny", "pass insert ai-gent/db/shop-dev"],
  ["deny", "printf \"%s\\n\" \"$TOKEN\" | pass insert -m ai-gent/github/pat"],
  ["ask", "pass rm ai-gent/db/old"],
  ["ask", "pass mv ai-gent/db/a ai-gent/db/b"],
  ["ask", "pass init 0xDEADBEEF"],
  ["deny", "pass-run PGPASSWORD=ai-gent/db/shop-dev -- psql -h db -l"],
  ["deny", "pass-run GH_TOKEN=ai-gent/github/pat -- env"],
  ["deny", "pass-run GH_TOKEN=ai-gent/github/pat -- sh -c \"echo $GH_TOKEN\""],
  ["deny", "pass-run X=ai-gent/github/pat -- /usr/bin/printenv X"],
  ["none", "pass-run GH_TOKEN=ai-gent/github/pat -- gh pr list"],
  ["none", "pass-run GH_TOKEN=ai-gent/github/pat -- gh api -- repos/x/y"],
  ["none", "pass"],
  ["none", "pass ls"],
  ["none", "pass ls ai-gent"],
  ["none", "pass find github"],
  ["none", "pass --version"],
  ["none", "command -v pass"],
  ["none", "which pass gpg"],
  ["none", "git commit -m \"docs: pass show prints the secret\""],
  ["none", "echo \"use pass show\" > notes.txt"],
  ["none", "grep -rn \"pass show\" docs/"],
  ["none", "gpg --list-secret-keys"],
  ["none", "python3 /x/plugins/db/scripts/dbrun.py test SHOP_DEV --reason check"],
  ["none", "bypass --flag"],
  ["none", "ls -la"],
]

console.log("    secrets: TypeScript rules vs plugins/secrets/hooks/guard.sh")
for (const [want, cmd] of secretsVectors) {
  check(cmd.replace(/\n/g, "⏎"), want, secretsGuard(cmd).decision)
  check(`shell ${cmd.replace(/\n/g, "⏎")}`, want, shellDecision(SECRETS_GUARD, "Bash", "/tmp", { command: cmd }))
}

// --------------------------------------------------------------- repo

const repoVectors = [
  ["ask", "python3 /home/me/ai-gent/plugins/repo/scripts/reporun.py cli github --write -- pr merge 3 --squash"],
  ["ask", "python3 /home/me/ai-gent/plugins/repo/scripts/reporun.py api github POST /repos/o/r/issues --data body.json --write"],
  ["ask", "python3 /home/me/ai-gent/plugins/repo/scripts/reporun.py cli azure-devops --write -- boards work-item update --id 12 --state Done"],
  ["ask", "cd /x && python3 /home/me/ai-gent/plugins/repo/scripts/reporun.py api jira PUT /rest/api/3/issue/X-1 --data - --write < f.json"],
  ["ask", "reporun cli gitlab --write -- mr note 4 -m ok"],
  ["none", "python3 /home/me/ai-gent/plugins/repo/scripts/reporun.py detect"],
  ["none", "python3 /home/me/ai-gent/plugins/repo/scripts/reporun.py cli github -- pr list"],
  ["none", "python3 /home/me/ai-gent/plugins/repo/scripts/reporun.py api github GET /repos/o/r/issues --paginate"],
  ["none", "python3 /home/me/ai-gent/plugins/repo/scripts/reporun.py api github GET /search/issues --query 'q=--write'"],
  ["none", "git commit -m \"docs: reporun --write asks first\""],
  ["none", "echo reporun --write"],
  ["none", "ls -la"],
]

console.log("    repo: TypeScript rules vs plugins/repo/hooks/guard.sh")
for (const [want, cmd] of repoVectors) {
  check(cmd, want, repoGuard(cmd).decision)
  check(`shell ${cmd}`, want, shellDecision(REPO_GUARD, "Bash", "/tmp", { command: cmd }))
}

// ------------------------------------------------------------- adapter

console.log("    evaluate(): OpenCode permission event adapter")
const adapterCases = [
  [{ action: "shell", resources: ["git push origin feat/x"] }, "ask"],
  [{ action: "shell", resources: ["git commit --no-verify -m x"] }, "deny"],
  [{ action: "shell", resources: ["psql -c 'DELETE FROM t'"] }, "deny"],
  [{ action: "shell", resources: ["ls -la"] }, "none"],
  [{ action: "shell", resources: ["pass show ai-gent/github/pat"] }, "deny"],
  [{ action: "shell", resources: ["pass rm ai-gent/db/old"] }, "ask"],
  [{ action: "shell", resources: ["reporun cli github --write -- pr merge 3"] }, "ask"],
  [{ action: "read", resources: ["/home/me/.config/ai-gent/db/connections.env"] }, "deny"],
  [{ action: "read", resources: ["/home/me/project/readme.md"] }, "none"],
  [{ action: "edit", resources: ["/home/me/.config/ai-gent/db/connections.env"] }, "deny"],
]
for (const [event, want] of adapterCases) {
  check(JSON.stringify(event), want, evaluate(event, { cwd: feat, branchOf }).decision)
}

console.log(`\n    ${count} checks, ${failures} failed`)
process.exit(failures ? 1 : 0)
