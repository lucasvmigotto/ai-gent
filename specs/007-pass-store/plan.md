Reconstructed by project:introspec on 2026-10-18 from 384c771

# Implementation Plan: Secrets through pass (passstore)

**Branch**: `docs/introspec-site-runners` | **Date**: 2026-10-18 | **Spec**: `spec.md`

## Summary

A stdlib-only Python module with five functions (`prefix`, `store`, `check`, `has`, `read`, `write`) and one exception, shared by relative symlinks into the two runners' `scripts/` directories [OBSERVED: `ls -l plugins/*/scripts/passstore.py`].

## Technical Context

**Language/Version**: Python 3.9+ stdlib. **Dependencies**: `pass` (and gpg) on the host. **Storage**: the user's password store. **Testing**: indirect, through `plugins/db/scripts/test_dbrun.py` and `plugins/repo/scripts/test_reporun.py` with a fake `pass` (51 + 35 tests passed locally, see `docs/product/introspec.md` § Runs). **Project Type**: library module.

## Constitution Check

N/A — no constitution. Checked against `shared/secrets.md` ("only a runner ever reads a value") ✓.
