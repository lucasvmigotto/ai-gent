# Platform probes for the owner (Windows and macOS) - DRAFT checklist

**Status: prepared, nothing here was run.** These probes need real machines (the preparer worked on Linux with no network). Each section says what to run, the expected output, and where to record the result. Record by pasting output under the "Result" line in a copy of this file in the release PR, or in the file the third column names. Base assumed: `origin/dev` at 41dff19. Nothing here ticks a task; the owner does.

Tag to test: whichever release the gate is for. Replace `<tag>` below. Until a release that contains the binary exists (see the first section of `evidence/g28.md`), probes that need release assets cannot be run from GitHub; use a CI artifact `bin-<triple>` from a `rust.yml` run (`gh workflow run rust.yml --ref dev`, then `gh run download`) and `AI_GENT_RELEASE_BASE` as `scripts/test-update.sh` does.

Machines: macOS arm64, macOS x64 (or a Rosetta shell), Windows 11 x64, Windows 11 arm64 without Git Bash. Linux x64 and arm64 are covered by CI and a container, listed only in probe 4.

## 1. Installers

| Item | Run | Expected | Record in |
|---|---|---|---|
| macOS, `install.sh` | `curl -fsSL https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.sh \| sh -s -- --skip-prompt --target claude` (system `/bin/bash` is 3.2; the installer is POSIX `sh`) | "installed X.Y.Z in ...", `~/.local/bin/ai-gent version --json` prints the version, no quarantine prompt | this file; also 032 `de-risk.md` rows P2, P4 |
| macOS, download in a browser | download the `ai-gent-<ver>-<triple>.tar.gz` in Safari, extract with Finder, run `bin/ai-gent version` from Terminal; `xattr -l <archive>`; `codesign -dv bin/ai-gent` | `com.apple.quarantine` on the archive, binary behavior, ad-hoc signature | 032 `de-risk.md` P3 |
| Windows, `install.ps1` one-liner | in Windows PowerShell 5.1 and again in PowerShell 7: `irm https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.ps1 \| iex` | "installed ..." under `%LOCALAPPDATA%\ai-gent\<version>`, `current` junction, `ai-gent.exe version --json` works, no Git Bash needed | 032 T058 transcript; `de-risk.md` P5, P8 |
| Windows, saved file | `powershell -NoProfile -ExecutionPolicy Bypass -File .\install.ps1 -List`, then `-Rollback` after a second install | lists versions; rollback swaps `current` and `previous` | this file |
| Windows, browser download | download the `.zip` in a browser, extract with Explorer, run from PowerShell | SmartScreen prompt or block for an unsigned file (row P6); also with Smart App Control on (P7); Defender scan clean (P10) | `de-risk.md` P6, P7, P10 |
| Claude Code hook invocation | a hook command that runs the installed binary, on macOS and on Windows | runs without a prompt | `de-risk.md` P9 |

Result:

## 2. `ai-gent doctor`

| Run | Expected | Record in |
|---|---|---|
| `ai-gent doctor` then `ai-gent doctor --json` (Windows: `ai-gent.exe doctor`) on the freshly installed home | exit 0; checks `binary`, `install_root`, `version_match`, `install_integrity`, `path_link`, `agent_links`, `hooks`, `guard_selftest`, `tools`, `state`, `update_cache`, `environment`, `permissions`, each `ok`, `warn` or `skip`, none `fail`; healthy run takes under about 2 s | this file; T063 requires "doctor green on a migrated home" |
| the same after `ai-gent update --rollback` and after updating again | still green | this file |
| Windows only | `tools` checks `powershell` instead of `sh` (034 T024); `ai-gent update` runs the target release's `install.ps1` | this file; the Windows leg of `scripts/test-install-ps1.ps1` is the CI evidence (034 T024, commits feb057b and 8be17ec) |
| macOS | `permissions` check: modes only | this file |

Result:

## 3. Guard latency (`scripts/bench-guard.py`)

Needs Python 3 and a binary (`--bin PATH` or `AI_GENT_BIN`). From a checkout of the repository:

```sh
python3 scripts/bench-guard.py --impl bin --bin /path/to/ai-gent --check     # Windows: py -3 scripts\bench-guard.py --impl bin --bin C:\path\ai-gent.exe --check
python3 scripts/bench-guard.py --impl sh                                      # the shell baseline, for the "baseline + 5 ms" rule
```

Expected: p50/p95/p99 over 100 runs per case; `--check` exits 0 when every p95 is within the absolute budget (15 ms Linux and macOS, 60 ms Windows in `FR-012`; CI uses `--budget-ms 25` on the shared runners). Numbers already recorded by CI (033 T053, PR #390), slowest case `git-commit-branch`, p95: Linux x64 11.74 ms, Linux arm64 12.19, macOS arm64 33.80, macOS x64 35.32, Windows x64 45.38, Windows arm64 54.40. Record the owner's own machine numbers (macOS and Windows rows are still open: 033 T002), and decide the macOS budget (CI numbers are above 15 ms) and the Windows budget (033 T003 replaces the `[ASSUMPTION]` in FR-012). On Windows run it from Git Bash if you want the `sh` baseline too; the `bin` implementation does not need it. Record in `specs/033-guard-evaluator/quickstart.md` (the table T001 started).

Result:

## 4. Six-target install (T063, after the release)

The six targets are the six archive triples: `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`. On each, with `releases/latest` pointing at the release under test:

| Step | Command | Expected |
|---|---|---|
| resolve | `curl -fsSL -o /dev/null -w '%{url_effective}\n' https://github.com/lucasvmigotto/ai-gent/releases/latest` | `.../releases/tag/<tag>` |
| install | `install.sh` (Linux, macOS) or `install.ps1` (Windows), `--skip-prompt` / `-SkipPrompt`; add `--verify-attestation` / `-VerifyAttestation` when `gh` is installed | checksum ok (and attestation ok); installs the triple's archive |
| run | `ai-gent version --json` | the release version and the matching triple |
| attest | `gh attestation verify <asset> -R lucasvmigotto/ai-gent` for the six archives and the installers | verified |
| health | `ai-gent doctor` | exit 0 |

Linux x64 and arm64 can be a clean container each (`podman run --rm -it debian:stable-slim`, install `curl`, `ca-certificates`, `tar`, `gzip`). A migrated home (T063 says "on a migrated home") is made by installing the previous 2.x first, then updating; `scripts/test-migrate.sh` is the offline model.

Result (one line per triple):

## 5. Codex live turn firing the guard hook

`contracts/tier-policy.md` ("Codex hook wiring: evidence") proves that Codex 0.166.0 parses the `[[hooks.PreToolUse]]` wiring and that `ai-gent guard` satisfies the hook contract (216 of 216 shell fixtures through Codex-shaped events). **Unproven:** a live Codex firing the hook, the shell tool's real `tool_name`, how Codex treats `ask`, and the trust flow. The owner run (needs a login and a model):

1. Build or install the binary. Put a throwaway `CODEX_HOME` (not your real one) with `config.toml`:
   ```toml
   [features]
   hooks = true

   [[hooks.PreToolUse]]
   matcher = "^(Bash|shell|local_shell|exec_command|shell_command)$"

   [[hooks.PreToolUse.hooks]]
   type = "command"
   command = "<abs path>/ai-gent guard --guard git"
   timeout = 10
   statusMessage = "ai-gent guard"
   ```
2. Start `codex` with that `CODEX_HOME`, review and trust the hook when Codex asks (it lists as `untrusted` until then; write down exactly how you trusted it).
3. In a scratch repository on a branch named `dev`, ask Codex to run `git commit --allow-empty -m test` (the git guard asks or denies a commit on a main branch), then `git push --force origin dev`, then a harmless `ls`.
4. Expected: the first two are stopped or flagged by the hook with the guard's reason; `ls` runs. Write down: the `tool_name` the hook received (add `tee -a /tmp/hook-in.json` in a wrapper command if needed; the fixture driver in the evidence section reads `tool_name`, `cwd`, `tool_input`), what Codex does with `permissionDecision: "ask"`, and whether a hook error or timeout let the call proceed (fail-open).
5. Record under "Codex hook wiring: evidence" in `contracts/tier-policy.md` or in a new "Live turn" paragraph there; the owner then decides whether tier 2 reads "enforced". Do not paste real keys or transcripts.

Result:

## 6. Windows `pass` / gopass gate (spec 037 T006)

On a Windows machine with Git, Gpg4win and gopass (`winget install gopass.gopass`), plus the `pass` shim if available (Git Bash or WSL):

1. Create an entry with `pass insert ai-gent/probe/windows` (or gopass), using a dummy value, never a real credential.
2. With the `ai-gent` binary: read it through the passstore path (the `repo`/`db` runner commands, or whatever 037 documents for the entry check). Verify: (a) reading an entry created by `pass` works through gopass; (b) the flag that prints only the first line of the entry; (c) where the store lives (`PASSWORD_STORE_DIR`, `~/.password-store`, or gopass's own location; FR-A4 marks this `[ASSUMPTION]`); (d) `.gpg` and `.age` entry file detection.
3. Record in `specs/037-repo-spec-pass-compact-tools/measurements.md` (T006 says so; the file already lists "T006 gopass on Windows" as open) and, if any assumption turns out wrong, correct FR-A4 in the spec. Also run the related owner gate T028 (`scripts/test-reporun-github.py` against a real GitHub repo) on the same machine if convenient; its token must come from `pass`, never typed into a command.

The assistant never reads values; only you run these.

Result:

## Checklist (all unticked)

- [ ] 1 Installers: macOS arm64, macOS x64, Windows x64 (PS 5.1 and 7), Windows arm64
- [ ] 2 `ai-gent doctor` green on each, after rollback and update
- [ ] 3 `bench-guard.py --impl bin --check` on macOS and Windows, numbers recorded
- [ ] 4 Six-target install after the release
- [ ] 5 Codex live turn fired the hook
- [ ] 6 Windows pass/gopass gate
