# Differences between `dbrun.py` and the Rust runner

Every behavior of `plugins/db/scripts/dbrun.py` that the port changes, or may change, is a row here. A change nobody listed is a bug. Two kinds of row: **Intended** (decided by the owner or by the spec, pinned by a test) and **Question** (found while porting, kept as the Python behavior until the owner answers). Fixtures name their row in a `difference` field and `rust/crates/ai-gent-dbrun/tests/contract.rs` fails if a fixture names a row that is not here. Task owners append rows; the SC-002 mapping of the 51 Python test runs to Rust tests is added to this file as groups land.

## Intended (3.x release that ships the port)

| ID | Item | Python | Rust | Pinned by |
|---|---|---|---|---|
| D-001 | DDL row count (FR-022, plan decision 9) | `cursor.rowcount`, `-1` for DDL and PRAGMA (sqlite3, psycopg, pymssql) | `0` | fixture `13-ddl-after-dml-rowcount` (`expected_port`), `request::rowcount` tests, `SqliteConnector` |
| D-002 | Oracle `NUMBER` (FR-022, plan decision 9) | python-oracledb type (an integer-valued `NUMBER(10,2)` probably prints `5.0`) | typed from its text (`5`; `1E-130` stays a string) | Oracle fixtures of T061 (no fixture yet: SQLite has no `NUMBER`) |
| D-003 | Redaction (FR-008) | host, user, password as whole words, values under 2 characters skipped | password everywhere in the text and in its quoted, escaped, doubled-quote and percent-encoded forms; host and user as whole words, any length, also percent-encoded; one pass, overlapping hits merge (FR-008 requires the redaction of host, user and password; the extra forms are the builder's, see the proposals table) | `core::redact` tests |
| D-004 | Timeout and size limits (FR-011) | none: a timeout of `0` cancels at once on SQLite and means "no limit" in PostgreSQL's `statement_timeout`; a string timeout raises in the middle of the connect; no cap on `max_rows`; no cap on the request | a `connect`-stage error `invalid timeout` unless the timeout is a number from 0.001 to 3600 s (a missing one is 30 s); `invalid max_rows` above 1,000,000; a request line above 16 MiB is a `request`-stage error | fixture `18-bad-timeouts` (port only), `connector::request` tests |
| D-005 | Connection fields of an engine that builds a connect string (FR-009) | `oracle_dsn()` interpolates host, port, service, sid unchecked | an allowlist per field (`core::fields`), refusal names the field, never the value; applied to Oracle (`request::STRING_BUILDING_ENGINES`), other engines pass fields as driver arguments | `core::fields` tests, `request` tests |
| D-007 | Row counts come from the connection's change counter (FR-010) | `cursor.rowcount`: `-1` for a `WITH ... DELETE` or any statement the driver does not count, `0` for `UPDATE ... RETURNING` (read before the rows are fetched) | the server's own count: a statement that changed rows reports its changes (3 for the CTE delete, 2 for the `RETURNING` update), one that returns rows and changed nothing reports `-1`, DDL and no-change DML report `0`. A trigger-only change (an `INSTEAD OF` trigger on a view) reads `changes()`, which may be a stale earlier count: open point for T020 | fixtures `21-cte-dml-count-from-the-server`, `14-dml-returning-rowcount` (`expected_port`) |
| D-008 | A plan recorded by the Python runner with `expect -1` for a DDL statement | commits (`-1` equals `-1`) | refused at `verify` with `statement N: expected -1, got 0`, rolled back, nothing created; the user plans again. Refusing is deliberate: accepting `-1` for DDL would also accept `-1` from a statement whose count is unknown | fixture `22-ddl-expect-minus-one` |
| D-009 | A request without `timeout` (FR-011) | `req["timeout"]` raises `KeyError` (a traceback); the CLI always sends one (default 30) | 30 s | `request` tests |
| D-010 | A request without `max_rows` | `req["max_rows"]` raises `KeyError`; the CLI sends 100 (`query`, default `--max-rows 100`), 1000 (`apply`), 1 (identity probe) | 200 (the crate's existing default; Q-008 asks whether it should be the CLI's 100) | `Request::new` |

## Questions for the owner (the Python behavior is what runs today)

| ID | Item | Python | Rust today | Why it may matter |
|---|---|---|---|---|
| Q-001 | `expect` longer than `statements` | the extra entries are ignored and the write commits | same | a plan whose expected counts outnumber its statements passes verification silently; failing closed would be a `verify` error. Pinned as Python by fixture `19-verify-edges` and `request::verify_counts` |
| Q-002 | An empty connection field | `c.get("service") or c.get("database")` treats `""` as unset | `core::fields` refuses an empty value; the profile loaders (T010) should turn a blank field into "absent" | a profile with `SERVICE=` would otherwise be refused at run time |
| Q-003 | Limits chosen here (a builder proposal, see below) | n/a | timeout 0.001 to 3600 s, `max_rows` at most 1,000,000, request line at most 16 MiB (FR-011 says "capped" and gives no number) | the numbers are mine; the owner may want others |
| Q-004 | A timeout that is out of the range of a JSON number (`1e999`) | Python reads it as `inf` and fails in the connect | the JSON parser rejects the line, so it is a `request`-stage error, not `connect` (FR-011 says `connect`) | same outcome (a refusal with no secret), different stage name; fixing it needs a hand-written number scanner |
| Q-005 | Redaction strictness | see D-003 | a one-character password or user is redacted wherever it occurs, which can make an error unreadable | safe over readable on a credential path; say if a minimum length is wanted |
| Q-006 | Host names in the field allowlist | any text | ASCII only (an internationalized name is given as punycode); IPv6 without brackets | a profile with a Unicode host is refused, with the field named |

## Original behavior pinned by the contract (T002, T005)

Each line is checked against the real `dbrun.py _exec` (process connector, profile `Original`, SQLite) and against the Rust connector (profile `Port`), by the fixture named.

| Behavior of `dbrun.py` | Fixture or test |
|---|---|
| Missing timeout means 30 s (`base_request` always sends one; the Rust request defaults it) | `request` tests |
| Read mode: `fetchmany(max_rows + 1)`, `truncated` is whether one more existed | `02-row-cap`, `16-max-rows-edges`, `request::cap_rows` |
| Write mode: `fetchall()` sliced to `max_rows`, `truncated` present and `false` | `15-write-mode-max-rows`, `16-max-rows-edges` |
| `max_rows` 0: rows `[]`, truncated only when the result is not empty | `16-max-rows-edges` |
| Row count: the driver's `rowcount` (rows returned `-1`, DML its changes, DML with `RETURNING` 0 on SQLite, DDL see D-001) | `13`, `14`, `request::rowcount` |
| `connect` failure: no `results`, `seconds`, `committed`, `server`, `links` | `17-connect-failure-shape` |
| `execute` failure: rolled back, earlier results kept, `seconds` present, no `committed` | `10`, `20-read-failure-keeps-results` |
| `verify` failure: rolled back, every mismatch listed, numbered from 1, `null` skips, no `committed` | `06`, `19-verify-edges`, `request::verify_counts` |
| `test` mode identifies the server and runs no statement | `09` |
| `EXPLAIN` form per engine | `12` |
| Error text is redacted (password, host, user) | `redaction.json`, `core::redact` tests |

## Builder proposals needing owner review (not mandated by the spec)

These are choices the worker made while writing the code. The spec says what to refuse (FR-009, FR-011) but not these specifics; each is open until the owner confirms or changes it.

| Proposal | Where |
|---|---|
| The exact limits: timeout 0.001 to 3600 s, `max_rows` at most 1,000,000, request line at most 16 MiB (Q-003) | `connector::request` |
| The request-error shape for a malformed line, `stage: "request"` (D-006: Python has none, the spec names no stage for it) | `connector::parse_request` |
| `::1` among the loopback names left unredacted | `core::redact` |
| The allowlist of connection fields (ASCII hosts, IPv6 without brackets, `_-.$#` in names) and refusing an empty field (D-005, Q-002, Q-006) | `core::fields` |
| The wider redaction forms of D-003 (dialects of percent-encoding, ASCII case-insensitive host and user) | `core::redact` |
| Oracle as the only string-building engine in `STRING_BUILDING_ENGINES` | `connector::request` |

A future `_exec` reader (the process-side entry point that reads the request from stdin) must bound its own read: the 16 MiB check in `Connector::run_line` runs only after the whole line is in memory, so the reader must stop at the limit while reading.

## Added questions

| ID | Item | Why it matters |
|---|---|---|
| Q-007 | `CountKind::Rows` is `-1` for every engine | that matches `sqlite3` and pymssql only. psycopg3 and PyMySQL report the number of rows for a SELECT and the affected count for `UPDATE ... RETURNING`. The engine groups must decide per engine (their fixtures pin it) and record the choice here; the shared helper only fixes `-1` for the SQLite and SQL Server style |
| Q-008 | Default `max_rows` 200 | the CLI default is 100 |
