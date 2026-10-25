#!/usr/bin/env bash
# A tracked feature's specs (one story, some tasks already done) and a
# GitHub remote, so `reporun detect` finds a platform from the remote alone
# (no network call). No token anywhere, in pass or the environment, and no
# network reaches the sandbox: a real create on GitHub can't happen here,
# so this checks the shape of the attempt (one batch, not one call per
# item) and that nothing is claimed to have synced that didn't.
set -euo pipefail
git init -q -b dev
git config user.name eval
git config user.email eval@example.invalid
git remote add origin git@github.com:acme/toolshare.git
mkdir -p specs/001-share-a-tool
cat >specs/001-share-a-tool/spec.md <<'EOF'
# 001 - Share a tool

### User Story 1 - Lend a tool (Priority: P1)

As a neighbor, I want to list a tool I own so others can borrow it.

#### Acceptance Scenarios

1. Given a signed-in neighbor, when they list a tool, then it appears in the catalog.
EOF
cat >specs/001-share-a-tool/tasks.md <<'EOF'
# Tasks: Share a tool

## Phase 1: User Story 1

- [x] T001 [US1] Add the Tool model and migration
- [x] T002 [US1] Build the "list a tool" form
- [ ] T003 [US1] Add the catalog page
EOF
cat >specs/README.md <<'EOF'
# Specs

| # | Feature | Frontend | Backend | QA |
|---|---|---|---|---|
| 001 | Share a tool | Implemented | Implemented | Implemented |
EOF
git add -A
git commit -q -m "docs(specs): 001 share a tool"
