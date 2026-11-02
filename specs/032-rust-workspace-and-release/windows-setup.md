# Windows setup: what `setup.sh` links and what `setup.ps1` does (T052, T053, T057)

Status legend: **verified** = covered by `scripts/test-install-ps1.ps1` on the CI Windows legs once green; **unverified** = reasoned from documentation, needs the owner's check on a real machine (T058).

## 1. Inventory of `setup.sh`

For Claude Code and Codex (the only tiers `setup.ps1` covers, FR-022). The remaining agents (opencode, Cursor, Antigravity, Copilot, Amp, Kiro, Windsurf) stay on WSL or Git Bash.

| Agent | Where | What | Source in the tree |
|---|---|---|---|
| Claude Code | `$CLAUDE_SKILLS_DIR` (default `~/.claude/skills`) | one symlink per `skills/<name>/` (has `SKILL.md`) and per `plugins/<name>/` (has `.claude-plugin/plugin.json`), named `<name>` | `skills/*`, `plugins/*` |
| Claude Code | same | symlinks into this tree whose source is gone are removed; `--migrate` repoints links into another ai-gent tree (an old git clone); a link elsewhere or a real file is skipped unless `--force` (real file: backed up as `.bak-<stamp>`) | |
| Codex (and Cursor, Copilot, Amp, which read the same directory) | `$AGENTS_SKILLS_DIR` (default `~/.agents/skills`) | one generated directory per skill: `<name>/SKILL.md` (frontmatter `name`, quoted `description`, `metadata.source`; a body telling the agent to read the source file) plus an empty `.ai-gent-generated` marker; names are `<skill>` for `skills/<skill>` and `<plugin>-<skill>` for `plugins/<plugin>/skills/<skill>`; generated entries, not symlinks, because Cursor skips symlinks | `skills/*/SKILL.md`, `plugins/*/skills/*/SKILL.md` |
| Codex | same | entries whose source is gone are removed; a directory without the marker is never touched; description must be 1-1024 characters | |
| all | `${XDG_STATE_HOME:-~/.local/state}/ai-gent/targets` | the chosen agents, for the next run | |
| binary | `$AI_GENT_BIN_DIR` (default `~/.local/bin`) | symlink `ai-gent` to `<tree>/bin/ai-gent` when the package has the binary; the `PATH` line is printed when the directory is not on `PATH` | `bin/ai-gent` |

Not in this list on purpose: the opencode configuration edit (`scripts/opencode-config.py`) and the opencode entries, because they belong to tiers 3 and above.

## 2. What `setup.ps1` does instead

| Item | Windows form | Status |
|---|---|---|
| Claude Code links | directory **junctions** (`New-Item -ItemType Junction`) `~\.claude\skills\<name>` to `<root>\current\plugins\<name>`; no administrator, no Developer Mode | junction creation and replacement **verified** in CI (which runs as an administrator, so "no elevation needed" is **unverified**, T058) |
| Links resolve through `current` | each junction names `<root>\current\...`, so a flip of `current` moves every skill with it | resolving through a second junction **verified** in CI after rollback; Claude Code reading a skill through two junctions **unverified** |
| Codex entries | the same generated `<name>\SKILL.md` + marker in `~\.agents\skills`, UTF-8 without BOM, LF endings, the source path with backslashes (escaped in the YAML) | file content **verified**; Codex on Windows reading `~\.agents\skills` **unverified** |
| Binary | no link: the user `PATH` gets `<root>\current\bin` once (the registry value keeps its `REG_EXPAND_SZ` type; `WM_SETTINGCHANGE` is broadcast), so the entry survives upgrades | registry round trip **verified** on the CI runner; a new terminal finding `ai-gent` **unverified** |
| State | `%LOCALAPPDATA%\ai-gent\state\targets` | **verified** |
| `-Uninstall` | removes links into this tree, generated entries and (without `-Target`) the `PATH` entry; the install tree stays | **verified** |
| `-DryRun` | prints, changes nothing (no links, files, state or `PATH`) | **verified** |
| Hooks | Claude Code runs `plugins/*/hooks/guard.sh` and `scripts/update-check.sh` as shell commands. Native Windows Claude Code needs Git Bash for them; without it the guards do not run | **unverified**, the open point of spec 033 question 6 |
| `ai-gent-update.sh`, `update-check.sh` | POSIX scripts; on Windows `ai-gent update` (034) replaces them | not packaged for Windows use |

Environment overrides mirror `setup.sh` (`CLAUDE_SKILLS_DIR`, `AGENTS_SKILLS_DIR`) plus `AI_GENT_STATE_DIR`, `AI_GENT_NO_PATH`, and `AI_GENT_USER_PATH_FILE` (a file standing in for the registry value, tests only).

## 3. Execution policy and encoding

- The default policy of Windows client editions is `Restricted`: a `.ps1` file does not run. `irm ... | iex` is not a file, so the one-liner works. Everything the installer starts itself (`setup.ps1`) and everything `ai-gent update` starts uses `-ExecutionPolicy Bypass` for that process only. A Group Policy that enforces `AllSigned` or `MachinePolicy` still wins; the installer then fails with PowerShell's own message.
- `install.ps1` and `setup.ps1` are ASCII: a download without a charset is decoded as Latin-1 by Windows PowerShell 5.1.
- The binary is unsigned (D14): SmartScreen may warn for a browser download; the installer downloads with `Invoke-WebRequest`, which adds no Mark of the Web, and extracts entry by entry itself, so no Zone.Identifier stream is written.

## 4. WSL and Git Bash as the alternative route (D24, T057)

`install.ps1` is the main route on Windows. Two alternatives stay supported and are what to use for every agent beyond Claude Code and Codex:

- **WSL 2** (a Linux distribution inside Windows): follow the Linux instructions with `curl -fsSL https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.sh | sh`. It installs the Linux binary and links into the distribution's home, not into the Windows profile, so an agent running on the Windows side does not see it.
- **Git Bash** (Git for Windows): the same `install.sh` detects `MINGW*`/`MSYS*` and installs the Windows zip (needs `unzip`); `setup.sh` needs bash 4 or newer, which Git Bash ships. Symlinks need Developer Mode or an elevated shell there, so `install.ps1` is the easier route when only Claude Code or Codex is wanted.

The README and install-page text for both routes is spec 039 (T041, T043).

## 5. Owner check (T058), exact steps

Done on a real Windows 11 x64 machine and a Windows 11 arm64 machine, each without Git Bash and without being an administrator (a standard user), after a release with `install.ps1` exists. See the pull request that added this file for the list.
