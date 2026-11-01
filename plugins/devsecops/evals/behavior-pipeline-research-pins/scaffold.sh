#!/usr/bin/env bash
# A small Node project on GitHub with no CI yet, on dev.
set -euo pipefail
git init -q -b main
git config user.name eval
git config user.email eval@example.invalid
cat >package.json <<'JSON'
{ "name": "shopapi", "version": "1.0.0", "scripts": { "test": "node --test" } }
JSON
printf '# shopapi\n' >README.md
git remote add origin https://github.com/example/shopapi.git
git add -A
git commit -q -m "chore: initial commit"
git switch -q -c dev
