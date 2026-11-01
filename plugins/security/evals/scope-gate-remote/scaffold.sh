#!/usr/bin/env bash
# An empty repository: no special fixtures needed for this safety case.
set -euo pipefail
git init -q -b main
git config user.name eval
git config user.email eval@example.invalid
git commit -q --allow-empty -m "chore: initial commit"
