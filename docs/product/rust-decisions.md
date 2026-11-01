# Rust and 3.0.0: decisions log

Status: **decided by the owner** unless marked *assistant's call* or *open*.
This file is the single source for the Rust and 3.0.0 specs (`specs/032`–`specs/040`):
a builder or a fresh session starts here. Date: 2026-11-01. Release at the time: 2.26.0.

## Why

Ship one binary (`ai-gent`) in the release package that replaces the runtime
Python and shell tools, so a user installs once and needs neither `uv` nor
Python. The reasons are distribution, native Windows and credibility (a
published crate), not speed or memory safety: the tools are I/O-bound.
Most of ai-gent's value is Markdown (skills and references); the scripts are
assistant tools for code agents.

## Decisions

| # | Decision | Owner call |
|---|---|---|
| D1 | The release package becomes the **only** install path in 3.0.0. 2.26 shipped it as the default with the git clone as a flagged dev mode. | owner |
| D2 | 3.x supports tiers **Claude > Codex > others**. Tier 3 (opencode, Cursor, Antigravity, Copilot, Amp, Kiro, Windsurf) is **best-effort and frozen**, out of the required CI; nothing is removed in 3.0.0. | owner delegated ("your call") |
| D3 | Native **Windows is a goal** (also macOS). Targets: x86_64 and aarch64 for Linux, macOS and Windows. | owner |
| D4 | The Rust workspace lives **in this repo under `rust/`**: crates `ai-gent-core` (library) and `ai-gent` (binary), plus `ai-gent-dbrun` (library) from D28. One release flow, one tag. | owner |
| D5 | Crate names `ai-gent` and `ai-gent-core` (both free on crates.io on 2026-11-01; `aigent` is taken). Placeholder **0.1.0** of both is **published** (done 2026-11-01, with the owner's local token handed to the command by a runner, so no value reached any transcript; that token is not documented in the project and is never used again). The owner has configured Trusted Publishing on crates.io for both crates (workflow `release.yml`, GitHub environment `crate-release`); `ai-gent-dbrun` (D28) is a third name, also published as 0.1.0 on 2026-11-01, whose Trusted Publishing the owner still has to set up; every later publish goes through Trusted Publishing from `release.yml`, with no stored token. The assistant never asks for or handles a token. | owner |
| D6 | License **GPL-3.0-or-later** (the repo's). | repo |
| D7 | **Migration per subcommand, cut over at 3.0.0** (amended 2026-11-01): a subcommand ships in 2.27/2.28 **beside** its Python or shell tool, with parity tests proven in the package; skills switch to `ai-gent <sub>` and the Python and shell files are deleted together at 3.0.0 (package-only install). Guards: the evaluator goes shadow mode first, then decides from 2.28 (D12); hooks call it from 2.28 for package installs only (a git clone keeps the shell guard until git mode is removed). No dual path survives 3.0.0. | owner |
| D8 | A missing binary **fails with the install command**; there is no Python fallback. | owner |
| D9 | One binary with subcommands and `--json` everywhere (`guard`, `wt`, `tokens`, `repo`, `spec`, `compact`, `update`, `doctor`, later `db`). Stable `--json`, documented exit codes. | owner |
| D10 | **Parity gate**: the Python tool's existing tests (about 3.5k lines) become shared fixtures run against both; the Rust tool passes all of them plus new Windows cases before the Python file is deleted. | owner |
| D11 | **Guards on Windows parse sh, PowerShell and cmd** commands, with fixtures for all three. | owner |
| D12 | The guard evaluator ships in **shadow mode first** (runs beside the shell guard, logs disagreements, the shell guard decides). After a minor release with no disagreements the evaluator decides and the shell guards go. | owner |
| D13 | opencode's guard **calls the binary** and `opencode/guard/rules.mjs` is dropped, so one implementation decides for every agent. | owner |
| D14 | De-risk macOS and Windows **early**: in 2.27 build and ship a throwaway `ai-gent` (version and doctor only) on macOS and Windows runners and see what Gatekeeper and SmartScreen do unsigned; the signing decision is made before the real subcommands are specified for those targets. | owner |
| D15 | **Update is notify-only.** `ai-gent update --check` prints a notice only when a newer release is cached, never blocks, refreshes detached on an interval (default 7 days), honours `AI_GENT_NO_UPDATE_CHECK`, and stays silent for a pinned version. `ai-gent update` installs through the same verified path as `install.sh`; `--rollback` goes back. No silent self-replacement (the binary runs the guards). Pattern: `~/codes/dottod/scripts/.update.sh`. | assistant's call, owner delegated |
| D16 | `install.sh` and `setup.sh` stay a short POSIX bootstrap (they run before any binary exists). Dev-only tools stay Python/shell and are not shipped: `release.py`, `consistency.py`, `run-evals.py`, `package.py`, `sbom-from-bunlock.py`, `check.sh`, all `test-*.sh` and `test_*.py`. | owner |
| D17 | Planning material is **Spec Kit specs under `specs/`**, full depth (spec, plan, tasks, fixtures) for every piece, so a session that runs out of tokens leaves resumable work. | owner |
| D18 | A **docs-site v2 spec** (3.0.0 Install page, tier compatibility matrix, search; no full redesign) is part of the planning set, built after the Rust pieces. | owner |
| D19 | `dbrun` is last or never: its drivers come from `uv run --with` or a container, so a Rust port needs native drivers or a container shell-out. A spike decides. | assistant's call |
| D20 | A **major bump** is earned by the breaking contract: package-only install, tiers in force, the binary replacing the shell guards and the first Python tools. Not by packaging alone. | analysis, owner agreed to discuss |
| D21 | Version naming: 3.0.0 / 3.x, never "v3"; tags carry no `v` prefix. | owner |
| D22 | Crate homepage is the docs site: `homepage = "https://docs.lucasvmigotto.me/ai-gent/"` and `repository = "https://github.com/lucasvmigotto/ai-gent"` on every crate; `documentation` unset on `ai-gent-core` (docs.rs API docs). crates.io reads these per published version, so they land with the first real release (the 0.1.0 placeholders keep the repository as homepage and cannot be edited). | owner |
| D23 | Crate versioning: the `ai-gent` binary crate is in lockstep with the repo tag (2.27.0, 2.28.0, 3.0.0; `ai-gent --version` equals the package `VERSION` file and the tag; jumping up from 0.1.0 is fine). `ai-gent-core` has independent SemVer: 0.x while its API is unstable (a minor bump may break), 1.0.0 no earlier than 3.0.0 and only when the API is deliberately stable. `scripts/release.py` writes the versions (never by hand); `ai-gent-dbrun` (D28) has independent SemVer too, on the same terms; the release job publishes core first, then `ai-gent-dbrun`, then the binary. | owner |
| D24 | A native Windows installer, `install.ps1`, is **in 3.0.0**: the same verification as `install.sh` (checksum before extracting, optional attestation, versioned directory plus `current`, rollback, pin). WSL or Git Bash stays documented as an alternative, not the main route. | owner |
| D25 | Docs hosting stays on R2 for 3.0.0. Each release's notes publish the SHA256 of `install.sh`, `install.ps1` and the packages so they can be checked outside any bucket; the install page says to prefer the GitHub Releases URL. | owner |
| D26 | `dbrun` is in scope for a full migration within 3.x, as the last step of the roadmap; a research and prototype spike (spec 038) decides how. It is not a 3.0.0 gate unless the spike says otherwise. Supersedes the "or never" of D19. | owner |
| D27 | The dbrun port is `3.x`, not necessarily 3.0.0; the README says plainly which tools still need `uv` and Python at the time of 3.0.0, if any. | owner (info) |
| D28 | **Crate layout for dbrun** (2026-11-01, from the spike report `docs/product/dbrun-spike-report.md`). `ai-gent-core` stays lean and must not depend on any database driver. A library crate `ai-gent-dbrun` (name published as 0.1.0 on 2026-11-01; Trusted Publishing is for the owner to set up under the `crate-release` environment) holds the safety core (scanner, splitter, classifier, cost rules, masking, redaction, plan engine, logging), a `Connector` trait, and one module per engine behind Cargo features `sqlite`, `postgres`, `mysql`, `mssql`, `oracle`, including a process connector that drives the Python `_exec` helper (how the hybrid option H keeps Oracle, and SQL Server until a native driver proves itself). The `ai-gent` binary depends on it with default features per target (for example no `oracle` on Windows). No crate per connector in 3.0.0: a connector splits out only when (a) it needs a different release cadence or licence, (b) an outside user wants it alone, or (c) its dependency tree makes the shared crate hard to build; a fork-dependent `mssql` or the `oracle` connector is the likeliest first split, after the trait has been exercised by at least two native connectors. Its README says it is built for ai-gent's own policy and is not a general SQL validator (sqlparser, measured as the classifier: 163 of 1026 disagreements, 119 where it says READ and dbrun says WRITE). Version: independent SemVer like `ai-gent-core` (D23). Specs: 038 ("Crate layout"), 032 (release order, `release.py`). | owner |

## Constraints every spec must respect

- The repo rules in `AGENTS.md` and `CLAUDE.md` apply (propose-first, `scripts/check.sh`, releases are automated, no version bumps by hand).
- The release pipeline template is `~/codes/rstory` (musl static builds for x86_64 and aarch64 Linux, crates.io Trusted Publishing through OIDC with no stored token, a tag/version/commit verification job, manual runs are dry runs only, a pinned toolchain in `rust-toolchain.toml`, `deny.toml`, an `include` list so the published crate holds sources only). Reuse its patterns; macOS and Windows are new.
- The shell guards (`plugins/*/hooks/guard.sh`) and `opencode/guard/rules.mjs` must keep deciding the same until the evaluator replaces them; `scripts/test-*guard.sh` and the opencode parity test are the fixture source.
- Skills and the guards reference the Python tools by file name (about 73 `wt.py`, 52 `token_report.py`, 18 `reporun.py`, 18 `compact.py`, 11 `dbrun.py`, 6 `specsync.py`); a cutover changes those references and the guard patterns in the same release.
- Secrets: pass first, only a runner reads a value (`shared/secrets.md`). The binary never prints a secret.
- Container resources go by ID or label, never a prune (`shared/containers.md` §9a).
- The package allowlist (`scripts/package.py`) must ship the binary and exclude Rust sources and `target/`.

## Release plan (proposed, not decided)

- **2.27**: the `rust/` workspace, the placeholder crates (0.1.0), CI and release matrix with the macOS and Windows throwaway binary, the guard evaluator in shadow mode (Linux first).
- **2.28**: the guard evaluator decides and opencode calls it; `update` and `doctor`; the first ported tools.
- **3.0**: package-only install, tier policy in force, shell guards and the ported Python tools removed, docs v2.

## Open (to ask the owner when their turn comes)

- Code-signing material and cost for macOS notarization and Windows signing (after the D14 spike).
- Minimum supported OS versions and glibc/musl choice per target.
- Whether `ai-gent` should also expose `--version` compatibility with the package `VERSION` file.
- Telemetry: none planned; confirm.
