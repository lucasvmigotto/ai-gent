# Contract: `install.ps1`

Mirrors `install.sh` (see `contracts/release-assets.md` for the layout).

```
irm https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.ps1 | iex
.\install.ps1 [-Version <X.Y.Z|latest>] [-Target claude,codex] [-List] [-Rollback] [-Prune] [-VerifyAttestation] [-SkipPrompt]
```

| Variable | Meaning |
|---|---|
| `AI_GENT_HOME` | install root, default `%LOCALAPPDATA%\ai-gent` |
| `AI_GENT_RELEASE_BASE` | `https://...`, `file:///C:/...` or a directory path; `<base>\<ver>\ai-gent-<ver>-<triple>.zip`, `SHA256SUMS`, `LATEST` |
| `AI_GENT_TARGET` | triple override (tests) |

Steps and their order (each failure stops before the next; nothing is written under the root before step 4 passes):

1. Resolve the version (`latest` from the `releases/latest` redirect, or `<base>\LATEST`); pick the triple from `PROCESSOR_ARCHITECTURE` (`AMD64` to `x86_64-pc-windows-msvc`, `ARM64` to `aarch64-pc-windows-msvc`).
2. Download the zip and `SHA256SUMS` to a temp directory.
3. `Get-FileHash -Algorithm SHA256`, compare with the `SHA256SUMS` line; optional `gh attestation verify`.
4. Open the zip with `System.IO.Compression.ZipFile`, reject entries with `..`, a rooted path or a drive letter, then extract into `.stage.<pid>` under the root.
5. Check `VERSION` equals the requested version, `bin\ai-gent.exe` exists; rename the stage to `<ver>`.
6. Flip `current` and `previous` (junctions, FR-020); write or remove `pinned`.
7. Run `current\setup.ps1` (links, user `PATH`); print the version and how to roll back.

Exit codes: 0 success; 1 failure; 2 bad arguments; 4 verification failed. Output prefix `ai-gent:` like `install.sh`.

Test (`scripts/test-install-ps1.ps1`, run by the `windows` job of `check.yml`): build a package with `package.py --target x86_64-pc-windows-msvc` from a stub `ai-gent.exe`, serve it from a directory base; cases: install, upgrade, rollback, prune, pin and unpin, bad checksum writes nothing, unsafe zip entry writes nothing, path with spaces, no administrator rights (run as the runner's normal user), junction targets resolve, second install is idempotent.
