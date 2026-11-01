# Quickstart: how this is proven and run

## Local, any contributor

```sh
cd rust && cargo build --release --locked && ./target/release/ai-gent version --json
scripts/check.sh --quick --quiet        # includes the Rust gates when cargo exists
python3 scripts/package.py --check      # package builds; no Rust sources inside
scripts/test-rust-workflows.sh          # workflow shape
```

## Rehearsal (before anything reaches `main`)

1. Push the branch to `dev`. Run `gh workflow run rust.yml --ref dev`.
2. Expect six green legs, six `bin-<triple>` artifacts, `package.py --check --with-binaries` green, and in the run: no tag, no release, no attestation, no publish.
3. Install from the rehearsal artifacts into a scratch home: serve `dist/` over `file://` and `AI_GENT_RELEASE_BASE=file://... AI_GENT_HOME=$(mktemp -d) sh install.sh --skip-prompt --target claude`.

## Owner steps (the assistant never touches a token)

A contributor without `pass-run` (this repository has none; dottod's is the owner's) does not publish: later publishes are Trusted Publishing from `release.yml`, and a manual one is the owner's alone.


1. Already done (2026-11-01, D5): placeholders `ai-gent-core 0.1.0` and `ai-gent 0.1.0` are published, through `pass` with the entry `ai-gent/crates.io/pat` handed to `cargo publish` by a runner; no value reached a transcript.
2. Already done by the owner: crates.io Trusted Publishing for both crates (repository `lucasvmigotto/ai-gent`, workflow file `release.yml`, environment `crate-release`). Verify with a dry run, then the first real publish.
3. On GitHub: Settings, Environments, confirm `crate-release` exists (owner action; a required reviewer is optional).
4. Leave the repository variable `AI_GENT_CRATES_PUBLISH` unset. Set it to `true` only when open question 2 is answered.
5. Optional, later: on crates.io turn on "Trusted Publishing only" for each crate [RESEARCHED: S1].

## Cutting 2.27

Normal flow (`shared/releasing.md`): merge `dev` into `main` by PR; the run does check, release, rust-build, package. If a matrix leg fails after the tag exists: fix, then "Re-run failed jobs" on the same run (the draft release and tag are reused). If the tag itself is wrong, the owner deletes the draft release and the tag by hand and the next push releases again; never move a published tag.

## Rollback

A bad published release: `sh install.sh --rollback` for users (flips `current`); for the project, publish a fixing release (patch). Nothing is yanked from crates.io while the publish switch is off.
