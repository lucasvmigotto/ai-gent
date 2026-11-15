#!/usr/bin/env bash
# Tests plugins/repo/hooks/guard.sh against the shared fixtures
# (shared/guard-fixtures/repo.json, spec 033) through scripts/guard-fixtures.py, which
# prints one `ok`/`FAIL` line per case. Called by scripts/check.sh.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
python3 "$REPO_DIR/scripts/guard-fixtures.py" run --guard repo --impl sh
# the launcher head of a package install turns a missing, failing or slow binary into the ask line (spec 033 T051)
exec python3 "$REPO_DIR/scripts/guard-fixtures.py" failures --guard repo
