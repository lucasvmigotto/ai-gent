# De-risk sheet: macOS and Windows unsigned binaries (D14)

Fill in during Phase 0. CI proves a binary runs on a runner of its OS and architecture; it cannot show Gatekeeper, SmartScreen or Smart App Control prompts, so P2-P8 need a person on real machines or a VM. Record the OS version, the date and the observed behavior. "Expected" is what research suggests (`research.md`), not a result.

| # | Probe | Expected | Observed | Date | OS version |
|---|---|---|---|---|---|
| P1 | CI: all six legs build and run `ai-gent version --json` | pass | | | runners |
| P2 | macOS arm64: `curl -fsSLO` the archive, `tar xzf`, run `bin/ai-gent version` | runs; no quarantine attribute [RESEARCHED: S3]; `codesign -dv` shows an ad-hoc signature [KNOWN, verify] | | | |
| P3 | macOS arm64: download the same archive in a browser, extract with Finder or `tar`, run from Terminal | `xattr -l` shows `com.apple.quarantine` on the archive; a CLI binary extracted by `tar` is not quarantined [RESEARCHED: S3]; Finder extraction may be | | | |
| P4 | macOS x86_64 (or Rosetta): P2 | runs | | | |
| P5 | Windows 11 x64: `curl.exe -LO` the zip, extract with `tar -xf`, run `ai-gent.exe version` | runs; no Mark of the Web [RESEARCHED: S4] | | | |
| P6 | Windows 11 x64: download in a browser, extract with Explorer, run from PowerShell | SmartScreen reputation prompt or block for an unsigned file [RESEARCHED: S4] | | | |
| P7 | Windows 11 with Smart App Control on: P5 | may block unsigned executables regardless of download path [RESEARCHED: S4] | | | |
| P8 | Windows 11 arm64: P5 | runs | | | |
| P9 | Claude Code hook invocation: a hook command that runs the binary on macOS and on Windows (Git Bash) | runs without a prompt | | | |
| P10 | Antivirus/Defender: scan result of the archive (Windows Security) | clean; note any heuristic hit | | | |

## Decision (owner)

One of: **A** ship unsigned, document the browser-download workaround (`xattr -d com.apple.quarantine`, "Unblock" in file properties) and recommend the installer path; **B** ad-hoc sign on macOS only; **C** Developer ID + notarization (macOS) and/or code signing (Windows) with recurring cost.

Recorded: _empty until P1-P10 are filled in._ Recommended (before evidence): **A** for 2.27-2.28 because the installer path is `curl`, which avoids the quarantine and Mark-of-the-Web triggers; revisit if P6/P7 show hard blocks or support cost appears.
