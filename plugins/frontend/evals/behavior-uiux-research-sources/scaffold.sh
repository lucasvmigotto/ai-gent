#!/usr/bin/env bash
# A small product brief for a public booking app sold in the EU and the US.
set -euo pipefail
git init -q -b main
git config user.name eval
git config user.email eval@example.invalid
mkdir -p docs/product
cat >docs/product/brief.md <<'MD'
# Brief: TableNow
Status: Accepted

TableNow lets diners book restaurant tables on the web and phone. Sold in the EU and the
United States. Audiences: diners (all abilities) and restaurant staff.
Goals: book a table in under 60 seconds; usable with a screen reader and keyboard only.
Compliance: must meet the accessibility law that applies in each market.
MD
git add -A
git commit -q -m "chore: initial commit"
git switch -q -c dev
