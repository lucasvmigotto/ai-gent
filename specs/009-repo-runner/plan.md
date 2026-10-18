Reconstructed by project:introspec on 2026-10-18 from 384c771

# Implementation Plan: Platform runner (reporun)

**Branch**: `docs/introspec-site-runners` | **Date**: 2026-10-18 | **Spec**: `spec.md`

## Summary

A stdlib-only Python CLI: `detect`, `cli`, `api`, `batch`, `chain`. REST calls use `urllib.request`; CLI calls run the platform's own binary with the token in the child environment [OBSERVED: plugins/repo/scripts/reporun.py:1-34].

## Technical Context

**Language/Version**: Python 3.9+ stdlib.

**Primary Dependencies**: optional `gh`, `glab`, `az` (+ azure-devops extension), `jira`; `pass`; `git`.

**Integrations** (names only): GitHub REST/GraphQL (`api.github.com` or GHES), GitLab REST/GraphQL, Azure DevOps REST (`dev.azure.com`, WIQL), Bitbucket Cloud REST, Jira REST/Agile — base URL from the remote or the entry's `url:` note / `git config ai-gent.jira.url` [OBSERVED: reporun.py:52-58,218-220,383-410].

**Storage**: none of its own (feature 010 owns `tracker.md`).

**Testing**: `plugins/repo/scripts/test_reporun.py` (27 test methods) with a fake pass and stub HTTP; passed locally [OBSERVED: scripts/check.sh --quick].

**Project Type**: CLI.

## Constitution Check

N/A — no constitution. Checked against `shared/secrets.md` ✓ and `plugins/repo/references/runner.md` (interface, not re-derived here).

## Project Structure

```text
plugins/repo/scripts/
├── reporun.py
├── specsync.py            (feature 010)
├── passstore.py -> ../../../shared/passstore.py
├── test_reporun.py
└── test_specsync.py
```
