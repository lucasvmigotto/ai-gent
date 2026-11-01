#!/usr/bin/env bash
# A small Node project on an end-of-life runtime and an old library, on dev.
set -euo pipefail
git init -q -b main
git config user.name eval
git config user.email eval@example.invalid
cat >package.json <<'JSON'
{ "name": "shopapi", "version": "1.0.0", "engines": { "node": "14" },
  "dependencies": { "express": "4.16.0", "lodash": "4.17.15", "request": "2.88.2" } }
JSON
printf '# shopapi\n' >README.md
git add -A
git commit -q -m "chore: initial commit"
git switch -q -c dev
