Reconstructed by project:introspec on 2026-10-18 from 384c771

# Feature Specification: Platform runner (reporun)

**Feature Branch**: `docs/introspec-site-runners` (reconstructed; the code already exists on `main`)

**Created**: 2026-10-18

**Status**: Draft

**Input**: Reverse-engineered from `plugins/repo/scripts/reporun.py` (765 lines) and `plugins/repo/scripts/test_reporun.py`. The `repo:*` skills' Markdown is out of scope.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Detect the platform and token state (Priority: P1)

An agent runs `reporun detect` and learns the repository's platform (from the remote URL or `git config ai-gent.platform`), its owner/project, whether the platform CLI is installed and signed in, and whether the token entry exists in pass — without decrypting anything.

**Independent Test**: `test_platforms`, `test_forced_platform`, `test_detect_never_decrypts`.

**Acceptance Scenarios**:

1. **Given** a GitHub, GitLab, Azure DevOps or Bitbucket remote, **When** `detect` runs, **Then** the platform and its coordinates are reported [OBSERVED: plugins/repo/scripts/reporun.py:163-216,238-273].
2. **Given** a Jira URL in `git config ai-gent.jira.url` or a Jira entry, **When** `detect` runs, **Then** Jira is reported too [OBSERVED: reporun.py:243-249].
3. **Given** a missing token, **When** `detect` prints, **Then** it shows the `pass insert -m <entry>` to run in the user's own terminal [OBSERVED: reporun.py:270-271].

---

### User Story 2 - Read through the platform CLI or REST API (Priority: P1)

An agent reads issues, PRs and checks through `reporun cli <platform> -- <args>` (gh, glab, az, jira) or `reporun api <platform> METHOD PATH` with the token from pass, never on a command line and never printed.

**Independent Test**: `test_cli`, `test_api`, `test_github_bearer_and_placeholders`, `test_azure_basic_and_api_version`, `test_jira_basic_with_email`, `test_gitlab_project_path_encoded`, `test_bitbucket_pagination`, `test_token_in_env_only_and_redacted`, `test_errors_redacted`.

**Acceptance Scenarios**:

1. **Given** a CLI call, **When** a token is in pass, **Then** it is passed only in the child's environment (`GH_TOKEN`, `GITLAB_TOKEN`, `AZURE_DEVOPS_EXT_PAT`, `JIRA_API_TOKEN`; `GH_ENTERPRISE_TOKEN` for GHES), debug variables are dropped, and stderr names the source [OBSERVED: reporun.py:52-58,330-347].
2. **Given** an API call, **When** it runs, **Then** auth is Bearer, or Basic for Azure DevOps (`:token`) and for Bitbucket/Jira with a user note; `{owner}`-style placeholders are filled and URL-encoded; Azure gets `api-version=7.1` [OBSERVED: reporun.py:412-428,483-498].
3. **Given** an absolute URL on another host, **When** called, **Then** refused: "the token goes nowhere else" [OBSERVED: reporun.py:485-487; test_other_hosts_refused].
4. **Given** `--paginate`, **When** called, **Then** pages follow the platform's next link up to `--max-pages` (10) [OBSERVED: reporun.py:450-457,509-540].
5. **Given** any output or error, **When** printed, **Then** the token and its Basic encoding are replaced by `<token>` [OBSERVED: reporun.py:140-145].
6. **Given** GitHub with no pass entry, **When** an API call runs, **Then** it proceeds unauthenticated ("none (public)") [OBSERVED: reporun.py:477-481,505-506].

---

### User Story 3 - Writes only when asked (Priority: P1)

Any call that changes something on the platform needs `--write`, which the repo guard turns into a question for the user; CLI subcommands and flags that can print a token or change the CLI's login are refused outright.

**Independent Test**: `test_write_needs_flag`, `test_refusals`, `test_write_with_flag`.

**Acceptance Scenarios**:

1. **Given** a non-GET/HEAD API call (except read-only POSTs: GitHub/GitLab GraphQL without `mutation`, Azure WIQL, Jira search), **When** run without `--write`, **Then** refused, exit 3 [OBSERVED: reporun.py:75-81,430-436,473-475].
2. **Given** a CLI call, **When** no read verb (`list`, `view`, `status`, `checks`, `diff`, …) is among its first four positional words, or an option precedes the subcommand, **Then** it is a write [OBSERVED: reporun.py:61-64,278-309].
3. **Given** `gh auth|alias|extension|secret|codespace|config`, `glab auth|alias|config|variable|token`, `az login|logout|account|extension|config` or `jira init`, or `--debug`/`--show-token`, **When** run, **Then** refused [OBSERVED: reporun.py:66-72,321-325].

---

### User Story 4 - Several calls, one confirmation (Priority: P2)

`reporun batch FILE` runs a JSON plan of named steps in order, with `{{step.field}}` references between them, one `--write` for the whole batch, `--dry-run` to list without calling, and stops at the first failure.

**Independent Test**: `test_needs_write_once`, `test_dry_run_calls_nothing`, `test_references_and_records`, `test_stops_at_first_failure`, `test_reads_need_no_write`.

**Acceptance Scenarios**:

1. **Given** a batch with write steps, **When** run without `--write` or `--dry-run`, **Then** refused, naming up to eight write steps [OBSERVED: reporun.py:604-618].
2. **Given** `--dry-run`, **When** run, **Then** each step prints as read/write and nothing is called [OBSERVED: reporun.py:619-623].
3. **Given** a failing step, **When** run, **Then** later steps don't run and the report is `ok: false`, exit 1 [OBSERVED: reporun.py:625-660].

---

### User Story 5 - The branch chain (Priority: P2)

`reporun chain` derives work → integration (`dev`/`develop`) → [staging (`staging`/`homolog`/`hml`)] → production (`main`/`master`, else the remote default), where stories count as integrated and where closing keywords act, and flags ambiguity to ask about.

**Independent Test**: `test_full_chain`, `test_ambiguous_asks`, `test_no_integration_branch`.

**Acceptance Scenarios**:

1. **Given** both `dev` and `develop`, **When** `chain` runs, **Then** integration is unset and an ASK line is printed [OBSERVED: reporun.py:665-701].
2. **Given** no integration branch, **When** `chain` runs, **Then** `integrated_on` is production [OBSERVED: reporun.py:689].

### Edge Cases

- A write CLI call whose positional arguments include a read verb is classified as a read and runs without `--write`: `cli_is_write("gh", ["label", "create", "status"])` and `["issue", "edit", "12", "view"]` return `False` [OBSERVED: run `python3 -I -c 'import reporun; …'` in plugins/repo/scripts on 2026-10-18; cause reporun.py:309]. Risk: a platform change without the user's question.
- GraphQL "read" is decided by the absence of the substring `mutation` in the body [OBSERVED: reporun.py:435] — safe for writes (they need the keyword), but a query mentioning "mutation" in a string needs `--write`.
- Redaction skips secrets shorter than 4 characters [OBSERVED: reporun.py:142].

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Tokens MUST come from pass entries under `<prefix>/<platform>/`; an entry under `db/` is refused [OBSERVED: reporun.py:100-108].
- **FR-002**: A token MUST NOT appear on a command line, in output or in errors.
- **FR-003**: Writes MUST need `--write` per call or per batch; unknown is a write.
- **FR-004**: A token MUST only be sent to the platform's API host.
- **FR-005**: Exit codes: 0 ok, 1 error, 2 usage, 3 refused [OBSERVED: reporun.py:36].

### Key Entities

See `data-model.md`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `test_reporun.py` passes [OBSERVED: `scripts/check.sh --quick` 2026-10-18 → "reporun, specsync (Ran 35 tests)" ok].
- **SC-002**: No platform write without `--write` — *not met* for CLI calls with read-verb arguments (Edge Cases).

## Assumptions

- [ASSUMPTION: the repo guard hook (`plugins/repo/hooks/guard.sh`, not reconstructed) is the only thing that asks the user; it keys on `--write`, so the misclassification above bypasses the question.]
