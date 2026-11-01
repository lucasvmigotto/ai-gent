# v3 notes: what the 2.26 package install taught us

2.26 makes the release package the default install and keeps the git clone as
a flagged dev mode. This is a test of the idea, not a plan.

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
- `skills/skill-docs` is a contributor skill and is not in the package.

## What v3 would change (breaking)

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
