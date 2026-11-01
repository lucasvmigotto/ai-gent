#!/usr/bin/env bash
# A brand-new repository: just the single-line README git:workflow starts one
# with, on dev — the point project:init runs at. No docs/product/ yet.
set -euo pipefail
git init -q -b main
git config user.name eval
git config user.email eval@example.invalid
printf '# Carebook\n' >README.md
git add README.md
git commit -q -m "chore: initial commit"
git switch -q -c dev
