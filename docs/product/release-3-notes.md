# Release 3.0.0 notes: what the 2.26 package install taught us

2.26 made the release package the default install and kept the git clone as
a flagged dev mode. The sections below record what that taught and what 3.0.0
changes in consequence. The changes are decided: the contract is
`specs/039-release-3-contract/` (`contracts/breaking-changes.md` lists them as
B1-B12, `contracts/migration-guide.md` is the user text), with the decisions in
`docs/product/rust-decisions.md` (D1-D30).

## What 2.26 learned

- A package is small (about 650 KB, 350 files) and the symlink layout survives
  a tarball: the 77 relative `references/` links resolve inside it, and
  `package.py` refuses to build if one would not.
- The update hook (`plugins/git/hooks/hooks.json`) reaches `../../scripts/`
  outside its plugin. It works in the package only because the script is
  shipped at that relative path. It now no-ops when the path is missing.
- Two install roots cannot share a path: the old clone and the package both
  want `~/.local/share/ai-gent`. The installer refuses and `setup.sh --migrate`
  moves links across.
- The checksum comes from the same host as the tarball, so it guards
  corruption; the attestation is what guards a tampered asset, and it is
  optional because it needs `gh`.
- `releases/latest` must never point at a release without a package, so the
  Release is created as a draft and published by the package job.
- An explicit `VERSION=x.y.z` writes a `pinned` marker in the install root
  and update-check stays silent; `latest` clears it. `SHA256SUMS` lists the
  tarball and `install.sh`, which is also attested.
- `skills/skill-docs` is a contributor skill and is not in the package.

## What 3.0.0 would change (breaking)

Decided. Each item below is a numbered change in
`specs/039-release-3-contract/contracts/breaking-changes.md`: git mode is B1,
links into `current/` B2, contributors in a checkout B3, tier-3 best-effort B10.
Windows is decided too: a native installer and native guards (D24; specs 032 and
033), with WSL or Git Bash as an alternative. The last bullet is kept as the
question it answered.

- Remove git mode (`AI_GENT_MODE=git`, `AI_GENT_BRANCH`, `AI_GENT_REPO`,
  `AI_GENT_DIR`) and the clone-specific branches of `ai-gent-update.sh` and
  `update-check.sh`.
- Links in the agents' skill directories always lead into `.../current/`;
  no live edit in place. Contributors work in a repo checkout and run
  `setup.sh` from it.
- Tier-3 agents (opencode, Cursor, Antigravity, Copilot, Amp, Kiro, Windsurf)
  best-effort and out of the required CI.
- Windows: the installer, `setup.sh` (bash 4), the five `guard.sh` hooks and
  `update-check.sh` are POSIX sh or bash and need WSL or Git Bash; the
  tarball's symlinks need privileges on native Windows. Decide whether native
  Windows is a goal before choosing a portable guard.
