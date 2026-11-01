#!/usr/bin/env bash
# A small API that calls a third-party payments vendor, on dev.
set -euo pipefail
git init -q -b main
git config user.name eval
git config user.email eval@example.invalid
mkdir -p docs/product
cat >docs/product/architecture.md <<'MD'
# Architecture
Capacity model: 50 requests/s average, 200 requests/s peak. SLO: p95 under 300 ms.
The checkout endpoint calls the Stripe payments API for every order.
MD
printf '# shopapi\n' >README.md
git add -A
git commit -q -m "chore: initial commit"
git switch -q -c dev
