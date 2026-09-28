#!/usr/bin/env bash
# A repository at the point project:spec starts: git, a brief and an architecture.
set -euo pipefail
git init -q -b dev
mkdir -p docs/product
printf '# Toolshare — product brief\n\nStatus: Draft\n\n## Glossary\n\n- Tool — an item a resident lends. Not: item, asset.\n' >docs/product/brief.md
printf '# Toolshare — architecture\n\nStatus: Draft\n\nModular monolith, TypeScript (Fastify) API and React client, PostgreSQL.\n' >docs/product/architecture.md

# The Spec Kit CLI, inside the workspace: the eval sandbox hides ~/.local/share,
# where uv keeps both its tools and its own Pythons, so a host install can't run
# there. Built on the system Python, copied (not linked) from uv's cache, pinned.
UV_TOOL_DIR="$PWD/.tools/uv" UV_TOOL_BIN_DIR="$PWD/.tools/bin" UV_LINK_MODE=copy UV_PYTHON_DOWNLOADS=never \
  uv tool install --quiet --python "$(command -v python3)" \
  "specify-cli @ git+https://github.com/github/spec-kit.git@v1.0.3"
echo .tools/ >>.git/info/exclude
