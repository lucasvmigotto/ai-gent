#!/usr/bin/env bash
# Tests plugins/db/hooks/guard.sh against the shared fixtures
# (shared/guard-fixtures/db.json, spec 033) through scripts/guard-fixtures.py, which
# prints one `ok`/`FAIL` line per case. Called by scripts/check.sh.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 "$REPO_DIR/scripts/guard-fixtures.py" run --guard db --impl sh
