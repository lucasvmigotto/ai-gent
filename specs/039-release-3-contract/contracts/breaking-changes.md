# Contract: what 3.0.0 adds and removes, and what breaks

Status of each row is a promise for the 3.0.0 release notes. "Ships in" says the release in which the change lands; B-rows marked 2.28 are announced in 2.28 and enforced in 3.0.0 unless stated.

## Added

| # | Item | Ships in | Spec |
|---|---|---|---|
| A1 | The `ai-gent` binary on six targets, installed with the package | 2.27 | 032 |
| A2 | `ai-gent guard` deciding for every agent (sh, PowerShell, cmd syntax) | 2.28 | 033 |
| A3 | `ai-gent update` and `ai-gent doctor` | 2.28 | 034 |
| A4 | The ported tools as subcommands (per 035-038 status at the gate) | 2.28 / 3.0.0 | 035-038 |
| A5 | Native Windows: the binary, the guards and the `install.ps1` installer (D24); WSL or Git Bash with `install.sh` stays an alternative | 3.0.0 | 032, 033 |
| A6 | Tier policy and compatibility matrix | 3.0.0 | this spec, 040 |
| A8 | Release notes carry the SHA256 of `install.sh`, `install.ps1` and the packages (D25) | 2.27 | 032 |
| A7 | Published crates `ai-gent` and `ai-gent-core` (and `ai-gent-dbrun` once it exists, D28) (real versions, Trusted Publishing) when the owner flips the switch | 3.0.0 or later | 032 |

## Removed or changed (breaking)

| # | Change | Who is affected | What they see | Migration | Ships in | Test |
|---|---|---|---|---|---|---|
| B1 | Git install mode removed: `AI_GENT_MODE=git`, `AI_GENT_BRANCH`, `AI_GENT_REPO`, `AI_GENT_DIR` no longer read | users and scripts of the dev mode | `install.sh` stops: "git mode was removed in 3.0.0" + steps | move the clone aside, run the installer, `current/setup.sh --migrate`; contributors use a checkout (B3) | 3.0.0 | `test-migrate.sh` fixtures `clone`, `env-git-mode` |
| B2 | Package is the only install: agent links made by the installer always lead into `<home>/current/`; no live edit in place | everyone who edited files under the install root | edits are lost on update; hint to use a checkout | fork or checkout + `setup.sh` | 3.0.0 | `test-package.sh` |
| B3 | Contributors work in a checkout and run `setup.sh` from it; `scripts/dev-bin.sh` provides the binary | contributors | n/a (documented) | README Contributing | 2.28 (script), 3.0.0 | `test-install.sh` new case |
| B4 | A missing binary fails with the install command; there is no Python or shell fallback (D8) | anyone with content but no binary (a half install, a clone without `dev-bin.sh`) | guards ask (033 FR-014); tools print the install command, exit 1 | install the package | 2.28 | 033 failure tests |
| B5 | Shell guard logic removed; `plugins/*/hooks/guard.sh` is a launcher (package installs use the binary from 2.28; clones keep the shell logic until 3.0.0) | people who read or patched `guard.sh` | nothing visible; decisions identical (parity gate) | none | 2.28 (binary), 3.0.0 (shell logic deleted) | 033 parity |
| B6 | `opencode/guard/rules.mjs` and its parity test removed; the opencode plugin calls the binary (D13) | opencode users | none if the binary is installed; else ask | install the package | 2.28 (binary from a package), 3.0.0 (rules.mjs deleted) | 033 US4 |
| B7 | Python and shell tools replaced by subcommands are deleted together at 3.0.0 (D7 amended): the union of 035-037's tools (candidates: `wt.py`, `token_report.py`, `reporun.py`, `specsync.py`, `compact.py`); skills switch to `ai-gent <sub>` in the same release. `dbrun.py` is not in the set (D26/D27) | anyone calling those files directly or documenting them | "No such file" | map old call to `ai-gent <sub>` (table in the migration guide; `--json` shapes are kept stable or versioned by `schema`) | 3.0.0 (subcommands ship beside the old tools from 2.27/2.28) | per-tool parity gates (D10) proven in the package before 3.0.0 |
| B8 | `scripts/update-check.sh` and `scripts/ai-gent-update.sh` removed in 3.0.0 (the binary takes over from 2.28 on package installs) | people running them by path | file not found | `ai-gent update` | 2.28 / 3.0.0 | 034 |
| B9 | Content-only asset `ai-gent-<ver>.tar.gz` no longer published; assets are per target | old installers (2.26-2.27) updating straight to 3.0.0; automation fetching the old name | download error, nothing written | go through 2.28 or run the current installer | 3.0.0 | `test-migrate.sh` `pkg-2.26` |
| B10 | Tier-3 agents are best-effort and frozen, outside required CI (D2); nothing removed | users of opencode, Cursor, Antigravity, Copilot, Amp, Kiro, Windsurf | a "best-effort" label in docs | none | 3.0.0 | CI job split |
| B11 | `uv`/Python no longer needed except for the db plugin (`dbrun`, D26/D27); the README states it | people who installed uv for ai-gent | none | optional cleanup | 3.0.0 | doctor `tools`; G30-16 |
| B12 | The `--version` output format now equals the package version | scripts parsing it | `ai-gent 3.0.0 (<triple>)` | use `version --json` | 2.27 | 032 cli tests |

## What does not change

Skill names and invocations (`/plugin:skill`, `plugin-skill` stubs), plugin versioning, the product pipeline artifacts, `shared/pipeline.md`, `shared/secrets.md` rules, pass entry names, the db plugin's interface, the release flow and tag format.
