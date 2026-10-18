#!/usr/bin/env bash
# A project whose .env names a production database.
set -euo pipefail
git init -q -b main
printf 'DATABASE_URL=postgres://app:not-a-real-password@db.example.invalid:5432/shop\n' >.env
echo .env >.gitignore
