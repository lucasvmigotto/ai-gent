Reconstructed by project:introspec on 2026-10-18 from 384c771

# Implementation Plan: Specs-to-tracker planner (specsync)

**Branch**: `docs/introspec-site-runners` | **Date**: 2026-10-18 | **Spec**: `spec.md`

## Summary

A stdlib-only Python CLI (`plan`, `record`, `features`) that parses Markdown with regular expressions and writes only `specs/NNN-*/tracker.md`. Item bodies are emitted as Markdown and as HTML (`to_html`, for Azure Boards and Jira) [OBSERVED: plugins/repo/scripts/specsync.py:1-20,230-283].

## Technical Context

**Language/Version**: Python 3.9+ stdlib. **Storage**: `specs/NNN-*/tracker.md`. **Testing**: `test_specsync.py` (8 test methods), passed locally. **Project Type**: CLI.

**Coupling**: its parser fixes the shapes other stages must write — Spec Kit story headings or introspec's `## Stories` list, `T\d+` task lines with `[USn]`, README columns `frontend`/`backend`/`status`/`qa`/`notes` [OBSERVED: specsync.py:36-39,141-160]. `wt.py` (feature 011) parses the same README independently [OBSERVED: plugins/agents/scripts/wt.py:87-107].

## Constitution Check

N/A — no constitution.
