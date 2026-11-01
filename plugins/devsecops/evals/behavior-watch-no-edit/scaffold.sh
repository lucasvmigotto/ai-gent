#!/usr/bin/env bash
# A tiny Python project whose CI runs pytest, with a dependency used by the
# code but missing from requirements.txt — the describable failure the
# prompt's log line names. No real GitHub Actions run exists for this repo
# (the sandbox has no network), so the fix comes from the given log plus
# reading the repository, not from `gh`.
set -euo pipefail
git init -q -b main
git config user.name eval
git config user.email eval@example.invalid
mkdir -p .github/workflows
cat >.github/workflows/ci.yml <<'EOF'
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: "3.12"
      - run: pip install -r requirements.txt
      - run: pytest -q
EOF
cat >requirements.txt <<'EOF'
flask==3.0.3
EOF
mkdir -p app tests
cat >app/__init__.py <<'EOF'
EOF
cat >app/client.py <<'EOF'
import requests


def ping(url: str) -> int:
    return requests.get(url, timeout=5).status_code
EOF
cat >tests/test_client.py <<'EOF'
from app.client import ping


def test_ping(monkeypatch):
    class FakeResp:
        status_code = 200

    monkeypatch.setattr("app.client.requests.get", lambda *a, **k: FakeResp())
    assert ping("https://example.invalid") == 200
EOF
git add -A
git commit -q -m "chore: initial commit"
git commit -q --allow-empty -m "feat(client): add a ping helper"
