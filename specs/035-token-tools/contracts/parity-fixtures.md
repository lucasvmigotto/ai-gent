# Parity fixture format (proposal; 032 may supersede)

A case is a directory `rust/fixtures/<tool>/<case>/`:

```
case.toml        name, description, argv = ["tokens","budget","--cards","001-core","--json"], cwd = "repo", env = {...}, now = "2026-10-20T12:00:00Z", expect_exit = 0, derived_from = "test_token_report.py::BudgetTest::test_verdicts_proceed_shrink_stop_after_what_the_session_used"
input/           the file tree to materialise: HOME/.claude/projects/<slug>/<sid>.jsonl, repo/specs/..., repo/.ai-gent/...
expect/stdout    exact stdout (or stdout.json compared as parsed JSON when "json = true")
expect/files/    exact contents of every file the command may write; files not listed must be unchanged
```

Rules: every Python test maps to at least one case, `derived_from` names it (a script checks the mapping is total: no Python test unmapped, no case without a source or a `new = true`). Cases marked `os = ["windows"]` run only there. Paths in expectations use `/`. The Python oracle runner materialises `input/`, runs the Python tool with the same argv, and diffs; the Rust runner does the same with the binary. `--bless` regenerates `expect/` from the Python oracle only and refuses to run once the Python file is gone.
