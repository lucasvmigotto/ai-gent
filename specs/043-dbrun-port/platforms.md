# Platform matrix: `ai-gent-dbrun` engine features per release target

One row per engine feature, one column per release target of spec 032 (`rust/rust-toolchain.toml`). A cell is **run** (what was built, linked and executed, with the evidence), or **not run** with the reason. A failing pair becomes a waiver only with the owner's signature (FR-029, FR-035); SQLite and the binary itself are not waivable (FR-029). Windows, macOS and aarch64 are untested and cannot be tested now; that is disclosed in the release notes, not gated (owner decision (chat 2026-11-14)). Rows for the other engines are added by their groups (T071, T091).

## `sqlite` (T023)

`rusqlite` 0.41.1 with `bundled`: `libsqlite3-sys` 0.39.1 compiles the SQLite amalgamation (SQLite 3.54.1) with the target's C compiler through `cc-rs`, so every target needs a C compiler for that target. Recorded 2026-11-14 by worker B on a Debian 13 x86_64 laptop (gcc 14.2.0, rustc 1.98.0, no musl, cross or MSVC toolchain installed).

| Target | Result | Evidence or reason |
|---|---|---|
| `x86_64-unknown-linux-gnu` (host, not a release target) | run: builds, tests pass, `_exec` runs | `cargo test -p ai-gent-dbrun --features sqlite`; `scripts/test-dbrun-engines.py --runner rust/target/debug/examples/sqlite_exec --engine sqlite` (8 of 8 journeys) |
| `x86_64-unknown-linux-musl` | not run to the end: fails here for want of a musl C compiler | `cargo build -p ai-gent-dbrun --features sqlite --lib --release --target x86_64-unknown-linux-musl`: `cc-rs` finds no `x86_64-linux-musl-gcc`. With `CC_x86_64_unknown_linux_musl=gcc` the library compiles, but linking the `sqlite_exec` example fails: the amalgamation compiled against glibc headers calls `open64`, `stat64`, `fstat64`, `ftruncate64`, `fcntl64`, which musl does not define. Needs a real musl toolchain (`musl-tools`' `musl-gcc`, a `x86_64-linux-musl` cross gcc, or `cargo zigbuild`) in the release job; not decided here (T091). Matches the spike (038): "musl failed for sqlite (no musl C compiler)". |
| `aarch64-unknown-linux-musl` | not run: no `aarch64-linux-musl-gcc` on this host | `cc-rs` stops at "failed to find tool aarch64-linux-musl-gcc"; the host gcc cannot target aarch64. Same need as above, for aarch64. |
| `aarch64-apple-darwin` | not run: no Apple SDK or clang for the target on this host | Needs a macOS runner (CI's `macos-*`), where the system clang compiles the amalgamation. |
| `x86_64-apple-darwin` | not run: as `aarch64-apple-darwin` | Needs a macOS runner. |
| `x86_64-pc-windows-msvc` | not run: no MSVC (`cl.exe`) on this host | Needs a Windows runner with the MSVC build tools. Run here: `cargo check -p ai-gent-dbrun --lib --target x86_64-pc-windows-msvc` (without `sqlite`, which needs the C compiler even to check) passes, so the `cfg(not(unix))` and no-feature paths of the crate type-check for Windows. Windows-aware points checked in the code instead: no path is built with `/` (paths are `Path::join`), a file name is percent-encoded into the `file:` URI byte by byte (`%`, `?`, `#` and non-ASCII bytes; whether SQLite's URI parser takes a drive path such as `file:C:\dir\app.db` as is, unverified until a Windows run), the request reaches the child on stdin (no command-line length limit), the FIFO and symlink tests are `#[cfg(unix)]`, and the owner-only file mode is Unix only (the Windows ACL is a later item, owner decision (chat 2026-11-14)). |
| `aarch64-pc-windows-msvc` | not run: as `x86_64-pc-windows-msvc` | Needs a Windows arm64 runner or the MSVC arm64 cross tools. |

Not a waiver: SQLite cannot be waived (FR-029), so the musl result is a release-job requirement (a musl C compiler), to be met in T091, not a signed exception.
