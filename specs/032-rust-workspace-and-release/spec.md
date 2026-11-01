# Feature Specification: Rust workspace and release

**Feature Branch**: `docs/specs-rust-core` (specification only; built on its own branches, one per phase)

**Created**: 2026-11-01

**Status**: Draft

**Input**: `docs/product/rust-decisions.md` (D1-D21). ai-gent will ship one binary, `ai-gent`, that replaces the runtime shell and Python tools. This feature is its foundation: the `rust/` workspace, the toolchain and lint gates, the cross-platform build and release matrix, the packaging, the installer change that fetches the right binary, crates.io Trusted Publishing, and the early macOS and Windows de-risk step (D14). The subcommands themselves are other specs (033 guard, 034 update and doctor, 035-038 ported tools).

**Depends on**: 006 (installer), the release flow in `shared/releasing.md`. Read by 033, 034, 035-038, 039.

## Context and ground rules

Verified in the repo on 2026-11-01 (commit 7c09c56 on `dev`):

- `.github/workflows/release.yml` runs on pushes to `main` (and `workflow_dispatch`): `check` -> `release` (plan with `release.py --dry-run`, commit, annotated tag, **draft** GitHub release, back-merge into `dev`) -> `package` (builds `ai-gent-<ver>.tar.gz`, `install.sh`, `SHA256SUMS` with `scripts/package.py`, attests provenance with `actions/attest-build-provenance`, uploads, publishes the draft). Tags are pushed with `GITHUB_TOKEN`, so they start no workflow: everything that must run for a release is a job of this file.
- `scripts/package.py` is an allowlist (`ROOT_FILES`, `TREES = [plugins, opencode/guard]`, symlinked `shared/` files); `scripts/package.py --check` runs in `scripts/check.sh`; `scripts/test-package.sh` covers determinism, checksum refusal, install, upgrade, rollback, prune.
- `install.sh` resolves `latest` from the GitHub redirect, fetches `ai-gent-<ver>.tar.gz` and `SHA256SUMS`, verifies the checksum, unpacks into `$AI_GENT_HOME/<ver>`, flips `current` and `previous`, writes `pinned` for an explicit `VERSION`, and runs `setup.sh`.
- `release.json` lists non-plugin manifests that `scripts/release.py` bumps (today only `docs/site/package.json`); crate versions would be bumped the same way.
- `scripts/check.sh` and `.github/workflows/check.yml` (ubuntu job and a macOS installer job) have no Rust. No `rust/` directory exists yet. `cargo` exists on the owner's machine.
- Template: `~/codes/rstory` (`release.yml`, `rust-toolchain.toml`, `deny.toml`, `Cargo.toml` with an `include` list, `scripts/test-release-workflows.sh`).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Know what macOS and Windows do before building on them (Priority: P1)

**Why this priority**: D14. The signing decision must precede the real subcommands for those targets; a surprise at 3.0.0 would be expensive.

**Independent Test**: the `rust-matrix` workflow builds the throwaway `ai-gent` (`version`, `doctor`) for all six targets on native runners; `de-risk.md` has every probe row filled in with an observed result and a date, and ends in a signing decision.

**Acceptance Scenarios**:

1. **Given** the throwaway workspace, **When** the matrix runs, **Then** six archives exist and each binary's own smoke step (`ai-gent version --json`) passes on a runner of its OS and architecture.
2. **Given** a macOS arm64 machine, **When** the archive is fetched with `curl`, unpacked with `tar` and run, **Then** the result (runs, or is killed) is recorded; **When** the same archive is fetched with a browser, **Then** the Gatekeeper result is recorded.
3. **Given** a Windows machine, **When** the zip is downloaded by a browser and by `curl.exe`, **Then** the SmartScreen and Smart App Control results are recorded.
4. **Given** the probes are filled in, **When** the owner reads `de-risk.md`, **Then** a decision (ship unsigned with documented workaround, ad-hoc sign, or buy signing) is written with its cost.

### User Story 2 - The workspace builds and is gated like every other part of the repo (Priority: P1)

**Independent Test**: `scripts/check.sh` runs format, clippy with warnings denied, tests and `cargo deny`; a PR that touches no Rust does not pay for them.

**Acceptance Scenarios**:

1. **Given** a clean checkout, **When** `scripts/check.sh` runs, **Then** it runs `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked` and `cargo deny check`, and prints a pass line per gate.
2. **Given** a pull request that changes only a SKILL.md, **When** CI runs, **Then** the Rust job is skipped by its change filter and the PR wall time is not longer than today's.
3. **Given** `cargo` is not installed locally, **When** `scripts/check.sh` runs, **Then** it prints a warning and skips the Rust gates (CI always runs them), the way it treats a missing shellcheck.
4. **Given** a dependency that links OpenSSL or a banned HTTP client, **When** `cargo deny check` runs, **Then** it fails.

### User Story 3 - A release produces one verified package per platform (Priority: P1)

**Independent Test**: `scripts/test-rust-workflows.sh` (shape test of the workflows) plus a dry-run dispatch that builds all targets, packages, checksums and attests nothing and publishes nothing.

**Acceptance Scenarios**:

1. **Given** a release is planned, **When** `release.yml` runs, **Then** it produces, for each of the six targets, `ai-gent-<ver>-<triple>.tar.gz` (a `.zip` on Windows) containing the plugin content and that target's binary, and `SHA256SUMS` lists every archive and `install.sh`.
2. **Given** the archives exist, **When** the package job runs, **Then** every archive and `install.sh` is attested, and the release is published only after all six archives and the checksums are attached.
3. **Given** a manual run of the rehearsal workflow `rust.yml` (`workflow_dispatch`), **When** it finishes, **Then** no tag, release, attestation or crates.io publish happened (dry run only). (`release.yml`'s own dispatch keeps today's behavior: it releases from `main` when the plan says so.)
4. **Given** one matrix leg fails, **When** the run ends, **Then** the draft release stays a draft and `releases/latest` still points at the previous release.

### User Story 4 - The installer fetches the right binary (Priority: P1)

**Independent Test**: `scripts/test-package.sh` and `scripts/test-install.sh` serve fixture archives for each triple over `file://` and check that `uname` combinations pick the right one, a checksum mismatch installs nothing, and the binary is executable at `$AI_GENT_HOME/current/bin/ai-gent`.

**Acceptance Scenarios**:

1. **Given** Linux x86_64, **When** `install.sh` runs, **Then** it fetches the `x86_64-unknown-linux-musl` archive; the same for the other Unix targets (`Linux aarch64`, `Darwin arm64`, `Darwin x86_64`).
2. **Given** an unsupported platform (FreeBSD, 32-bit), **When** `install.sh` runs, **Then** it stops before downloading with a message naming the platform and the supported list.
3. **Given** a checksum mismatch on the archive, **When** `install.sh` runs, **Then** nothing is written under `$AI_GENT_HOME` (today's rule).
4. **Given** an existing 2.26 install, **When** the old `install.sh` runs against a newer release, **Then** it still finds the content-only asset `ai-gent-<ver>.tar.gz` during the 2.x series (see FR-014).

### User Story 5 - The crates are owned and publish without a stored token (Priority: P2)

**Independent Test**: `contracts/workflows.md` checks in `scripts/test-rust-workflows.sh`: no `CARGO_REGISTRY_TOKEN` secret anywhere, `id-token: write` only on the publish job, `environment: crate-release`, the OIDC action pinned by SHA.

**Acceptance Scenarios**:

1. **Given** `ai-gent-core 0.1.0` and `ai-gent 0.1.0` are published and Trusted Publishing is configured for both (2026-11-01, D5; environment `crate-release`, workflow `release.yml`), **When** a dry-run publish job runs and then the first real publish happens, **Then** both succeed with no stored token.
2. **Given** the publish switch is off (the default until the owner turns it on), **When** a release runs, **Then** the publish job is skipped and the rest of the release is unaffected.
3. **Given** a failed publish after a tag, **When** the job is re-run, **Then** it detects the version is already on crates.io and skips instead of failing.

### User Story 6 - The 2.27 release exercises all of it (Priority: P1)

**Independent Test**: the 2.27.0 release run is the test; `quickstart.md` has the pre-merge rehearsal and the rollback.

**Acceptance Scenarios**:

1. **Given** the work is merged into `dev`, **When** the rehearsal dispatch runs on `dev`, **Then** it passes before anything reaches `main`.
2. **Given** the 2.27.0 run fails in the matrix after the tag exists, **When** the owner re-runs the failed jobs, **Then** the draft release is completed or deleted by the documented steps and no user sees a half release.

### User Story 7 - Windows installs natively with `install.ps1` (Priority: P1)

**Why this priority**: D24; native Windows is a 3.0.0 goal and `install.sh` needs a POSIX shell.

**Independent Test**: `scripts/test-install-ps1.ps1` runs on a `windows-2025` job (and `windows-11-arm`) of `check.yml` against a `file://`-style local release base, the way `scripts/test-package.sh` does for `install.sh`.

**Acceptance Scenarios**:

1. **Given** a local release base with the Windows zip and `SHA256SUMS`, **When** `install.ps1` runs, **Then** it verifies the checksum before extracting, unpacks into `%LOCALAPPDATA%\ai-gent\<ver>`, makes `current` point at it (a directory junction, no administrator rights), keeps `previous`, and runs `setup.ps1`.
2. **Given** a checksum mismatch or a zip with an unsafe entry name (`..`, absolute path), **Then** nothing is written under the install root.
3. **Given** `-Version 2.27.0`, **Then** a `pinned` file is written; `-Version latest` clears it (same semantics as `VERSION=` in `install.sh`).
4. **Given** `-Rollback`, **Then** `current` and `previous` swap and `setup.ps1` relinks the agents; `-List` and `-Prune` behave like `install.sh`.
5. **Given** `-VerifyAttestation` and `gh` on `PATH`, **Then** the zip and the script are verified with `gh attestation verify`; without `gh` it says the attestation was not verified.
6. **Given** a fresh Windows user without Git Bash, **When** they run `irm https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.ps1 | iex`, **Then** Claude Code and Codex skills are linked and `ai-gent version` runs from a new terminal.

## Requirements *(mandatory)*

### Functional requirements

- **FR-001**: The workspace is `rust/` with `rust/Cargo.toml` (resolver 3, edition 2024), members `crates/ai-gent-core` (library) and `crates/ai-gent` (binary named `ai-gent`), license `GPL-3.0-or-later`, both crates with `description`, `repository = "https://github.com/lucasvmigotto/ai-gent"`, `homepage = "https://docs.lucasvmigotto.me/ai-gent/"` (D22), `readme`, `keywords`, `categories` and an `include` list that publishes sources only (D4, D6; the `rstory` pattern).
- **FR-002**: The toolchain is pinned in `rust/rust-toolchain.toml` (channel, components `rustfmt` and `clippy`, all six targets); CI, the devcontainer and local runs read that one file.
- **FR-003**: `rust/deny.toml` allows only GPL-3.0-or-later-compatible licenses, denies `openssl`, `openssl-sys`, `native-tls`, `reqwest`, `hyper` and `ureq` (the binary shells out to `curl` for the network, see 034), and denies unknown registries and git sources.
- **FR-004**: `[workspace.lints]` forbid `unsafe_code`, warn on `clippy::pedantic` subsets named in `plan.md`, and CI fails on any warning.
- **FR-005**: `scripts/check.sh` gains a Rust section (fmt, clippy, test, deny) that is skipped with a warning when `cargo` is absent and with `--quick` skips only the slow tests; the CI `check` workflow runs it behind a change filter on pull requests and always on `main`.
- **FR-006**: The build matrix is six native legs: `x86_64-unknown-linux-musl` (ubuntu-24.04), `aarch64-unknown-linux-musl` (ubuntu-24.04-arm), `aarch64-apple-darwin` (macos-15), `x86_64-apple-darwin` (macos-15-intel), `x86_64-pc-windows-msvc` (windows-2025) and `aarch64-pc-windows-msvc` (windows-11-arm) [RESEARCHED: S2]. No cross-compilation toolchain (zig, cross) and no `cargo-dist` (reasons in `plan.md`).
- **FR-007**: Windows binaries link the C runtime statically (`+crt-static`) so no Visual C++ redistributable is needed [KNOWN].
- **FR-008**: One release package **per target**, `ai-gent-<ver>-<triple>.tar.gz` (`.zip` for Windows), holding the plugin content exactly as today's package plus `bin/ai-gent[.exe]`; `package.py` gains `--target <triple> --binary <path>` and stays deterministic.
- **FR-009**: The content-only `ai-gent-<ver>.tar.gz` keeps being published for every 2.x release (so a 2.26 installer upgrading in place works) and is no longer published from 3.0.0.
- **FR-010**: `SHA256SUMS` lists every archive and `install.sh`; the package job attests all of them (`actions/attest-build-provenance`, already used for the tarball).
- **FR-011**: `install.sh` maps `uname -s` and `uname -m` to a triple, fetches that archive, verifies it with the existing rules, and installs `bin/ai-gent` under `$AI_GENT_HOME/<ver>/bin/`; `setup.sh` links `~/.local/bin/ai-gent` to `$AI_GENT_HOME/current/bin/ai-gent` when that directory is on `PATH` or tells the person to add it.
- **FR-012**: The release workflow's Rust jobs are a reusable workflow `rust-build.yml` (inputs: ref, release version, dry run), called by `release.yml` and by the rehearsal workflow `rust.yml`; `rust.yml` runs (pull request or `workflow_dispatch`) never tag, release, attest or publish.
- **FR-018**: Before every publish, the metadata (homepage, repository, license, readme, keywords, categories) is verified in the output of `cargo publish --dry-run` (`cargo package --list` and the packaged `Cargo.toml`); crates.io reads it from each published version.
- **FR-019**: `install.ps1` (D24) is published beside `install.sh` (asset, `SHA256SUMS` line, attested). It runs on Windows PowerShell 5.1 and PowerShell 7 [ASSUMPTION: 5.1 compatibility to be proven by the windows test job], is wrapped in a function called on its last line (a partial download never executes), and mirrors `install.sh` verbatim in behavior: checksum before anything is written, versioned directory, `current` and `previous`, `pinned`, `-List`, `-Rollback`, `-Prune`, `-VerifyAttestation`, environment `AI_GENT_HOME` (default `%LOCALAPPDATA%\ai-gent`) and `AI_GENT_RELEASE_BASE` (an https URL, `file://` URL or a plain directory).
- **FR-020**: On Windows `current`, `previous` and the agent skill links are directory junctions (`New-Item -ItemType Junction`), created per user without administrator rights or Developer Mode [KNOWN]; flipping `current` creates `current.new`, removes the old junction without touching its target, and renames `current.new` (a short window without `current` is recovered from `previous` by `-Rollback`; documented).
- **FR-021**: The Windows archive cannot carry symlinks: `scripts/package.py --target *-windows-msvc` replaces every symlink of the package (the 77 `references/` links) with a copy of its target and still refuses a dangling or escaping one.
- **FR-022**: `setup.ps1` (the Windows `setup` equivalent) links the skills of tiers 1 and 2 (Claude Code and Codex) and creates the `ai-gent` PATH entry (user `PATH`, no admin); tier-3 agents on Windows use Git Bash or WSL with `setup.sh`. The set of links mirrors what `setup.sh` does for those two agents (inventory task T052).
- **FR-023** (D25): the release notes carry the SHA256 of `install.sh`, `install.ps1` and every package archive, appended by the package job after upload; the docs install page tells readers to prefer the GitHub Releases URL (task in 039).
- **FR-017**: `shared/secrets.md` documents the entry `ai-gent/crates.io/pat` (area `crates.io`): read only by a runner such as `pass-run` for a manual publish; the repository has no `pass-run` of its own (the owner's is dottod's `scripts/pass-run.sh`), so a contributor without it publishes nothing by hand: every publish after the placeholders goes through Trusted Publishing from `release.yml`, and a one-off manual publish is the owner's.
- **FR-013**: Publishing uses crates.io Trusted Publishing from `release.yml` in the `crate-release` environment through `rust-lang/crates-io-auth-action` pinned by SHA; `id-token: write` is held by that job only; no token is a repository secret; the assistant never receives or handles a token (D5). The publish job is gated by a repository variable `AI_GENT_CRATES_PUBLISH == 'true'`, default unset.
- **FR-014**: Crate versions (D23). The `ai-gent` binary crate is in lockstep with the repo tag: `scripts/release.py` writes the release version into `rust/crates/ai-gent/Cargo.toml` and the matching `Cargo.lock` entry in the release commit, so `ai-gent --version` equals the package `VERSION` file and the tag (the first lockstep version is 2.27.0, jumping up from 0.1.0). `ai-gent-core` has independent SemVer, bumped by `release.py` from conventional commits that touch `rust/crates/ai-gent-core` (while below 1.0.0: a breaking commit bumps the minor, anything else that releases bumps the patch); 1.0.0 comes no earlier than 3.0.0 and only when the API is deliberately stable. `release.py --dry-run [--json]` previews both; a check fails the release when the binary crate's `ai-gent-core` version requirement does not match the core version being published. `AI_GENT_RELEASE` is still embedded by the workflow for the version output of builds outside a release.
- **FR-015**: `ai-gent version [--json]` and `ai-gent doctor [--json]` exist in 2.27 as the throwaway pair; their output schema is in `contracts/cli.md`.
- **FR-016**: Shape tests for the new workflows live in `scripts/test-rust-workflows.sh` (pure bash, greps the real workflows with comments stripped, like `rstory`'s) and run in `scripts/check.sh`.

### Key entities

See `data-model.md`: Target, Archive, Release asset set, Install root, Crate, Release version vs crate version.

## Edge cases

- A user on macOS 11 or Windows 8: the minimum OS versions are an open question; until answered the binary refuses to start on an older OS only by the OS's own loader error, and `install.sh` prints the supported minimums.
- Alpine and other musl distros: the Linux binary is statically linked against musl so it runs on both glibc and musl [KNOWN].
- Rosetta: an Intel Mac binary runs on Apple Silicon under Rosetta; `install.sh` picks by `uname -m`, which reports `x86_64` for a shell started under Rosetta; it prefers `arm64` via `sysctl -n hw.optional.arm64` when available.
- Windows without bash: `install.ps1` (D24, user story 7) is the main route; Git Bash or WSL with `install.sh` is a documented alternative.
- A matrix leg is flaky: the leg is re-run; the release does not skip it.
- `Cargo.lock` out of step with a bumped version: `--locked` fails in CI; the release job must update the lock.

## Out of scope

- Any subcommand beyond `version` and `doctor` (033-038).
- Code signing and notarization themselves: D14 decides; this spec only produces the evidence and wires a signing step if the decision is to sign.
- A Rust `setup` subcommand (the Windows `setup.ps1` covers tiers 1-2; see open question 4).
- Homebrew, winget, apt, nix and container image distribution.
- Telemetry (none planned).

## Success criteria *(mandatory)*

- **SC-001**: `de-risk.md` has no empty probe row and ends in a signed-off decision before any spec of a subcommand targeting macOS or Windows is accepted.
- **SC-002**: `scripts/check.sh` (full) passes with the Rust gates, and a PR without Rust changes shows the Rust job skipped; wall time of the `check` job on such a PR is within 10 percent of the pre-change median.
- **SC-003**: A release run produces 6 target archives + the content-only archive + `install.sh` + `SHA256SUMS`, each attested, and `gh attestation verify` passes for each.
- **SC-004**: `scripts/test-rust-workflows.sh` and `scripts/test-package.sh` pass, including a per-triple install matrix over `file://`.
- **SC-005**: A rehearsal dispatch on `dev` completes with zero tags, releases or publishes created.
- **SC-006**: `grep -rn 'CARGO_REGISTRY_TOKEN' .github` finds only the step output wiring of the auth action, no secret reference.
- **SC-007**: The 2.27.0 release completes with all six archives attached and `install.sh` on a clean Linux container installs and runs `ai-gent version --json`.

## Open questions (for the owner)

8. **`documentation` for the `ai-gent` binary crate.** Unset on `ai-gent-core` (docs.rs, D22). Recommended for `ai-gent`: leave it unset too, so crates.io links docs.rs, which shows only a thin binary crate; or set it to `https://docs.lucasvmigotto.me/ai-gent/` so the page that explains the product is one click from the crate. Recommendation: set it on the binary, since users of a binary read the docs site, not API docs.

7. **Workflow file name.** crates.io was configured with `release.yml`; this spec keeps publishing from `release.yml` (job `publish-crates`). If a different workflow file were ever needed, the owner would have to reconfigure both crates; none is planned.

1. **Minimum OS versions** (decisions file, open). Recommended: macOS 12 (`MACOSX_DEPLOYMENT_TARGET=12.0`), Windows 10 version 1809 or later, Linux any kernel 3.2+ (static musl). [ASSUMPTION: verify the macOS floor against what `macos-15` runners and Rust's tier-1 policy accept.]
2. **When do real crate versions start publishing?** Recommended: keep `AI_GENT_CRATES_PUBLISH` unset through 2.28 (placeholders only) and turn it on with 3.0.0, when the binary is the product.
3. **Signing budget.** Decided after the probes in US1; the recommendation is written into `de-risk.md` then. Not asked before the evidence exists.
4. **`setup.ps1` scope.** D24 puts `install.ps1` in 3.0.0. Recommended: `setup.ps1` covers Claude Code and Codex only (tiers 1-2); tier 3 on Windows goes through Git Bash or WSL; a Rust `ai-gent setup` that replaces both `setup.sh` and `setup.ps1` is a 3.x spec.
5. **Is the repository public** (needed for free arm64 and Intel macOS runners)? [ASSUMPTION: public; verify before the matrix runs.]
6. **Who can run the interactive Gatekeeper and SmartScreen probes?** CI cannot. Recommended: the owner on one Mac and one Windows VM or machine, using the probe sheet.
