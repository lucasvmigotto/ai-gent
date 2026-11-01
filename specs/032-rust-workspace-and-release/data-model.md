# Data model: Rust workspace and release

No database. The entities are build and install artifacts.

| Entity | Where | Fields |
|---|---|---|
| Target | `rust/rust-toolchain.toml`, matrix in `rust-build.yml` | triple, runner label, archive extension, smoke command |
| Binary | `rust/target/<triple>/release/ai-gent[.exe]` | crate version, release version (`AI_GENT_RELEASE`), triple, commit, size |
| Archive | `dist/ai-gent-<ver>-<triple>.tar.gz` or `.zip` | top dir `ai-gent-<ver>-<triple>/`, `bin/ai-gent[.exe]`, plugin content, `MANIFEST`, `VERSION`, `LICENSE`, `README.md` |
| Content-only archive | `dist/ai-gent-<ver>.tar.gz` | as today (2.x only) |
| `SHA256SUMS` | release asset | `<sha256>  <asset name>` lines for each archive and `install.sh` |
| `LATEST` | `package.py --out` (file:// releases) | the version string |
| Install root | `$AI_GENT_HOME` (default `${XDG_DATA_HOME:-~/.local/share}/ai-gent`; Windows `%LOCALAPPDATA%\ai-gent`, where `current` and `previous` are junctions) | `<ver>/`, `current`, `previous`, `pinned` (shared with 034) |
| PATH link | `~/.local/bin/ai-gent` | symlink to `$AI_GENT_HOME/current/bin/ai-gent` |
| Crate | crates.io | name, version (`ai-gent`: lockstep with the tag; `ai-gent-core`: independent SemVer, D23), owners, Trusted Publishing config (repo, workflow file `release.yml`, environment `crate-release`) |

## Version relations (D23)

- Tag and `VERSION` file: `X.Y.Z` of the release (no `v`).
- `ai-gent` (binary crate): equals the tag, written by `release.py` into its `Cargo.toml` and `Cargo.lock` entry.
- `ai-gent-core`: independent SemVer in its own `Cargo.toml`, bumped by `release.py` from commits touching `rust/crates/ai-gent-core`; below 1.0.0 a breaking commit is a minor bump. The binary's dependency line `ai-gent-core = { path = "../ai-gent-core", version = "<requirement>" }` must match the core version published in the same release (checked).
- Publishing order: core first, wait for the index, then the binary.
- `ai-gent --version` prints `ai-gent <X.Y.Z> (<triple>)`, and `<X.Y.Z>` equals the package `VERSION` (the "version compatibility" open item of the decisions file is thereby answered); `doctor` fails when it differs from the install's `VERSION` file. Contract text in `contracts/cli.md` keeps a `release` field, equal to the crate version for release builds and `dev` otherwise.

## Triple to platform map (used by `install.sh`)

| `uname -s` | `uname -m` | Triple |
|---|---|---|
| Linux | x86_64 | `x86_64-unknown-linux-musl` |
| Linux | aarch64, arm64 | `aarch64-unknown-linux-musl` |
| Darwin | arm64 | `aarch64-apple-darwin` |
| Darwin | x86_64 (and no `hw.optional.arm64`) | `x86_64-apple-darwin` |
| MINGW*/MSYS*/CYGWIN* | AMD64/x86_64 | `x86_64-pc-windows-msvc` (zip; via Git Bash `install.sh`) |
| Windows PowerShell (`install.ps1`) | `AMD64` / `ARM64` | `x86_64-pc-windows-msvc` / `aarch64-pc-windows-msvc` |
| other | any | refuse with the supported list |

## Size budget (to be measured in T015)

Binary at most 8 MB stripped per target [ASSUMPTION]; archive at most 10 MB; release total at most 80 MB. If exceeded, review dependencies before raising the budget.
