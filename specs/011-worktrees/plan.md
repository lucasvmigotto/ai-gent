Reconstructed by project:introspec on 2026-10-18 from 384c771

# Implementation Plan: Worktrees for parallel tracks (wt.py)

**Branch**: `docs/introspec-site-runners` | **Date**: 2026-10-18 | **Spec**: `spec.md`

## Summary

A stdlib-only Python CLI over `git worktree`, with `plan`, `add`, `list`, `remove` [OBSERVED: plugins/agents/scripts/wt.py:1-15,216-241].

## Technical Context

**Language/Version**: Python 3.9+ stdlib. **Dependencies**: git ≥ 2.31 [INFERRED: `rev-parse --path-format=absolute`, wt.py:44 → git 2.31+; confirm against the oldest supported git]. **Storage**: `.worktrees/` at the main checkout's root, `.git/info/exclude`. **Testing**: `test_wt.py` (4 test methods) in temporary repositories; passed locally. **Project Type**: CLI.

## Data model

Worktree: branch, path (under `.worktrees/`), dirty, ahead, behind against dev [OBSERVED: wt.py:170-195]. README row: number, name, depends[], sides{} [OBSERVED: wt.py:87-107]. No other state.

## Constitution Check

N/A — no constitution.
