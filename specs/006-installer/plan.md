Reconstructed by project:introspec on 2026-10-18 from 384c771

# Implementation Plan: Installer (install.sh and setup.sh)

**Branch**: `docs/introspec-site-runners` | **Date**: 2026-10-18 | **Spec**: `spec.md`

## Summary

`install.sh` (POSIX sh) owns the clone; `setup.sh` (bash) owns the per-agent install, driven by the `AGENT_IDS` registry with a label, detection probes and a directory per agent [OBSERVED: setup.sh:70-84]. opencode's JSON config is edited through `scripts/opencode-config.py` [OBSERVED: setup.sh:502].

## Technical Context

**Language/Version**: POSIX sh (`install.sh`), bash ≥ 4 (`setup.sh`), Python 3 for the opencode config helper.

**Primary Dependencies**: git, bash; optional fzf; python3 when opencode is chosen.

**Storage**: `${XDG_STATE_HOME:-~/.local/state}/ai-gent/targets`, `opencode-config.json`, `opencode-config-created` [OBSERVED: setup.sh:56-57,492-494].

**Testing**: `scripts/test-install.sh` — throwaway HOME, 70 checks, run by `scripts/check.sh` (skipped under `--quick`) and in CI [OBSERVED: scripts/check.sh:308-314].

**Target Platform**: Linux [ASSUMPTION].

**Project Type**: CLI scripts.

## Constitution Check

N/A — no constitution. Checked against `CLAUDE.md`: adding an agent means label, probes, directory with env override and a test case ✓ (all present for the nine agents).

## Project Structure

```text
install.sh
setup.sh
scripts/opencode-config.py      (not reconstructed here; covered by test-install.sh)
scripts/test-install.sh
opencode/guard/                 (the plugin setup.sh registers)
```
