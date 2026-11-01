# Data model: the 3.0.0 contract

| Entity | Where | Fields |
|---|---|---|
| Gate | `contracts/release-gates.md` | `id` (G27-n, G28-n, G30-n), `criterion`, `measure` (command, file or CI run), `evidence` (link or pasted output, empty until met), `owner` (builder or owner), `waiver` (owner-signed reason, optional) |
| Breaking change | `contracts/breaking-changes.md` | `id` (B1-B10), `what changes`, `who is affected`, `what they see`, `migration`, `ships in` (2.28 or 3.0.0), `test` |
| Tier | `contracts/tier-policy.md` | `tier` (1-3), `agents`, `promise`, `CI`, `feature table` |
| Migration path | `contracts/migration-guide.md` | `from` (state), `to`, `steps`, `verifies with` (`ai-gent doctor` check ids), `fixture` in `scripts/test-migrate.sh` |
| Doc task | `tasks.md` Phase 3 | `file`, `section`, `change`, `check` (grep or script) |

## Fixture homes for `scripts/test-migrate.sh`

| Fixture | Built from | Expected after upgrading to the candidate |
|---|---|---|
| `pkg-2.26` | `git archive 2.26.0` + `package.py` at that tag, installed by that tag's `install.sh` | updater refuses with the guidance; reinstall with the new installer works, links in `current/` |
| `pkg-2.28` | candidate 2.28.x package | `ai-gent update` gives 3.0.0; rollback gives 2.28.x |
| `clone` | a git clone at the root, links into it | installer refuses; after move-aside, `setup.sh --migrate` relinks; old clone untouched |
| `mixed` | links into both a clone and a package | `--migrate` reports and repoints the clone links |
| `env-git-mode` | environment with `AI_GENT_MODE=git` | installer stops with the removal message |
| `pinned` | install with `pinned` file | `--check` silent; `update --to latest` clears it |
