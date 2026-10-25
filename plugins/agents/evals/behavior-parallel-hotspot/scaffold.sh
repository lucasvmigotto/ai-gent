#!/usr/bin/env bash
# Two independent, ready features whose tasks both touch CHANGELOG.md — the
# shared file agents:parallel names as a hot spot that needs one owner.
set -euo pipefail
git init -q -b dev
git config user.name eval
git config user.email eval@example.invalid
printf '# Changelog\n\n## Unreleased\n' >CHANGELOG.md
mkdir -p specs/004-export-timesheet specs/006-tool-ratings
cat >specs/README.md <<'EOF'
# Specs

| # | Feature | Frontend | Backend | QA | Depends on |
|---|---|---|---|---|---|
| 004 | Export timesheet | Planned | Planned | Planned | — |
| 006 | Tool ratings | Planned | Planned | Planned | — |
EOF
cat >specs/004-export-timesheet/tasks.md <<'EOF'
# Tasks: Export timesheet

## Phase 1: User Story 1

- [ ] T001 [US1] Add the CSV export endpoint
- [ ] T002 [US1] Add the export button to the timesheet page
- [ ] T003 [US1] Note the feature in CHANGELOG.md
EOF
cat >specs/006-tool-ratings/tasks.md <<'EOF'
# Tasks: Tool ratings

## Phase 1: User Story 1

- [ ] T001 [US1] Add the Rating model and migration
- [ ] T002 [US1] Add the star-rating widget
- [ ] T003 [US1] Note the feature in CHANGELOG.md
EOF
git add -A
git commit -q -m "docs(specs): 004 export timesheet, 006 tool ratings, both planned"
