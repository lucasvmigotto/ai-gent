#!/usr/bin/env bash
# A repository on dev with a small, consistent spec handed in as input.
set -euo pipefail
git init -q -b main
git config user.name eval
git config user.email eval@example.invalid
printf '# Pantry\n' >README.md
git add README.md
git commit -q -m "chore: initial commit"
git switch -q -c dev
mkdir -p docs/product/references
cat >docs/product/references/spec.md <<'SPEC'
# Pantry spec

Pantry is a mobile web app for households in the UK to track groceries at home and get a
shopping list of what is running low. Primary user: the person who does the household shopping.
Job that must work in v1: add items by scanning a barcode and see what to buy. Out of scope:
online ordering, recipes. About 2,000 households in year one, 5 requests per household per day,
99.5% availability, page loads under 2 seconds on 4G. Barcode lookups use the Open Food Facts
public API. Personal data: email address only, so UK GDPR applies. Team of 2, launch in 4 months,
hosted in the EU. Success: 40% of households still active after 8 weeks.
SPEC
