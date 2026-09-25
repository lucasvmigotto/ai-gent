# Terminal interfaces — design and CLI conventions

Used by `frontend:uiux` (the vision's terminal section), `frontend:spec`
(`tui.md`) and `frontend:build`'s terminal sibling `frontend:tui`.

## When there is a terminal interface at all

- **A TUI only when the user asked for one.** Never add it on your own —
  not as a default, not as a bonus, not as a suggestion slipped into a
  plan. A web client is not a reason for a TUI, nor the other way round.
- **A CLI** when the product is a command-line tool, when the user asks
  for one, or as the non-interactive layer every requested TUI sits on.

## CLI conventions

Following clig.dev's guidelines:

- **Commands:** subcommands for distinct actions (`tool order list`,
  `tool order cancel <id>`), named with glossary terms; `--help` on every
  command with a usage line, a description and real examples; `--version`.
- **Flags:** long names always, short ones for the frequent few; `-` means
  stdin/stdout; no positional arguments whose meaning depends on order
  beyond the obvious one.
- **Output:** results to stdout, diagnostics and progress to stderr.
  Human-readable by default; `--json` (stable schema, documented,
  versioned like an API) for scripts and agents; `--quiet` for exit-code
  use.
- **Exit codes:** 0 success, 1 failure, 2 usage error, then documented
  codes for distinct conditions (not found, conflict, refused).
- **Not a terminal → no interaction.** No prompts, spinners or colors when
  stdin/stdout isn't a TTY or `--no-input` is set; confirmations become
  `--yes` / `--force`, and a destructive action on many items asks for
  the count or the name to be typed back.
- **Color:** respect `NO_COLOR`, `--no-color` and `TERM=dumb`.
- **Configuration precedence:** flags > environment variables > project
  config > user config (`$XDG_CONFIG_HOME/<tool>/`). Secrets never as
  flags (they show in the process list): environment, a file, or stdin.
- **Errors** say what happened and how to fix it, in the interface's
  voice, with the failing input named; stack traces only with `--debug`.
- Shell completion generation and a man page or `help <command>`.

## TUI design — the vision's terminal section

- **Layout in character cells.** A minimum size (80×24 unless the content
  demands more) with a clear "terminal too small" message; how every
  screen reflows on resize; regions such as header, body and a status bar
  carrying the key hints for the current context.
- **Keymap**, consistent on every screen: `?` help (lists every key),
  `q` / `Esc` back or quit (confirm only with unsaved work), `Ctrl-C`
  always exits cleanly, arrows and `h j k l`, `Enter` select, `Tab` /
  `Shift-Tab` focus, `/` search or filter, `:` or `Ctrl-P` command
  palette. No hidden modes; the current mode is always visible. Mouse
  support optional, never required.
- **Focus** always visible (reverse video, a highlighted border), with a
  defined focus order per screen.
- **Color** in tiers — truecolor, 256, 16, monochrome — with semantic
  roles (accent, success, warning, danger, muted) mapped per tier; dark
  and light themes; a high-contrast theme. Meaning never by color alone:
  a symbol or word goes with it.
- **Characters:** Unicode box drawing and symbols with an ASCII fallback
  (non-UTF-8 locale, `--ascii`).
- **Feedback:** a spinner for waits of a few seconds, a progress bar with
  counts for longer ones; the interface stays responsive and cancellable
  (`Esc`); errors inline where they happened plus a status-bar summary;
  empty states name the next action.
- **Data:** long lists paged or virtualized with a filter; tables with
  column priorities for narrow terminals; timestamps and numbers in the
  user's locale.
- **Motion:** only to show what changed; animations can be turned off.
- **Copy:** the same glossary terms and voice as the rest of the product.
- **Accessibility:** screen readers handle full-screen TUIs poorly, so the
  CLI (and a `--plain` line-by-line mode where it helps) is the
  accessible path to every action.

## `tui.md` — per feature

Written by `frontend:spec` for each feature the requested terminal
interface covers:

- **Screens** — ASCII mock-ups at 80×24 and 120×40, with what each region
  shows.
- **Keymap** — a table per screen: key, action, when it's available.
- **Focus order** and **states** — loading, empty, error, partial,
  offline, too small.
- **Data** — the contract `operationId` behind each view and action.
- **CLI equivalents** — the command (with `--json`) for every action.
- **Tests** — snapshot sizes, keystroke journeys, the `vhs` tape.
