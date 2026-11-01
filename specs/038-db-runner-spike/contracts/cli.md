# CLI contract that must not regress (dbrun today)

Subcommands (from `build_parser`, lines 1748-1827):

- `profiles [--json] [--export-env --out FILE]`
- `classify PROFILE [--json]`
- `test PROFILE [--reason] [--timeout 30]`
- `query PROFILE --sql FILE|- --reason R [--max-rows 100] [--timeout 30] [--reveal] [--format table|csv|json] [--explain] [--allow-full-count]`
- `plan PROFILE --sql F --reason R [--expect N,N]`
- `apply PROFILE PLAN_ID [--confirmed] [--timeout 120]`
- `restore PROFILE PLAN_ID [--confirmed]`
- `script PROFILE --sql F --reason R --out FILE`
- `session start|stop|status PROFILE [--reason]`
- `secrets migrate [PROFILE] [--all]`
- `log [--tail 5]`
- the hidden `_exec`

Exit codes: 0 ok, 1 error, 2 usage, 3 refused (policy), 4 connect (including "neither uv nor a container engine available").

If the verdict is "move", this table is the parity baseline and `ai-gent db` mirrors it; if "stay", it is unchanged. The `_exec` protocol (a JSON request on stdin, one JSON result line on stdout) is internal and may be replaced by native calls.
