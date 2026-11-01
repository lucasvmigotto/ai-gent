# Contract: workflows (checked by `scripts/test-rust-workflows.sh`)

Modeled on `~/codes/rstory/scripts/test-release-workflows.sh`: pure bash, strips comments, greps the real files. Every check below is one `ok` line.

## `rust-build.yml` (reusable, `workflow_call`)

Inputs `ref` (string), `release` (string, default `dev`), `dry_run` (boolean). Permissions `contents: read` at the top, nothing else. Matrix of six legs with `fail-fast: false` before the 3.0.0 gate and `true` after (039). Steps per leg: checkout at `inputs.ref` with `persist-credentials: false`, install the pinned toolchain from `rust/rust-toolchain.toml`, `cargo build --release --locked --target <triple>` with `AI_GENT_RELEASE` set, smoke (`ai-gent version --json` parsed; `ai-gent doctor --json` allowed to fail only on `install_root`), `upload-artifact` of the binary as `bin-<triple>`.

Checks: all actions pinned by 40-hex SHA; no `secrets` context; `permissions` top-level present; six triples present; runner labels exactly those in `spec.md` FR-006; `+crt-static` reaches Windows legs; smoke step present.

## `rust.yml`

Triggers `pull_request` (paths filter) and `workflow_dispatch`; calls `rust-build.yml` with `dry_run: true`; no `contents: write`, no `id-token`, no `attestations`, no `secrets`. Checks: none of those permissions; `dry_run: true` literal.

## `release.yml` additions

- `rust-build` job `needs: release`, `if: needs.release.outputs.version != ''`, `ref: refs/tags/<version>`.
- `package` `needs: [release, rust-build]`; downloads `bin-*`; runs `package.py` once per triple plus the content-only run; `sha256sum -c SHA256SUMS`; attest step lists every archive and `install.sh`; the release is published (`--draft=false --latest`) only after upload.
- `publish-crates` `needs: [release, rust-build, package]` (a failed package leaves no crate published), `if: vars.AI_GENT_CRATES_PUBLISH == 'true' && needs.release.outputs.version != ''`, `environment: crate-release`, `permissions: { contents: read, id-token: write }`, uses `rust-lang/crates-io-auth-action@<sha>`, publishes `ai-gent-core` then waits for the index then `ai-gent-dbrun` (once it exists, D28) then waits again then `ai-gent`, skips a crate whose version already exists.
- `id-token: write` appears exactly twice in the file: `package` (attestation, existing) and `publish-crates`.
- `grep CARGO_REGISTRY_TOKEN` finds only `env:` wiring from the auth step output, never `secrets.`.
- `git add` in the release step lists `rust/crates/*/Cargo.toml rust/Cargo.lock` (the versions live in the crates' manifests, not in the workspace `rust/Cargo.toml`), guarded by `[ -f rust/Cargo.lock ]`.
- `package` runs `scripts/package-release.sh` (one `package.py` call per triple plus content-only, `sha256sum -c`, the `## Checksums` notes); `rust.yml` runs the same script. The `aarch64-pc-windows-msvc` archive is skipped with a warning when its experimental leg produced no binary.
- `scripts/rust-publish.sh` (`metadata`, `dry-run`, `publish`) holds the idempotent publish and the T033a metadata gate; `scripts/test-rust-publish.sh` tests it with a fake cargo and index.
- `publish-crates` is never reachable from `rust.yml` (it is a job of `release.yml` only); `release.yml`'s `release` job keeps its `github.ref == 'refs/heads/main'` condition, so a dispatch from another ref builds nothing.

## D24, D25 additions

- `check.yml` gains a required `windows` job (windows-2025; the arm64 leg is `windows-11-arm`) that runs `scripts/test-install-ps1.ps1` and the Windows guard fixtures.
- `package` uploads `install.ps1` and attests it; a final step appends a `## Checksums` block (sha256 of `install.sh`, `install.ps1`, every archive) to the release notes with `gh release edit --notes-file` before `--draft=false`. Shape checks: the step exists and runs before the publish call.
