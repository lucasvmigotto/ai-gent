# Measurements: 037 repo, spec, pass-store and compact tools

Status: Groups A (module), B and C delivered beside the Python tools; D not started. Spike results (T060-T062) and manual gates (T006, T028) are still to record here.

## Binary size (T001-T027 delivery)

Release build, `cargo build --release --locked -p ai-gent`, glibc x86_64 Linux, `opt-level = "z"`, LTO, strip:

| Build | Bytes |
|---|---|
| `dev` at 13bc8d0 (version, doctor, update, guard, tokens) | 1,870,504 |
| this branch (adds spec, repo, pass-store) | 2,162,848 |
| delta | +292,344 (+15.6%) |

No new dependencies: `regex`, `serde_json`, `clap` are unchanged; SHA-1, base64, percent-encoding and the URL pieces are hand-written (tested against vectors); HTTP is the system `curl` (see T023 evidence).

## Parity

| Gate | Fixtures | Result |
|---|---|---|
| specsync (8 tests) + live corpus | 16 | oracle and binary equal, 0 differences |
| reporun (45 tests) | 53 (45 mapped + edge cases + canary) | oracle and binary equal, 0 differences |

Disagreements found while porting: none left. One Python defect noticed (not ported, not fixed): a `batch` CLI step whose word is a whole `{{step.field}}` reference to a number crashes `reporun.py` with `AttributeError` (the fixture uses a string field).

Deliberate differences from `reporun.py` (all stricter, none visible to a fixture): absolute API URLs must match the base URL's scheme as well as its host; `REPORUN_TEST_BASE` is honoured only for loopback hosts; the HTTP client does not follow redirects; `Last sync` in `tracker.md` is the UTC date (Python used the local date).

## Not measured yet

- T006 gopass on Windows.
- T028 `scripts/test-reporun-github.py` against a real repository (owner).
- Group D (grammar table, spike verdict).
