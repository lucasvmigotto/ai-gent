#!/usr/bin/env bash
# A repository at the point project:spec starts: git, a brief and an architecture.
set -euo pipefail
git init -q -b dev
mkdir -p docs/product
printf '# Toolshare — product brief\n\nStatus: Draft\n\n## Glossary\n\n- Tool — an item a resident lends. Not: item, asset.\n' >docs/product/brief.md
printf '# Toolshare — architecture\n\nStatus: Draft\n\nModular monolith, TypeScript (Fastify) API and React client, PostgreSQL.\n' >docs/product/architecture.md
