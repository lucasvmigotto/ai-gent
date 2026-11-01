# Parity fixture format (as built)

A case is one file, `rust/fixtures/tokens/<case>.json`, written by `scripts/test-parity-tokens.py --bless` from the unmodified Python tool and reviewed in the PR:

```
name, derived_from      the case and the Python test(s) it comes from ("Class::test_name")
files                   {path relative to the temp root: text}; roots are home/ (HOME), work/ (cwd), ai-home/ (AI_GENT_HOME), codes/
runs[]                  {argv (after the subcommand), stdin, exit, stdout}, run in order in work/
expect_files            {path: text} of every file a run created or changed (nothing else may change)
```

Placeholders in `files` and `argv`: `<ROOT>` is the temp root; `@NOW-<seconds>@` is the run's clock minus that many seconds as `YYYY-MM-DDTHH:MM:SSZ`. Output is normalised before comparing: the root becomes `<ROOT>` and `Generated YYYY-MM-DD HH:MM` becomes `Generated <TS>`.

Runners: `scripts/test-parity-tokens.py` (oracle `token_report.py`, or `--bin` for the binary) and `cargo test -p ai-gent --test parity_tokens`. `--bless` refuses to run once `token_report.py` is gone. `derived_from` makes the mapping auditable; `fixture-map.md` lists all 68 Python tests. Windows-only cases (`os = ["windows"]`) are not built yet (T050).
