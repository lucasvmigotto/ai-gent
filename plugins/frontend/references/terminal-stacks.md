# Terminal stacks — choosing and using one

Used by `frontend:tui` (and by `project:architecture` when a terminal
surface was requested). Pick per project and record the choice as an ADR;
check each library's current release and license before committing to it.

## The rule that decides the language

- **The terminal app is a client of the API** (it talks to the backend
  through `contracts/openapi.yaml`): choose the best terminal stack freely —
  a Go or Rust TUI for a Java or .NET backend is normal. Generate its API
  client from the contract.
- **It must embed the project's own code** (a library, a local tool, no
  API in between): stay in that language.

## Defaults

| Situation | Default | Why |
|---|---|---|
| Most business or ops TUIs, free choice | **Go — Bubble Tea** | fastest to build well; largest component set; single static binary; golden-file tests; can be served over SSH |
| Performance-critical, dense or high-frequency data, or a Rust codebase | **Rust — Ratatui** | immediate mode with tight control over every frame; strong types; single binary |
| Python team, data-heavy internal tools | **Python — Textual** (Rich for plain output) | quickest to a polished result; CSS-like styling; built-in async workers and snapshot testing |
| Must embed JVM code | **Java — Lanterna** (picocli for the CLI) | stays in-process; GraalVM native image removes JVM start-up |
| TypeScript/Bun team, sharing code with the web client | **TypeScript — Ink** | React model and skills carry over; `bun build --compile` gives one executable |
| Must embed .NET code | **.NET — Spectre.Console** (rich output, prompts, live views) or **Terminal.Gui** (full windowed TUI) | stays in-process; Native AOT gives one fast executable |
| Only a command-line tool, no interactive screens | the project's language with its standard CLI library (below) | a TUI adds nothing a good CLI doesn't give |

Weigh, per project: the team's language, distribution (single binary vs.
a runtime), start-up time, the component ecosystem, testability, Windows
support, and whether it must run over SSH.

## Per stack

| Stack | TUI model | CLI layer | Tests | Packaging |
|---|---|---|---|---|
| **Go — Bubble Tea** (+ Bubbles components, Lip Gloss styling, Huh forms, Glamour markdown, Wish for SSH) | Elm architecture: `Model`, `Update(msg)`, `View()`; side effects as `tea.Cmd` | Cobra (or Kong); the TUI is one subcommand | `teatest` golden files per screen; table tests on `Update` | goreleaser (archives, Homebrew, deb/rpm, checksums, SBOM) |
| **Rust — Ratatui** (+ crossterm backend) | immediate mode: redraw the whole frame from state each tick; own event loop | clap (derive) | `TestBackend` + `insta` snapshots per screen and size; unit tests on state | cargo-dist; `cargo install` |
| **Python — Textual** (Rich for non-interactive output) | retained widgets, CSS (`.tcss`), reactive attributes, `@work` workers for I/O | Typer (or Click) | `App.run_test()` with the Pilot for keystrokes; `pytest-textual-snapshot` | `uv tool install`, pipx; a single binary only if required (PyInstaller) |
| **Java — Lanterna** | layered: raw `Terminal`, `Screen` buffer, or the `TextGUI` widget toolkit | picocli (native-image friendly) | `DefaultVirtualTerminal` for headless tests of screens and keystrokes | GraalVM native image (picocli's annotation processor generates the config); JReleaser. Lanterna is LGPL-3.0: fine as a library dependency, note it in the license review |
| **TypeScript/Bun — Ink** (+ `@inkjs/ui` components) | React components rendered to the terminal; hooks for input (`useInput`, `useFocus`) | `util.parseArgs` (Bun and Node) or commander | `ink-testing-library` (`lastFrame()` snapshots, `stdin.write` for keys) | `bun build --compile` per target; npm package. Virtualize long lists — Ink re-renders the tree |
| **.NET — Spectre.Console / Terminal.Gui** | Spectre: tables, trees, prompts, `Live`/`Progress` displays over a normal console app; Terminal.Gui: windows, views, layout, mouse | `Spectre.Console.Cli` or System.CommandLine | Spectre.Console.Testing (`TestConsole`); Terminal.Gui's fake driver | `dotnet publish -p:PublishAot=true` (single file); check Terminal.Gui's current major (v1 vs. v2) before starting |

## Terminal lifecycle, whatever the stack

- Enter raw mode and the alternate screen through the library, and
  **always restore** on normal exit, error, panic and signals (Ratatui:
  `ratatui::init()` / `restore()` plus a panic hook; Bubble Tea and Textual
  do it for you — don't bypass them).
- Handle resize (`SIGWINCH` / the library's resize event) by re-laying out,
  never by crashing or clipping silently.
- Keep network and disk work off the render loop: commands, workers,
  tasks or threads that post results back as messages.
- Redraw on change, not in a busy loop; cap animation frame rates.
- Detect capabilities instead of assuming: color depth, Unicode, mouse,
  terminal size — and whether stdout is a terminal at all.
