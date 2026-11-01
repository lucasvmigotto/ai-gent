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
| Install root | `$AI_GENT_HOME` (default `${XDG_DATA_HOME:-~/.local/share}/ai-gent`; Windows `%LOCALAPPDATA%\ai-gent`) | `<ver>/`, `current`, `previous`, `pinned` (shared with 034) |
| PATH link | `~/.local/bin/ai-gent` | symlink to `$AI_GENT_HOME/current/bin/ai-gent` |
| Crate | crates.io | name, version (independent SemVer), owners, Trusted Publishing config (repo, workflow file `release.yml`, environment `crate-release`) |

## Version relations

- Tag and `VERSION` file: `X.Y.Z` of the release (no `v`).
- Crate version: independent SemVer in `rust/Cargo.toml` `[workspace.package]`, bumped by `release.py` on commits touching `rust/`.
- `ai-gent --version` prints `ai-gent <crate> (release <AI_GENT_RELEASE>, <triple>)`; `doctor` fails when the release differs from the `VERSION` file of the install it lives in (open item "version compatibility" of the decisions file, answered here).

## Triple to platform map (used by `install.sh`)

| `uname -s` | `uname -m` | Triple |
|---|---|---|
| Linux | x86_64 | `x86_64-unknown-linux-musl` |
| Linux | aarch64, arm64 | `aarch64-unknown-linux-musl` |
| Darwin | arm64 | `aarch64-apple-darwin` |
| Darwin | x86_64 (and no `hw.optional.arm64`) | `x86_64-apple-darwin` |
| MINGW*/MSYS*/CYGWIN* | AMD64/x86_64 | `x86_64-pc-windows-msvc` (zip; binary only until open question 4) |
| other | any | refuse with the supported list |

## Size budget (to be measured in T015)

Binary at most 8 MB stripped per target [ASSUMPTION]; archive at most 10 MB; release total at most 80 MB. If exceeded, review dependencies before raising the budget.
