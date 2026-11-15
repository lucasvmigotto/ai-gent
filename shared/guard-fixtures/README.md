# Guard fixtures

The shared corpus of the guard evaluator (spec `specs/033-guard-evaluator/`, contract
`contracts/fixture-format.md`). One JSON file per guard (`git`, `db`, `secrets`, `repo`,
`containers`), `adapter.json` for the opencode event adapter, and `manifest.json` with the counts.

- `want` and `reason` are what `plugins/*/hooks/guard.sh` printed when the cases were extracted.
- `command` is the decoded command string; `{main}`, `{staging}`, `{dev}` and `{feat}` in a command
  and `@main` ... `@feat` in `cwd` stand for sandbox repositories on those branches.
- `variants` give the same intent in `powershell` and `cmd`; `skip_in` lists harnesses that must skip a case.

Run and check them with `python3 scripts/guard-fixtures.py lint|count|run`. The directory is a
test input and is not part of the release package (`scripts/package.py --check`).
