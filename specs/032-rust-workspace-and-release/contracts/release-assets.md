# Contract: release assets and install tree

## Assets of release X.Y.Z (2.x series)

| Asset | Content | Attested |
|---|---|---|
| `ai-gent-X.Y.Z.tar.gz` | content only, as in 2.26 (compat for old installers) | yes |
| `ai-gent-X.Y.Z-x86_64-unknown-linux-musl.tar.gz` | content + `bin/ai-gent` | yes |
| `ai-gent-X.Y.Z-aarch64-unknown-linux-musl.tar.gz` | same | yes |
| `ai-gent-X.Y.Z-aarch64-apple-darwin.tar.gz` | same | yes |
| `ai-gent-X.Y.Z-x86_64-apple-darwin.tar.gz` | same | yes |
| `ai-gent-X.Y.Z-x86_64-pc-windows-msvc.zip` | content + `bin/ai-gent.exe` | yes |
| `ai-gent-X.Y.Z-aarch64-pc-windows-msvc.zip` | same | yes |
| `install.sh` | the installer of that release | yes |
| `SHA256SUMS` | one line per asset above except itself | no (checksums prove integrity only) |

From 3.0.0 the content-only asset is not published.

## Archive layout

`ai-gent-X.Y.Z-<triple>/` then: `bin/ai-gent[.exe]` (0755), `setup.sh`, `install.sh`, `plugins/`, `opencode/` (until D13 removes the guard rules), `shared/` files reached by symlinks, `LICENSE`, `README.md`, `VERSION`, `MANIFEST` (`sha256sum -c` format, includes the binary). Never present: `rust/`, `Cargo.*`, `target/`, `*.rs`, `deny.toml`, `rust-toolchain.toml`, `shared/guard-fixtures/`.

Determinism: sorted entries, fixed mtime/uid/gid/modes, gzip without a timestamp (zip: fixed timestamps, stored order sorted). The binary's bytes depend on the build; reproducibility of the binary is a non-goal in 2.27 (the archive built from a given binary is deterministic).

## Install tree

```
$AI_GENT_HOME/
  X.Y.Z/            unpacked archive (bin/ai-gent inside)
  current  -> X.Y.Z
  previous -> W.V.U
  pinned            (file) present after an explicit VERSION=x.y.z install
```

`~/.local/bin/ai-gent` -> `$AI_GENT_HOME/current/bin/ai-gent`. Hooks reference `${CLAUDE_PLUGIN_ROOT}/../../bin/ai-gent` (the plugin root sits at `<ver>/plugins/<name>`), so a hook works even when `PATH` has no link.

## `install.sh` additions

- `AI_GENT_TARGET=<triple>` overrides detection (tests, odd shells).
- Failure texts: `unsupported platform <os>/<arch>; supported: <list>`; `no archive for <triple> in release <ver>` (an old release); checksum mismatch as today.
- `--verify-attestation` verifies the platform archive and `install.sh`.
