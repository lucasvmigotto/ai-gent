# Implementation Plan: Rust workspace and release

**Branch**: `docs/specs-rust-core` (spec) | **Date**: 2026-11-01 | **Spec**: `spec.md`

## Summary

A `rust/` Cargo workspace (`ai-gent-core` library, `ai-gent` binary) with a pinned toolchain and strict gates, built natively for six targets by a reusable workflow, packaged one archive per target by `scripts/package.py`, fetched by `install.sh` per platform, attested, and published to crates.io only through Trusted Publishing from the owner-controlled `crate-release` environment. A throwaway `version` + `doctor` binary de-risks macOS and Windows first.

## Technical context

**Language**: Rust, edition 2024; toolchain channel copied from `~/codes/rstory/rust-toolchain.toml` at build time (1.98.1 there on 2026-11-01), `rust-version` 1.92 as rstory. **Dependencies (initial)**: `clap` (derive), `serde`, `serde_json`, `thiserror`; nothing with TLS. **Testing**: `cargo test`, `assert_cmd`, `insta` snapshots for `--json`, `tempfile`. **Targets**: the six triples in `spec.md` FR-006. **CI**: GitHub Actions, actions pinned by commit SHA like the existing workflows. **Constraints**: AGENTS.md/CLAUDE.md (propose first, `scripts/check.sh` before every commit, no hand version bumps or tags, `git:workflow`).

## Constitution and constraint check

| Constraint (source) | Check | How satisfied |
|---|---|---|
| D4 workspace under `rust/`, two crates | pass | FR-001 |
| D5 names, placeholders 0.1.0 (already published 2026-11-01), owner sets up Trusted Publishing, assistant never handles a token | pass | FR-013, `quickstart.md` owner steps; no task asks for a token |
| D6 GPL-3.0-or-later | pass | `license` field, `deny.toml` allow-list |
| D3 six targets | pass | FR-006 |
| D14 de-risk first | pass | Phase 0 is first; Phase 3 depends on its decision |
| D16 install.sh/setup.sh stay POSIX bootstrap; dev-only tools stay Python/shell | pass | `install.sh` gets a platform map, no Rust dependency; `package.py`, `release.py`, tests stay dev-only |
| D7/D8 no dual paths, no fallback | n/a here | enforced in 033-038 |
| Package allowlist ships the binary, excludes Rust sources and `target/` | pass | `package.py --binary` adds only `bin/`; `rust/` is not in `TREES`; a test asserts no `*.rs`, `Cargo.*`, `target/` in the archive |
| `shared/releasing.md`: releases are automated, tags without `v`, atomic push, draft until assets attach | pass | the matrix is added between `release` and `package`; the draft rule is unchanged |
| `shared/secrets.md` | pass | no token touched by the assistant; no secret printed |
| CLAUDE.md: Layout rules (plugin vs `opencode/` vs new dir) | decision | `rust/` is not a plugin; `scripts/consistency.py` must not try to read it as one (task T020) |
| Naming rule D21 (3.0.0 / 3.x; tags carry no `v`) | pass | this feature never writes it |

## Architecture

### Workspace layout

```
rust/
  Cargo.toml              workspace: members, [workspace.package], [workspace.lints], [workspace.dependencies]
  Cargo.lock              committed (a binary workspace)
  rust-toolchain.toml     channel, components, 6 targets
  deny.toml
  .cargo/config.toml      [target.'cfg(windows)'] rustflags = ["-C", "target-feature=+crt-static"]
  crates/
    ai-gent-core/         library: output envelope, exit codes, paths, version, (033) guard, (034) update
      src/lib.rs
      src/output.rs       Json envelope {schema, ok, data}, writers
      src/exit.rs         ExitCode enum (see contracts/cli.md)
      src/version.rs      crate version + AI_GENT_RELEASE + target triple (build.rs sets TARGET)
      src/paths.rs        install root, state, config dirs per OS (XDG on Unix, %LOCALAPPDATA% on Windows)
      build.rs            emits AI_GENT_TARGET, AI_GENT_COMMIT (git rev-parse, optional)
    ai-gent/              binary
      src/main.rs         parse, dispatch, catch_unwind -> exit codes
      src/cli.rs          clap derive: global --json, subcommands
      src/cmd/version.rs
      src/cmd/doctor.rs   throwaway in 2.27 (034 replaces)
      tests/cli.rs        assert_cmd + insta
```

Tests that need repo fixtures (`shared/guard-fixtures/`, see 033) read them by relative path from the repository; they are not packaged and not part of the crates' `include` list, so a published crate's own tests (`cargo test` from the tarball) skip them through a `#[cfg_attr]`-free runtime check (`if !fixtures_dir().exists() { return }`), never fail.

### Why native runners, not zig, `cross` or cargo-dist

- Native runners are free and unlimited for public repositories, including `macos-15`, `macos-15-intel`, `windows-11-arm` and `ubuntu-24.04-arm` [RESEARCHED: S2, S6]. Each binary is smoke-tested on its own OS and architecture, which cross-compilation cannot do.
- `cargo-zigbuild`/`zig cc` cross-compiles Linux/macOS/Windows from one host [KNOWN] but adds a toolchain to pin, cannot run the result, and macOS cross builds from Linux need an SDK whose licence is awkward [KNOWN]. It buys nothing here because the runners exist.
- `cross` needs a container engine in CI and targets Linux well, macOS/Windows poorly [KNOWN].
- `cargo-dist` generates and owns the release workflow [ASSUMPTION: its docs were not retrieved, see `research.md`]. ai-gent's release already exists (draft release, atomic push, `package.py`, attestation, back-merge); adopting a generator would replace it, which D4 ("one release flow, one tag") argues against. We take its idea (per-target archives, checksums) and keep our flow.

### Packaging decision: one package per target

Options: (A) one universal package holding six binaries; (B) one package per target with the content + that target's binary; (C) content package + separate bare binaries.

Chosen **B**. Reasons: the content is about 650 KB while a Rust binary is a few MB, so A makes every user download about six binaries to use one; C splits one install into two downloads, two checksums and two attestation checks and lets content and binary drift; B keeps today's mental model ("one archive, one checksum, one attestation, one version directory") and keeps `current/` self-contained so `previous` rollback restores binary and content together (D15 layout). Cost: six content-identical packaging runs (seconds) and a larger release asset list (7 archives). The content-only `ai-gent-<ver>.tar.gz` stays for 2.x for old installers (FR-009).

Archive layout: `ai-gent-<ver>-<triple>/` top directory (as today's `ai-gent-<ver>/`), `bin/ai-gent[.exe]` at mode 0755, everything else as today. `MANIFEST` includes the binary's sha256. `VERSION` unchanged.

### Release workflow shape

```
push to main
  check (+ rust gates)
   -> release (plan, commit incl. rust/Cargo.toml + Cargo.lock, tag, DRAFT release, back-merge)   [unchanged except git add list]
       -> rust-build (reusable, matrix x6: build --release --locked, smoke, upload artifact)
           -> package (download 6 binaries; package.py per target + content-only; SHA256SUMS; attest all; upload; publish release)
           -> publish-crates (needs rust-build, if vars.AI_GENT_CRATES_PUBLISH == 'true'; environment release; id-token: write)
```

`rust.yml` (pull_request on `rust/**`, `shared/guard-fixtures/**`, workflow files; and `workflow_dispatch`) calls `rust-build.yml` with `dry_run: true` and stops after uploading artifacts and running `package.py --check --with-binaries`. Manual runs of `release.yml` keep working as they do; the new jobs follow the existing `if: needs.release.outputs.version != ''` guard, so a dispatch that plans nothing builds nothing.

Differences from rstory worth recording: rstory is tag-triggered with a `verify` job and a `tag.yml` dispatcher; ai-gent tags inside the release job, so the `verify` job's checks (tag equals commit, on main) are satisfied by construction and are replaced by a lighter assertion step at the start of `package` (tag resolves to `HEAD` of the checkout, release outputs equal `VERSION`). The OIDC publish stays in `release.yml` itself because crates.io Trusted Publishing validates the workflow file name [KNOWN from rstory's comment; the crates.io page is where the owner confirms it]. S1 adds that `workflow_run` and `pull_request_target` cannot publish, which is another reason to keep it in the tag/main workflow.

### macOS and Windows specifics

- macOS: build with `MACOSX_DEPLOYMENT_TARGET`; Rust's linker produces an ad-hoc signature on arm64 by default so the kernel will run it [KNOWN, verify in probe P2]; `curl`-fetched files carry no quarantine attribute [RESEARCHED: S3], so the `install.sh` path avoids Gatekeeper; browser downloads are the exposed path. A notarization step (if the decision is to sign) goes in `rust-build.yml` after `build`, guarded by the presence of signing secrets stored by the owner as environment secrets and never read by the assistant.
- Windows: `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` with static CRT; `curl.exe` ships with Windows 10 1803 and later [KNOWN]. Unsigned executables get SmartScreen reputation warnings when they carry the Mark of the Web, and Smart App Control can block unsigned executables regardless [RESEARCHED: S4]; probe P5-P7 measure it.

### CI wiring without slowing everything

- `check.yml` gets a `changes` step: on pull requests it diffs against the base and sets `rust=true` when any of `rust/**`, `shared/guard-fixtures/**`, `plugins/*/hooks/guard.sh`, `opencode/guard/**`, `.github/workflows/rust*.yml`, `scripts/test-rust-workflows.sh` changed; on `main` and schedule it is always true. The Rust steps run in the existing ubuntu job (no extra runner) with `Swatinem/rust-cache`-style caching keyed on `rust/Cargo.lock` and the toolchain file [ASSUMPTION: pin by SHA when chosen].
- `scripts/check.sh` runs the same four gates locally when `cargo` is on `PATH`; measured timing is a task (T023), the target is a cached incremental run under 60 seconds on the owner's machine [ASSUMPTION].

## Files

| Path | What |
|---|---|
| `rust/**` | the workspace (above) |
| `.github/workflows/rust-build.yml` | reusable matrix build and smoke |
| `.github/workflows/rust.yml` | PR gate and dry-run rehearsal |
| `.github/workflows/release.yml` | gains `rust-build`, per-target packaging, `publish-crates`, wider attestation and `git add` list |
| `.github/workflows/check.yml` | change filter, Rust steps |
| `scripts/package.py` | `--target`, `--binary`, content-only output; refuses `rust/` sources |
| `scripts/test-package.sh` | per-triple install matrix over `file://` |
| `scripts/test-rust-workflows.sh` | shape test, wired in `check.sh` |
| `scripts/check.sh` | Rust section |
| `scripts/release.py`, `scripts/test-release.sh` | D23: binary crate lockstep with the tag, core independent from commits, Cargo.lock sync, dependency-requirement check, dry-run preview |
| `scripts/consistency.py` | ignores `rust/`; checks crate metadata equals `plugin.json` license/author |
| `install.sh`, `setup.sh` | platform map, binary install, PATH link |
| `specs/032-.../de-risk.md` | probe sheet and decision |

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| `release.py` manages per-manifest versions from commit types, not a lockstep tag version, and reads JSON-style manifests | D23 needs two behaviors (write the tag into the binary crate; commit-driven bump for core, minor for breaking while 0.x) and a Cargo.lock edit | T021 specifies and implements them as a small `rust` target kind in `release.py` with tests in `scripts/test-release.sh`; `--dry-run --json` lists both targets (`kind` `rust`) |
| `Cargo.lock` stale after the bump | `--locked` fails on the release commit | release job runs `cargo update --workspace --offline` or edits the two entries; covered by a test that bumps in a temp repo |
| First real run of the matrix happens on `main` | tag and draft exist, nothing published | rehearsal on `dev` (T031); the draft rule means `latest` is untouched; recovery in `quickstart.md` |
| `windows-11-arm` or `macos-15-intel` label retired or flaky | a required leg blocks releases | probes record availability; the leg is required at the 3.0.0 gate (039) and optional (`continue-on-error`) before it, with the failure surfaced in the run summary |
| Unsigned macOS/Windows binaries are blocked for browser downloads | support burden | D14 decision; `install.sh` path avoids quarantine; README documents the workaround |
| Package size grows (7 archives) | slower releases, bigger assets | binary profile `opt-level="z"`/`strip`/`lto` measured in T015; size budget in `data-model.md` |
| Crates.io Trusted Publishing for two crates: per-crate configuration and workflow-file match | publish fails | `quickstart.md` owner steps; dry run; publish job idempotent |
| A public crate named `ai-gent` before the binary is useful | confusion | placeholders' README says so; real publishing off until the owner flips the variable |

## Phased approach

- **Phase 0 - de-risk (D14)**: workspace skeleton with `version` and `doctor`; `rust-build.yml` matrix on a branch; probe sheet; decision. Nothing ships.
- **Phase 1 - workspace and gates**: toolchain, deny, lints, `check.sh` and CI wiring, `consistency.py`.
- **Phase 2 - packaging and installer**: `package.py`, tests, `install.sh`, `setup.sh`.
- **Phase 3 - release integration**: `release.yml`, attestation, release.py/Cargo.lock, shape test, rehearsal.
- **Phase 4 - owner steps and the 2.27 release**: Trusted Publishing configured (placeholders are already published), `shared/secrets.md` row for `ai-gent/crates.io/pat`, the variable stays off, cut 2.27.
