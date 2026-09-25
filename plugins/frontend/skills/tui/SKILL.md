---
name: tui
description: Build a terminal interface the user asked for — a TUI and the CLI it sits on (subcommands, --json, exit codes), or a CLI alone — with Go/Bubble Tea, Rust/Ratatui, Python/Textual, Java/Lanterna, TypeScript/Ink or .NET/Spectre.Console, against the API contract, with snapshot and keystroke tests and packaging. Use for "build a TUI", "terminal dashboard", "design the CLI". Never creates a TUI unasked.
---

# Terminal interfaces — TUI and CLI

## Role

Act as a senior engineer who builds terminal tools people enjoy: fast to
start, obvious to drive from the keyboard, scriptable, and never leaving
the terminal broken. The same product thinking as the web frontend
applies — the vision's principles, the glossary, the contract — drawn in
character cells.

## Only when asked

Build a **TUI only when the user asked for one** in so many words (a
TUI, a terminal UI, a terminal dashboard, an interactive terminal app).
Never add one on your own or propose it as a default. A **CLI** is in
scope when the product is a command-line tool, the user asks for one, or
it's the layer under a requested TUI. If the request is ambiguous ("a
tool to manage orders"), ask which interface they want.

## Before starting

1. Read `../../references/pipeline.md`,
   `../../references/terminal-design.md` and
   `../../references/terminal-stacks.md`.
2. Read what exists: `docs/product/ux-vision.md` (its terminal section),
   each feature's `tui.md` and the `## Frontend` tasks tagged `[TUI]` /
   `[CLI]`, `contracts/openapi.yaml`, the brief's glossary, and
   `docs/product/architecture.md`'s Stack decision. Without a vision or
   specs, work from the user's request and say which inputs were missing.
3. **Stack.** Use the architecture's choice; if it's open, choose with
   `terminal-stacks.md` (client of the API → free choice; embeds the
   project's code → its language) and record an ADR. Default when free:
   Go with Bubble Tea.
4. **Environment.** Build and test in the project's tools container
   (`devcontainer:workflow`); interactive runs need a TTY (`run -it`).

## Build — phase by phase

Branch per phase (`git:workflow`); every phase ends at the checkpoint.

1. **CLI core.** Commands, flags, config precedence, `--json` output
   schemas, exit codes and errors per `terminal-design.md`, backed by an
   API client generated from the contract (or the embedded code). Every
   action a TUI will offer exists here first.
2. **TUI shell** (only for a requested TUI). Terminal lifecycle with
   guaranteed restore, the app loop with I/O off the render path, layout
   regions, themes and color tiers, the global keymap, the help screen,
   the command palette, resize and "too small" handling, and a start-up
   check that falls back to the CLI when stdout isn't a terminal.
3. **Screens, feature by feature** from `tui.md`: views, keymaps, focus
   order and every state, against a Prism mock of the contract so the
   backend never blocks the work; then against the real API.
4. **Packaging.** The stack's release tool (`terminal-stacks.md`),
   version and build info in `--version`, shell completions, a man page or
   `help` command, checksums; built in the tools container, released
   through `devsecops:pipeline`.

## Checkpoint — every phase

1. **Tests:** unit tests on commands and state updates; a snapshot of
   every screen at 80×24 and 120×40; scripted keystroke journeys for the
   phase's flows (the stack's test tool); `--json` output validated
   against its schema; exit codes asserted.
2. **Record it:** a `vhs` tape per main flow, rendered and reviewed like a
   screenshot — layout, focus, feedback, copy.
3. **Terminal hygiene:** `Ctrl-C` in the middle of work and a forced error
   both leave the terminal usable; resize mid-screen; `NO_COLOR=1`,
   monochrome and `--ascii` runs stay legible; piping the output
   (`| cat`) gives plain, non-interactive output.
4. **Copy check:** glossary terms, the vision's voice, errors that say how
   to fix them.

Mark the feature's frontend status `Implemented` in `specs/README.md`
only after the last checkpoint passes.

## Quality floor

- Every TUI action has a CLI equivalent with `--json`.
- The terminal is restored on every exit path.
- No blocking I/O on the render loop; no busy redraws.
- Meaning never by color alone; `NO_COLOR` respected; a high-contrast
  theme exists.
- No secrets as flags; nothing sensitive in logs or `--debug` output.

## Handoff

The commands and screens built, the stack and its ADR, test and tape
results, the release artifacts, and what's open. `qa:e2e` covers
cross-stack terminal journeys; `project:docs` publishes the CLI reference
and the recorded demos. Commits follow `git:workflow`.
