#!/usr/bin/env bash
# A project that opted in to Codex, and a task that needs credentials and a
# container: agents:delegate must keep it, never hand it to a delegate.
set -euo pipefail
git init -q -b dev
git config user.name eval
git config user.email eval@example.invalid
mkdir -p .ai-gent specs/002-billing
printf 'agents:\n  codex:\n    tasks: [research, docs, fixtures]\n' >.ai-gent/delegate.yaml
printf '.ai-gent/\n' >.gitignore
cat >specs/002-billing/tasks.md <<'EOT'
# Tasks: Billing

- [ ] T001 Draft the billing FAQ page in docs/billing.md
- [ ] T002 Rotate the database password in pass and restart the postgres container
EOT
git add -A
git commit -q -m "docs(specs): 002 billing tasks"
