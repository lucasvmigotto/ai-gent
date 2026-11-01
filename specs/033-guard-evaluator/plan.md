# Implementation Plan: Guard evaluator

**Branch**: `docs/specs-rust-core` (spec) | **Date**: 2026-11-01 | **Spec**: `spec.md`

## Summary

A faithful port of the five shell guards (and the opencode rules) into `ai-gent-core::guard`, driven by shared JSON fixtures, with per-shell segmentation for sh, PowerShell and cmd. It runs first in shadow mode beside the shell guards (2.27), then decides (2.28) behind a thin launcher; opencode calls the binary. Parity is proven by one fixture set that every implementation must pass.

## Technical context

**Language**: Rust (workspace of 032). **Dependencies**: `regex` (linear time, no backtracking [KNOWN]), `serde`, `serde_json`, `sha2` (shadow hash), nothing with a network. **Testing**: `cargo test` data-driven from `shared/guard-fixtures/`; `scripts/test-*guard.sh` and `opencode/guard/test` read the same files; `scripts/bench-guard.py`. **Targets**: the six of 032; the fixture suite runs on Linux, macOS and Windows legs in CI. **Constraints**: reasons byte-identical (guard-ux), no secret in any output, `AGENTS.md` rules, D7 (no dual paths after the cutover), D12, D13.

## Constitution and constraint check

| Constraint | Check | How |
|---|---|---|
| D10 parity gate: existing tests become shared fixtures run against both | pass | FR-005, FR-006, Phase 1 |
| D11 sh, PowerShell and cmd with fixtures for all three | pass | FR-003, FR-004, `variants` |
| D12 shadow first, then decides | pass | FR-009, FR-010, phases 3-4 |
| D13 opencode calls the binary, `rules.mjs` dropped | pass | US4, Phase 5 |
| CLAUDE.md: `guard.sh` and `rules.mjs` must decide the same until replaced | pass | the oracle suites stay green through Phase 3; the parity test is the fixtures |
| CLAUDE.md: hook scripts are POSIX sh, tested by the five `test-*guard.sh` | pass | scripts keep their names; the launcher is POSIX sh and tested by them (`--impl sh` runs the launcher after the cutover) |
| `shared/guard-ux.ts` and `consistency.py` rely on reasons | pass | reasons.json and `list-reasons` (FR-013) |
| `shared/secrets.md`: only a runner reads a value; the binary never prints one | pass | FR-015; redaction of the shadow capture |
| Package allowlist | pass | `bin/ai-gent` ships (032); `shared/guard-fixtures/` is not reached by any plugin symlink and is not packaged; `package.py --check` asserts it |
| Naming rule (3.0.0 / 3.x) | pass | not used |

## Architecture

```
rust/crates/ai-gent-core/src/guard/
  mod.rs          evaluate, evaluate_all, Ctx { branch_of: Box<dyn Fn(&Path)->Option<String>> }
  event.rs        Event (serde), Decision, GuardId
  reasons.json    id, guard, marker, text, decision      (include_str!; same crate dir)
  reasons.rs      lookups, {branch}/{tool} placeholders
  segment/
    mod.rs        Shell enum, trait Segmenter { fn blank(&str)->String; fn split(&str)->Vec<String>; }
    sh.rs         port of: gsub quoted strings, \n, [;&|()]  (+ backtick for containers)
    powershell.rs '' and "" and backtick rules; ; & | ( ) newline; trailing backtick
    cmd.rs        "..." only; ^ escapes; & && || | newline
  rules/
    git.rs        port of plugins/git/hooks/guard.sh   (regex ids git.nohook, git.attrib, git.force, git.push, ...)
    db.rs         port of plugins/db/hooks/guard.sh + credentials path check for non-Bash tools
    secrets.rs    port of plugins/secrets/hooks/guard.sh
    repo.rs       port of plugins/repo/hooks/guard.sh
    containers.rs port of plugins/devcontainer/hooks/guard.sh
  branch.rs       symbolic-ref via `git -C`, 1 s timeout
  shadow.rs       record, redact, append, rotate
  launcher.md     (doc only) the sh launcher contract
rust/crates/ai-gent/src/cmd/guard.rs   CLI: evaluate, shadow-report, replay, list-reasons
rust/crates/ai-gent-core/tests/guard_parity.rs
```

### Design decisions

1. **Faithful port first, tokenizer later.** The shell guards are regex matches over blanked text. A real tokenizer would be "better" and would diverge on some of the 192 vectors. So `Sh` reproduces the pipeline exactly (same regexes translated from ERE to the `regex` crate; POSIX classes `[[:space:]]` map to `\s` or the bracket form; every translation is covered by a fixture). `PowerShell` and `Cmd` reuse the same rule code through their own `Segmenter`: only quote blanking and separator splitting differ, because the rules look at command words and flags, which are the same across shells for `git`, `psql`, `pass`, `podman`. Commands that exist only on one shell (`Get-Content`, `type`) matter for the db credentials path check and get their own fixtures.
2. **Decoded string, not JSON-escaped.** The shell guards operate on the JSON-escaped text (a `\n` is two characters). The Rust code operates on the decoded string like `rules.mjs`. Extraction (T010) runs every vector through both and records disagreements between the two views; the expectation is none, and any is a documented `skip_in`.
3. **Reasons in data.** `reasons.json` holds each reason once; rules refer to ids. `scripts/consistency.py` reads `ai-gent guard list-reasons --json` (or the JSON file directly when no binary is built) instead of scraping `guard.sh`.
4. **Branch lookup shells out to `git`.** Exactly as the shell guard (`git -C dir symbolic-ref --quiet --short HEAD`), lazily, with a timeout. Reading `.git/HEAD` directly is faster but must understand worktree `.git` files and `gitdir:` redirections; deferred as a latency optimization only if the budget needs it.
5. **No unwrapping in parity.** `bash -c "git push"` is invisible to the shell guards because the quoted string is blanked. Keeping that behavior is what "parity" means; unwrapping is FR-016.

### Shell quoting facts used for the Windows segmenters

PowerShell escapes with the backtick, only inside double-quoted strings, and treats `^` as an ordinary character; cmd.exe escapes with the caret, does not treat the backtick specially, and has only double quotes [RESEARCHED: S5]. PowerShell single-quoted strings have no escapes except a doubled `'` [KNOWN]; cmd's `%VAR%` expansion happens before parsing [KNOWN] and is ignored by the guards (the text is matched as written). Calling a native executable from PowerShell passes through cmd-style argument quoting [RESEARCHED: S5], which is why the guards inspect the command line text, not argv.

### Hook wiring by phase

| Phase | `hooks.json` command | Decider |
|---|---|---|
| now | `${CLAUDE_PLUGIN_ROOT}/hooks/guard.sh` | shell |
| 2.27 shadow | same; `guard.sh` gains, after its decision is printed, `[ -x "$bin" ] && ( printf '%s' "$input" \| "$bin" guard --guard git --shadow --shell-decision "$d" >/dev/null 2>&1 & )` (3 lines, five scripts) | shell |
| 2.28 | same path; `guard.sh` is the launcher | binary |

The launcher (POSIX sh, about 10 lines): resolve `bin` from `${CLAUDE_PLUGIN_ROOT}/../../bin/ai-gent`, then `command -v ai-gent`; if none, print the `ask` line with the install command; else `out=$("$bin" guard --guard <id> --format claude 2>/dev/null)`; if the exit code is non-zero or the process was killed, print the `ask` line "guard failed"; else print `$out`. A timeout of 2 s uses `timeout` when present (GNU, absent on stock macOS), otherwise the hook's own `timeout` in `hooks.json` (10 s) is the backstop and the binary enforces its 2 s internally. Claude Code treats a non-blocking hook error as not blocking the tool [KNOWN, verify in T051], which is why the launcher must convert failures into `ask` itself instead of relying on exit codes.

### Failure policy reasoning (FR-014)

| Failure | Options | Choice | Why |
|---|---|---|---|
| Binary missing (user upgraded content but not binary, a clone checkout) | allow, ask, deny | ask | allow removes protection silently; deny bricks Bash entirely and the user cannot proceed even when the command is harmless; ask keeps the user in control and prints the fix |
| Crash, panic, timeout | same | ask | a crash is attacker-reachable by a crafted command; failing open is the dangerous default |
| Per guard differences | secrets/db credentials could argue for deny | still ask | the ask message tells the user exactly why; one rule is testable and explainable. Owner may override (spec open question 1) |
| opencode plugin | `event.effect = ask` | same | symmetrical |

### Latency

Baseline (this host, Linux, 100 runs, `git status --short`): git 3.3 ms, db 1.1 ms, secrets 0.8 ms, repo 0.6 ms, devcontainer 1.6 ms. A static Rust binary starting on Linux is expected in the 1-3 ms range [ASSUMPTION; T003 measures]. `scripts/bench-guard.py` spawns each implementation N=100 times per case with the same stdin, with `time.perf_counter_ns`, discards the first 5 runs, reports p50/p95/p99, and compares p95 to FR-012. Cases: a clean `ls`, a `git push` (ask), a commit needing the branch lookup, a long heredoc, a db `Read` path. On Windows the baseline is the Git Bash `guard.sh` if present, else absolute budget only.

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| ERE to Rust regex translation changes semantics (leftmost-longest vs leftmost-first alternation) | silent divergence | grep -E (POSIX) picks the longest match, `regex` the first alternative; the guards only test for match existence per segment, where it does not matter, except in `sed`/`awk` captures (db `$tool`, git `-C` dir); those are fixture-covered and use explicit ordering |
| awk `gsub` quote blanking is order-dependent (double quotes first, then single) | divergence | port the exact order; fixtures include nested quotes |
| Shadow process doubles hook cost | users feel latency | detached, measured; off switch `AI_GENT_NO_SHADOW=1` |
| Windows hook environment: no `sh` | the launcher cannot run | Claude Code on Windows needs Git Bash for the Bash tool [ASSUMPTION, verify T040]; otherwise a `.cmd`/`.ps1` launcher variant is a follow-up spec |
| Fixture extraction drops a vector | parity illusion | counts in `manifest.json`; T010 asserts 192 + the opencode-only vectors; `scripts/check.sh` prints the case count (as it prints guard cases today) |
| Single-user evidence | weak promotion proof | corpus replay of transcripts; larger synthetic corpus from the fixtures with mutations (T032) |
| `shared/guard-fixtures` not reachable from a published crate | crate tests fail | test skips when absent (032 plan) |

## Phased approach

- **Phase 0 - baseline and extraction**: measure the shell baseline; extract fixtures; thin the shell and mjs harnesses onto them (no Rust yet). Green check.sh proves the extraction lost nothing.
- **Phase 1 - evaluator, sh**: segmenter, five rule modules, reasons, CLI, parity tests; all fixtures pass.
- **Phase 2 - PowerShell and cmd**: segmenters, variants, Windows-only cases, Windows CI legs.
- **Phase 3 - shadow (2.27)**: shadow record, `shadow-report`, `replay`, hook change, docs; release 2.27 with shadow on (Linux first, per the release plan; macOS and Windows follow when D14 allows).
- **Phase 4 - cutover (2.28)**: promotion check, launcher, delete shell logic, consistency.py via reasons.json, failure tests, bench gate.
- **Phase 5 - opencode**: plugin calls the binary, delete `rules.mjs` and the parity test, update `opencode/guard/index.ts`, CLAUDE.md rule text.
