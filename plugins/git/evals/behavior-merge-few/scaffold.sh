#!/usr/bin/env bash
# A repository with dev and feat/small (2 commits), no remote.
set -euo pipefail
git init -q -b main
git config user.name eval
git config user.email eval@example.invalid
git commit -q --allow-empty -m "chore: initial commit"
git switch -q -c dev
git switch -q -c feat/small
for n in $(seq 1 2); do echo "$n" >"small-$n.txt"; git add .; git commit -q -m "feat(small): part $n"; done
git switch -q dev
