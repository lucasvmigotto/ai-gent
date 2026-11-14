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
| D-006 | A malformed request line (FR-011) | Python has no request-stage error: a traceback | `{"ok":false,"stage":"request","error":...}`, nothing run | `connector::parse_request` tests |
| D-007 | Row counts come from the connection's change counter (FR-010) | `cursor.rowcount`: `-1` for a `WITH ... DELETE` or any statement the driver does not count, `0` for `UPDATE ... RETURNING` (read before the rows are fetched) | the server's own count: a statement that changed rows reports its changes (3 for the CTE delete, 2 for the `RETURNING` update), a read-only statement that returns rows (a SELECT) reports `-1`, DDL, no-change DML and a `RETURNING` that matched nothing report `0` (as Python). Trigger rows are not counted (`changes()` is the statement's own rows; checked in review: an `INSTEAD OF` trigger statement reports `0`, not a stale count); a `changes()` above the total-changes difference is refused. A `PRAGMA` that returns rows but is not read-only reports `0` where Python reports `-1`: open point for T020 | fixtures `21-cte-dml-count-from-the-server`, `14-dml-returning-rowcount` (`expected_port`) |
| D-008 | A plan recorded by the Python runner with `expect -1` for a DDL statement | commits (`-1` equals `-1`) | refused at `verify` with `statement N: expected -1, got 0`, rolled back, nothing created; the user plans again. Refusing is deliberate: accepting `-1` for DDL would also accept `-1` from a statement whose count is unknown | fixture `22-ddl-expect-minus-one` |
| D-009 | A request without `timeout` (FR-011) | `req["timeout"]` raises `KeyError` (a traceback); the CLI always sends one (default 30) | 30 s | `request` tests |
| D-010 | A request without `max_rows` | `req["max_rows"]` raises `KeyError`; the CLI sends 100 (`query`, default `--max-rows 100`), 1000 (`apply`), 1 (identity probe) | 200 (the crate's existing default; Q-008 asks whether it should be the CLI's 100) | `Request::new` |
| D-011 | PostgreSQL: DDL, `EXPLAIN` and `SHOW` row counts (T031, FR-022) | psycopg `cursor.rowcount`: `-1` for DDL, the rows received for `EXPLAIN` and `SHOW` | `0` for every statement whose `CommandComplete` tag carries no number (the client crate keeps only the number); a `SELECT` and a DML keep the server's number, as psycopg | `connector::postgres` tests `the_count_is_the_command_tag_number_for_every_kind_of_statement`; `real_server_counts_come_from_the_server` (needs a server) |
| D-012 | PostgreSQL TLS (T032, FR-015..FR-017) | libpq `sslmode=prefer` through psycopg: TLS when offered, plain text otherwise, certificate not verified | always TLS, certificate verified (system roots or `tls_ca_file`, name from `tls_server_name` or the host); a server that answers `N` to the SSL request is a `connect` failure, never a plain session; `unverified` only through `TlsSettings::authorize`; the request's host must be the host the policy was checked for | `tests/connector_postgres.rs` (fake TLS server with a throwaway CA) |
| D-013 | PostgreSQL port | a non-numeric `port` raises in the connect; an empty one is 5432 | a port that is not 1-65535 is a `connect` failure `invalid port` (the spike fell back to 5432 on a parse error, which could connect to the wrong service); an empty or missing one is 5432 | `bad_connection_facts_are_refused_without_echoing_them` |
| D-014 | PostgreSQL silent server, and an abandoned request | none: a server that accepts and then never answers hangs the runner (`connect_timeout` covers the TCP connect only) | the connector waits `connect_timeout` (10 s) plus up to 2 s for the connection to be established and fails at stage `connect` (exit 4 class, no `seconds`) when it is not; after that it abandons the request at `grace (5 s) + timeout x (statements + 3)` with a stage `execute` failure. An abandoned request never commits: the worker checks a shared flag before `BEGIN`, each statement and `COMMIT` and rolls back; when `COMMIT` was already sent the message is `commit outcome unknown` (check the data). The abandoned thread may hold its socket until the process ends, and the server's `idle_in_transaction_session_timeout` (60 s) closes an open transaction | `a_server_that_accepts_and_goes_silent_is_a_connect_failure_at_the_connect_deadline`, `an_abandoned_write_never_commits`, `a_commit_that_does_not_answer_in_time_is_reported_as_unknown` |
| D-015 | PostgreSQL session settings (review of T030) | none: the server's `DateStyle` and `bytea_output` apply | the startup options pin `DateStyle=ISO` and `bytea_output=hex`, so the value normalisation of timestamps and `bytea` cannot be defeated by a server or role setting (a statement may still `SET` them for its own session) | `a_verified_connection_with_the_ca_reaches_the_server_and_sends_the_session_options`, `read_mode_begins_read_only_checks_after_every_statement_and_never_commits` |
| D-016 | PostgreSQL read mode after transaction control (review of T030) | the session default alone: `COMMIT` then `BEGIN READ WRITE` or `SET default_transaction_read_only = off` make the next statement writable | after every statement the connector reads `transaction_read_only` and the transaction's start time (`pg_catalog.current_setting` and `pg_catalog.now()`, schema-qualified); the flag must be `on` and the start time unchanged, otherwise rollback and an `execute` failure before the next statement runs (one extra round trip per statement). The statement that flipped it has already run. For `COMMIT` or a `SET` that is harmless; a `DO` block or `CALL` that commits inside itself is refused by the server itself: run against PostgreSQL 17 (2026-11-14) every variant (`DO` with `COMMIT` then `CREATE TABLE`, with `set_config` and `COMMIT`, with `SET TRANSACTION READ WRITE`, and `DO{COMMIT}` then a `CREATE`) failed with SQLSTATE 2D000 `invalid transaction termination` because the connector's transaction is explicit, and no table was created. A `DO` that only sets the GUC cannot change the current transaction. The recheck remains a backstop | `read_mode_stops_when_a_statement_made_the_session_writable`; ignored `real_server_read_mode_refuses_a_write_with_25006_and_a_second_statement` |

Added in the second review round (2026-11-14), part of D-MY-01: the lexer refuses any non-ASCII space or control character outside quoted text and counts only ASCII blanks after `--` (the server and Rust disagree on Unicode spaces); read mode refuses, before connecting, any statement that does not start with SELECT, WITH, SHOW, DESCRIBE, DESC, EXPLAIN, TABLE or VALUES (and `EXPLAIN ANALYZE`); after session setup `@@session.sql_mode` is read back and a session whose mode turns on `ANSI_QUOTES` or `NO_BACKSLASH_ESCAPES`, directly or through ANSI, ORACLE, MSSQL, DB2, POSTGRESQL or MAXDB, is refused. The live test `read_only_is_checked_after_every_statement` now checks the allowlist refusals plus the server's own 1792 for a function that deletes (`SELECT wr()`); the connector's "no longer read only" recheck has no live test, because no allowed statement can switch the session to read write, and stays as a backstop.

## Questions for the owner (the Python behavior is what runs today)

| ID | Item | Python | Rust today | Why it may matter |
|---|---|---|---|---|
| Q-001 | `expect` longer than `statements` | the extra entries are ignored and the write commits | same | a plan whose expected counts outnumber its statements passes verification silently; failing closed would be a `verify` error. Pinned as Python by fixture `19-verify-edges` and `request::verify_counts` |
| Q-002 | An empty connection field | `c.get("service") or c.get("database")` treats `""` as unset | `core::fields` refuses an empty value; the profile loaders (T010) should turn a blank field into "absent" | a profile with `SERVICE=` would otherwise be refused at run time |
| Q-003 | Limits chosen here (a builder proposal, see below) | n/a | timeout 0.001 to 3600 s, `max_rows` at most 1,000,000, request line at most 16 MiB (FR-011 says "capped" and gives no number) | the numbers are mine; the owner may want others |
| Q-004 | A timeout that is out of the range of a JSON number (`1e999`) | Python reads it as `inf` and fails in the connect | the JSON parser rejects the line, so it is a `request`-stage error, not `connect` (FR-011 says `connect`) | same outcome (a refusal with no secret), different stage name; fixing it needs a hand-written number scanner |
| Q-005 | Redaction strictness | see D-003 | a one-character password or user is redacted wherever it occurs, which can make an error unreadable; host and user also match in any ASCII case, so a user `a`, `user`, `select` or `sa`, or a host `db`, turns "A table named a is a problem" into "<user> table named <user> is <user> problem" | safe over readable on a credential path; say if a minimum length is wanted |
| Q-006 | Host names in the field allowlist | any text | ASCII only (an internationalized name is given as punycode); IPv6 without brackets | a profile with a Unicode host is refused, with the field named |
| Q-PG-01 | PostgreSQL: a `SELECT`'s `rowcount` | psycopg: the number of rows the server sent (`SELECT 3` gives `3`) | the same (the server's number); the contract's `-1` for a statement that returns rows comes from SQLite and is not applied, because `CommandComplete` cannot tell `SELECT 3` from `UPDATE ... RETURNING` with 3 rows without reading the statement text | a fixture for PostgreSQL would need to say which of the two the contract wants |
| Q-PG-02 | PostgreSQL: a host that is empty or a unix socket | no host means libpq's default socket; a path starting with `/` is a socket and uses no TLS | an empty host is a `connect` failure `missing conn.host`; a socket path (Unix only) uses no TLS, since it never leaves the machine. Caveat: the socket is not authenticated, so a process that can create a socket in a world-writable directory (such as `/tmp`) can impersonate the server and receive the password if the server asks for a cleartext one; the profile's host is the user's choice and a socket under a directory only the user can write is safe | say if a socket host should also be refused, or limited to directories not writable by others |
| Q-PG-03 | PostgreSQL without TLS | psycopg connects to a server with TLS off (`prefer`) | unreachable, so a plain local `postgres` container fails to connect with `the server must offer TLS`: there is no `disable` mode in the profile model (`verify` or `unverified`), and `unverified` still encrypts. A plain `postgres:17` container has TLS off, so the engines script (T034) must start it with `ssl=on` and a self-signed certificate and set `tls_mode = unverified` on its proven-local profile | DECIDED by the owner (2026-11-14): no `disable` mode; a local container must use TLS (`ssl=on`), and the db docs say how |
| Q-010 | PostgreSQL values: `json` and `jsonb` | psycopg returns a Python object and `to_jsonable` prints `str(dict)` (`{'a': 1}`) | the server's JSON text (`{"a": 1}`) | the Rust text is valid JSON; the Python one is a repr |
| Q-011 | PostgreSQL values: arrays | `str(list)` (`[1, 2]`) | the server's text (`{1,2}`) | consumers that parse arrays |
| Q-012 | PostgreSQL values: `interval`, `time`, `timetz` | `str(timedelta)` (`1 day, 2:03:04`), `time.isoformat()` | the server's text (`1 day 02:03:04`, `03:04:05+00` for `timetz`) | not normalised; a `time` with a fraction is `.5` in the text where Python gives `.500000` |
| Q-013 | PostgreSQL values: fractional and special timestamps | `isoformat()`: microseconds always six digits when not zero (`.500000`); `infinity` and `BC` dates are an error or a string | the server's text with `T` for the space, so `.5` stays `.5`; `infinity` stays `infinity` | the digits differ for a fraction with trailing zeros |
| Q-014 | PostgreSQL values: `NaN` and `Infinity` floats | a Python float, which `json.dumps` writes as bare `NaN` and `Infinity` (not valid JSON) | the strings `"NaN"`, `"Infinity"`, `"-Infinity"` | the Rust output is valid JSON |
| Q-015 | PostgreSQL values: `bytea` with `bytea_output = escape` | `<N bytes>` of the decoded value | pinned to `hex` in the session (D-015), so `<N bytes>` is the same; a statement that sets `escape` itself would count the escaped text | only through a statement that changes its own session |

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
| PostgreSQL: every row the server sends is buffered before `max_rows` is applied (`simple_query`, like psycopg's client-side cursor): `max_rows` bounds the output, not memory | `connector::postgres` (comment at `run_statement`) |

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

## Worker A2 entries (T007, T010)

## TLS (T007)

- D-A2-01. Field names and keys are proposals: `tls_mode` (`verify` default, `unverified`), `tls_ca_file`
  (absolute path or `~/...`), `tls_server_name`; in a `.env` file `<PROFILE>_TLS_MODE`,
  `<PROFILE>_TLS_CA_FILE`, `<PROFILE>_TLS_SERVER_NAME`. Keep, or rename (for example `sslmode` style)?
- D-A2-02. `tls_ca_file` must be absolute or start with `~/`; a relative path is refused so the result does
  not depend on the directory the runner starts in. Should a relative path resolve against the profiles
  file's directory instead?
- D-A2-03. `unverified` is refused when the profile is DECLARED production (which includes a profile with no
  class at all) even if the local proof succeeds, because the declaration is the owner's word. A profile
  that names a container but no class needs `CLASS=local` to use `unverified`. Intended?
- D-A2-04. `unverified` plus `tls_ca_file` is refused as contradictory (the file would be ignored).
- D-A2-05. The system root store is `rustls-native-certs`; when it yields no root the error names
  `tls_ca_file`. A CA file holding a private key is refused. The CA file is capped at 1 MiB and 128
  certificates.
- D-A2-06. Unverified connections do a full handshake each time (session resumption off) so the warning is
  logged on every connection.

## Profiles (T010)

- D-A2-10. Container names are checked against the engines' own name rule before they reach `inspect`
  (`dbrun.py` passed any string, so `-v` could be read as an option). A profile whose container name does not
  match is "not proven local". Hardening, no valid name is affected.
- D-A2-11. The published-port detection (FR-020) is new: a proven container whose engine port is published on
  a wildcard or loopback address is reached by the Rust driver on `127.0.0.1:<host port>`; anything else
  (no mapping, a mapping bound to another address, host networking, a podman pod's member) takes the
  container exec path. `dbrun.py` always joined the container's network namespace.
  Question: should a mapping on a specific non-loopback address of this machine count as published?
- D-A2-12. `secrets migrate` (single and `--all`) plans first and changes nothing until the plan is safe:
  it refuses a symbolic link profiles file before any pass entry is written, takes the file's size and
  modification time and its text BEFORE writing entries, builds the rewritten text with the same line
  splitter as the parser (Python `splitlines`, every terminator kept), and verifies it (no old plaintext for
  a target, the reference in its place, every other line equal and in order); any failure refuses the
  whole migration. Only then are the pass entries written and the file replaced through a new file
  created exclusively next to it, after a second size/time comparison against the snapshot.
  `dbrun.py` split with `\n` only on rewrite and wrote a `.tmp` file, then chmod.
- D-A2-13. `profiles --export-env` creates the file exclusively with mode 600 from its creation (Unix; see
  D-A2-23), refuses a value with a line break of any kind Python's `splitlines` knows or a control character
  other than tab, and quotes a value with both whitespace and `"` with single quotes (and
  refuses one with both quote kinds); `dbrun.py` wrote such values unquoted-safe only for the simple case. It
  also exports the three TLS keys.
- D-A2-14. Profile files over 1 MiB, with NUL bytes or not UTF-8 are refused (`dbrun.py` read any size and
  crashed on bad UTF-8). Line splitting follows Python's `splitlines`, `\r\n` counting once.
- D-A2-15. TOML syntax errors report line and column only; `tomllib`'s text and the Rust `toml` crate's both
  can quote the offending line, which can hold a value. Messages for unknown keys and tables are the same.
- D-A2-16. At most 64 database containers are inspected for discovery; a container engine answer over 8 MiB is
  treated as a failure (no local profiles).
- D-A2-17. `DOCKER_HOST` or `CONTAINER_HOST` other than `unix://` makes the engine non-local, as before. On
  Windows Docker Desktop reports an `npipe://` endpoint, so no profile can be proven local there by this rule
  (the same as `dbrun.py`). Question: accept `npipe://` as local on Windows?
- D-A2-18. Command log entries for `secrets migrate` belong to the log module (T009): the library returns the
  report and the log line (`MigrateReport::log_line`) for the caller to log.
- D-A2-19. `oracle_dsn` refuses descriptor characters in `host`, `port`, `service` and `sid` (defence in
  depth; the shared validator of `core::fields` is the contract and replaces it when A1 lands).
- D-A2-20. The command word in messages stays `dbrun` here; T011 and T080 rename it.
- D-A2-21. An explicit `pass:<entry>` is validated with the pass store's entry rule at LOAD: an invalid
  reference (leading `-` or `/`, `..`, whitespace, a line break) refuses loading the profiles, naming the
  profile and never the text. `dbrun.py` validated at read time, so `dbrun profiles` still listed the profile.
  Consequence: one bad reference blocks every command until it is fixed. Question: list it with an error
  marker instead?
- D-A2-22. `get_profile` registers only the password with the process-wide redactor of `ai-gent-core`
  (replacement text `<token>`, values under four characters ignored, substring match). Host and user are
  not registered there, because a substring match mangles ordinary text (user `postgres` turns
  `postgresql` into `<token>ql`; found in review). `dbrun.py` redacted host and user as whole words with
  the labels `<host>` and `<user>`: `Profile::redaction_pairs` offers them for the shared whole-word
  `Redactor` of T004, which T011/T012 must apply to every driver message.
- D-A2-25. Known limits left by review (2026-11-08): `migrate --all` re-checks each value against what was
  loaded only for the single-profile path, so a change between load and the phase-2 read (milliseconds) is
  not detected; the two FIFO tests use a FIFO already in place, so they cannot tell the open-first fix from
  the old metadata-first check (a swap race is not tested); `Authorized::host()` is carried but no driver
  connects to it yet (T011/T014 must connect to `host()`).
- D-A2-23. Mode 600 is a Unix guarantee. On Windows the migrate and export files are created exclusively
  but inherit the directory's ACL: owner-only is NOT enforced there and nothing in the code claims it.
  Question: enforce an owner-only ACL on Windows, or document it for the user?
- D-A2-24. Reads of profile and CA files open first (non-blocking on Unix, so a FIFO swapped in cannot
  hang the open) and then check the opened file is regular and under the cap. The container engine's
  output is read by `ai-gent-core`'s `exec::run` to the end and only then compared with the 8 MiB cap: the
  30 s timeout bounds that read, the cap does not bound memory during it.
- D-A2-25. `Authorized` carries the host it was checked against (`Authorized::host`) and `authorize` repeats
  the `unverified` + `tls_ca_file` refusal, so settings built by hand cannot skip it.

## Owner questions (not decided by the worker)

1. Relative `tls_ca_file`: keep refusing it, or resolve against the profiles file's directory? (D-A2-02)
2. A container profile with no `CLASS` is declared production and cannot use `unverified` even when proven
   local: keep, or let a proof make it eligible? (D-A2-03)
3. A published port bound to a specific non-loopback address of this machine: published, or exec path?
   (D-A2-11)
4. `npipe://` (Docker Desktop on Windows): count as local? (D-A2-17)
5. Podman pod members and host networking show no published port in their own `inspect`, so they take the
   exec path: acceptable, or inspect the pod's infra container? (D-A2-11)
6. A bad `pass:` reference blocks loading all profiles (D-A2-21): keep, or list with an error marker?
7. Windows owner-only files (D-A2-23): enforce an ACL, or document?

## MySQL and MariaDB (worker D, T040-T042)

Ids carry `MY` so they cannot clash with the other groups' rows. `D` rows are intended and pinned by a test; `Q` rows keep the Python behavior or the builder's choice until the owner answers.

| ID | Item | Python (PyMySQL) | Rust | Pinned by |
|---|---|---|---|---|
| D-MY-01 | An item that holds more than one statement, an executable comment, a client command, `LOCAL INFILE`, or a change to how text is read | PyMySQL sets neither `CLIENT_MULTI_STATEMENTS` nor `CLIENT_LOCAL_FILES`: the server answers a syntax error (stage `execute`) | refused by the connector before it connects, stage `execute`, fixed text with no part of the statement. The `mysql` crate sets both capabilities and cannot unset them, so without this a second statement would run as one counted item. The connector reads items with its own lexer that follows the server (`mysql::server_view`): `--` needs a following blank, `#` is a comment, `/*! */` and `/*M! */` are code and are refused, a `;` outside text and comments is refused (one trailing is fine), a backslash outside text (`\G`), `DELIMITER`, unterminated quotes and comments, NUL, `sql_mode`, `character_set_client`, `character_set_connection`, `SET NAMES`, `SET CHARACTER SET`, `SET CHARSET` are refused. The shared scanner follows Python and differs from the server (it reads `SELECT 1--1` as a comment and `#'` as a string), so it must not be the only judge on MySQL; a caller that classifies must classify `server_view`'s text. The session drops `NO_BACKSLASH_ESCAPES` and `ANSI_QUOTES`, sets `SET NAMES utf8mb4`, and in read mode checks `@@transaction_read_only` (`tx_read_only` on MariaDB) after every statement, aborting and rolling back when it is not 1 | fixture `24` (`only: port`: 30 refusal steps, the Unicode-space refusals after `--`, a read-mode allowlist refusal, a write-mode CALL with two result sets; 52 steps in all), `connector::mysql` tests `every_adversarial_item_is_refused_before_connecting_and_leaks_nothing`, `text_comments_and_one_trailing_semicolon_are_one_statement`, live `guards`, `read_only_is_checked_after_every_statement`, `the_session_reads_text_as_the_lexer_assumes` |
| D-MY-02 | TLS on every MySQL and MariaDB connection (FR-015) | no `ssl` argument: unencrypted | verify by default (the CA file or the crate's roots); `unverified` only on a proven-local loopback profile; a server that offers no TLS fails naming the cause | `tests/mysql_engine.rs` (`#[ignore]`, needs Podman), `connector::mysql` tests |
| Q-MY-01 | Row count of a SELECT (Q-007 for MySQL) | rows returned (the result is buffered) | kept: the number of rows the SELECT returned, counted past `max_rows` without storing them | fixture `24` |
| Q-MY-02 | Reading a result past the request timeout | PyMySQL buffers everything with no bound but the socket timeout per read | reading rows stops with an `execute` error at the request timeout, and the socket reads and writes are bounded at `timeout + 5` s each. That is NOT a bound on the whole call: when a result set is dropped, the crate reads the rest of its rows and the remaining result sets itself, so a huge or endless result can still hold the runner. The real bound is the child process deadline of T012 (not built yet) | `run_statement` (live test only for the timeout error) |
| Q-MY-03 | Error text | `OperationalError: (1792, 'Cannot execute ...')` | `MysqlError: ERROR 1792 (25006): Cannot execute ...` (the spike's format, kept by the engines script); I/O, driver and TLS errors are `IoError:`, `DriverError:`, `TlsError:` | fixture `24` (`error_contains` the code) |
| Q-MY-04 | TLS trust (FR-015, FR-016): **DECIDED by the owner (2026-11-14): accept and document** | n/a | the `mysql` crate builds its own rustls configuration: a certificate that chains to its bundled Mozilla roots (`webpki-roots`) is accepted in addition to `tls_ca_file`, so a publicly trusted certificate for the host is accepted when only the profile's CA should be; with no CA file the bundled roots are used, not the operating system's store. FR-016 exempts only Oracle. Done: `core::tls` checks the policy and the CA file (read once, bounded; the client gets a private temporary copy of those bytes, so it never reads the path itself and reads no more than the cap); verify mode with no CA file asks `core::tls` for nothing (the system roots would be ignored). Options for the owner: (a) accept and document the wider trust for MySQL and MariaDB (the name check and signature check still hold; an attacker needs a public-CA certificate for the exact host); (b) a different client, or our own handshake over `mysql_common`'s protocol types with `core::tls::client_config` (the MySQL TLS upgrade is a short SSLRequest exchange, but the client's connection and authentication code would be ours); (c) a patch or fork of the crate with a `root_cert_only` option. T042 stays unticked until the owner chooses | no test can show the extra roots; `the_ca_copy_handed_to_the_client_is_the_validated_bytes`, live `tls_checks` | Owner decision: accept the wider trust for MySQL and MariaDB and document it for users (the signature and name checks still hold; the extra trust is the bundled public roots).
| Q-MY-05 | `tls_server_name` | n/a | refused unless it equals `host` (the crate verifies against the host it connects to) | `a_tls_server_name_that_differs_from_the_host_is_refused` |
| Q-MY-06 | A request with no `conn.host` | `host=None`: PyMySQL connects to `localhost` | `connect` failure `missing conn.host` | `missing_host_and_bad_ports_are_refused_before_connecting` |
| Q-MY-07 | A port that is not 1 to 65535 | `int()` of the text, any number accepted by the OS call | refused before connecting (`invalid port`) | same test |
| Q-MY-08 | A TIME or datetime the text cannot parse, invalid UTF-8 in a text column | PyMySQL returns the text as is, or raises on a bad byte | the text as is; invalid bytes become U+FFFD | `time_matches_python_str_of_timedelta` (unparsable TIME); the UTF-8 case has no test |
| Q-MY-09 | DNS resolution of the host | no bound of its own | no bound of its own (the crate resolves with the system call; the 10 s limit covers the TCP connect only) | none: recorded limit |
| Q-MY-10 | Login as an account the server does not have | `1045 Access denied` (PyMySQL speaks `sha256_password`) | MySQL 8.4 answers an unknown user with an `sha256_password` auth switch the `mysql` crate does not implement: `DriverError: Unknown authentication protocol` (stage `connect`, redacted); a wrong password for a real account is still `1045` | `wrong_password` in `tests/mysql_engine.rs` |
| Q-MY-10 (amended) | Login as an account the server does not have, or an account on the `sha256_password` plugin | `1045 Access denied` for the unknown user; PyMySQL speaks `sha256_password` and logs in with it | MySQL 8.4 answers an unknown user with an `sha256_password` auth switch the `mysql` crate does not implement: `DriverError: Unknown authentication protocol` (stage `connect`, redacted). The same limit means an account that really uses `sha256_password` cannot log in at all (the crate supports `mysql_native_password` and `caching_sha2_password`). A wrong password for a `caching_sha2_password` account is still `1045` | `wrong_password` in `tests/mysql_engine.rs` (unknown user only) |
| Q-MY-11 | Advisory RUSTSEC-2025-0134 (`rustls-pemfile` unmaintained), pulled by the `mysql` crate with `rustls` | n/a | `cargo deny check` with `--features ai-gent-dbrun/mysql` fails on it. No ignore entry was added: it is an owner decision (accept with a dated ignore and a reason, or move to a client that uses `rustls-pki-types`). The default build (no engine feature) passes `cargo deny check` | `cargo deny --features ai-gent-dbrun/mysql check` | Owner decision (2026-11-14): ignore RUSTSEC-2025-0134 in `deny.toml` with a dated reason; review when the MySQL client or its TLS dependency changes.
| Q-MY-12 | License `CDLA-Permissive-2.0` (`webpki-roots`, the CA list the `mysql` crate bundles) | n/a | added to `rust/deny.toml`: a permissive license like MIT and ISC (use and redistribution with a notice, no copyleft condition), so it is the same family already allowed | `cargo deny check licenses` |
| Q-MY-13 | The connection character set and SQL modes | PyMySQL: utf8mb4; the server's `sql_mode` | `SET NAMES utf8mb4`; `NO_BACKSLASH_ESCAPES` and `ANSI_QUOTES` are removed from the session so that the connector's lexer and the server read text the same way; a statement that names `sql_mode` or the connection character set is refused | `the_timeout_variable_follows_the_server_family`, live `the_session_reads_text_as_the_lexer_assumes` |
| Q-MY-14 | Images and containers of the live tests | n/a | pinned by tag (`mysql:8.4`, `mariadb:11.4`); TODO pin by digest. Containers carry `me.lucasvmigotto.ai-gent.project=ai-gent` and are removed by ID | `tests/mysql_engine.rs` |

## Worker A3 entries (T006, T009, T008, T012)

### T006: classifier, scanner, cost rules, system databases (parity list)

Behaviors of `dbrun.py` listed before the tests, one test each (all in `rust/crates/ai-gent-dbrun/tests/golden.rs`, data made by the committed generators `rust/spikes/dbrun/gen_golden.py` and `rust/fixtures/db-core/gen_writes.py`):

| Behavior of `dbrun.py` | Test |
|---|---|
| The 16 driver-free tests of `test_dbrun.py`, every call and refusal text recorded from the original | `core_original_sixteen_tests_match_python` |
| Verdict and reason text of `classify_statement`, 6 engines, 1026 read-heavy strings | `classify_matches_python` |
| `split_statements` (including the backslash refusal), `is_ddl`, `cost_rules` on the same corpus | `split_ddl_cost_match_python` |
| Write side: 203 write and look-alike statements (comment-split keywords, mixed case, hidden literals, locks, sequences, side-effect functions, PRAGMA, EXPLAIN ANALYZE, DO/CALL/COPY/ATTACH, statements after a quote trick, empty) x 6 engines: verdict, reason, split result and refusal text, `is_ddl`, `cost_rules` for production, staging, local and with `--allow-full-count`, `explain_sql` | `core_writes_classify_split_ddl_cost_match_python` |
| `require_user_database` matrix (36 cases: system names in any case, unset and empty database on SQL Server, names with surrounding spaces, other engines) with the refusal text | `core_writes_system_databases_match_python` |
| Masking, redaction labels, `network_args` | `mask_redact_network_match_python` |
| Refusals name the rule and never echo the input | `core_refusals_name_the_rule_and_never_echo_the_input` |

- Found by this corpus and fixed (a parity bug, not a difference): `require_user_database` treated an EMPTY database name as a named one, so a SQL Server profile with `DATABASE=` (which connects to `master`) passed. Python uses `p.get("database") or "master"`; Rust now does the same.
- D-A3-01. Not copied from Python: the refusal text uses `'name'` quoting like Python's `{db!r}` only for names without a quote; a system name never has one, so the text is equal in every case that can be refused.

### T008: backup and restore interface (decision and parity list)

Decision recorded (plan decision 5, FR-018): `Backup` is a trait in `core/backup.rs` (re-exported as `connector::Backup`), with `take` and `restore` and a `supported(engine)` default equal to `BACKUP_SUPPORT`. The one real implementation, `EngineBackup`, runs the same shell scripts as `dbrun.py` through an `Exec` trait (the container exec; the real `ContainerExec` runs `<engine> exec -i <container> sh -c <script> sh <user> <database>`, T014 builds the profile-driven selection on it). SQLite uses the backup API on the file. No Python, no Rust logical dump.

| Behavior of `dbrun.py` | Test (`tests/backup.rs`) |
|---|---|
| PostgreSQL: `pg_dump -Fc -h 127.0.0.1 -U $1 -d $2`, password on the first stdin line, dump to `<id>.pgdump` mode 600 | `postgres_backup_drives_pg_dump_with_the_password_on_stdin_only`, `real_exec::arguments_carry_no_password_and_stdin_reaches_the_tool` |
| MySQL/MariaDB: `mysqldump` or `mariadb-dump` with `--single-transaction --routines --triggers --events --add-drop-database --databases $2`; no database is an error | `mysql_and_mariadb_use_their_dump_tool_and_need_a_database` |
| Restore: `pg_restore --clean --create --if-exists --no-owner ... -d template1`, the `mysql`/`mariadb` client for SQL dumps, the file after the password line | `restore_streams_the_recorded_file_and_refuses_any_other_location` |
| SQLite backup and restore through the backup API, file mode 600 | `sqlite::sqlite_roundtrip_through_the_backup_api` |
| A failing tool: `backup/restore command failed: <last 400 characters of stderr>`, redacted | `a_failed_tool_is_an_error_without_secrets_and_leaves_no_backup_file` |
| System databases refused (`require_user_database`) | `a_system_database_is_refused_for_backup_and_restore_and_nothing_runs` |
| Oracle has no backup; `apply` stays refused there | `engines_without_a_method_are_refused_not_guessed` |
| 1800 s deadline of `container_shell` | `real_exec::a_hang_is_killed_at_the_deadline_and_leaves_no_file` |

Differences (Python did none of these):

- D-A3-02. `take` and `restore` refuse a system database themselves (Python checked it in `cmd_plan` and `cmd_apply` only; `cmd_restore` never did, so a plan file with a system profile could restore over `master`).
- D-A3-03. `restore` recomputes the backup location from the plan id and refuses a recorded `backup_location` that differs. Python read whatever path the (editable) plan file named and fed it to `pg_restore` or `mysql`.
- D-A3-04. User, database and container name are matched against an allowlist (letters, digits, `_ - . $ #`, no leading `-`) before they become arguments; Python passed any string (`postgresql://...` as the database made `pg_dump -d` connect to another host). A password with a line break is refused (it would put a second line on the tool's stdin). Question: a user such as `app@host` is refused by the allowlist; is that acceptable for local container profiles?
- D-A3-05. Every exec has a deadline and a size cap (64 GiB, a bound chosen here; Q-003 style), stdout streams to the file instead of memory, restore streams the file instead of `read_bytes()`, a failed or capped dump is deleted, and the readers get a 2 s grace after the engine client is killed (a grandchild holding the pipes cannot hang the runner).
- D-A3-06. SQL Server: refused until its connector offers the autocommit path (FR-019); Python used a one-off `pymssql` script. `BackupError::Refused` says so; no backup is attempted, so `apply` on SQL Server stops before writing.
- D-A3-07. SQLite restore checks the 16-byte SQLite header of the backup file first and refuses anything else.
- Windows: the kill is by child handle, never by name; the removal of a partial dump after a kill can fail while a leaked grandchild still holds the file open (the file is then left, with the plan not recorded as backed up).

### T009: plan, apply, restore, script, query and the command log (parity list)

Behaviors of `dbrun.py` listed before the tests (`rust/crates/ai-gent-dbrun/tests/run.rs`; golden data from `rust/fixtures/db-core/gen_writes.py`):

| Behavior of `dbrun.py` | Test |
|---|---|
| `test_read_masks_and_logs`, `test_row_cap`, `test_remote_refusals`, `test_remote_read_is_read_only_even_past_the_classifier`, `test_plan_apply_restore`, `test_apply_rolls_back_on_count_mismatch`, `test_tampered_plan_refused`, `test_script_never_executes` (the 8 `SqliteEndToEndTest` cases) | `e2e_1` .. `e2e_8` |
| The plan record keys and values (`profile`, `engine`, `reason`, `statements`, `expect`, `ddl`, `backup`, `sha256`, `evidence`), the text `plan` prints, and the command-log entry (heading, engine line, evidence, reason, plan line, fenced statements, `status: started`, result line), 7 plans over 5 engines | `parity_plan_record_text_and_log_entry_match_the_original` |
| The change script text and its refusals (UPDATE or DELETE without WHERE, backslash split, no statements), 17 scripts | `parity_change_script_text_and_refusals_match_the_original` |
| `print_table` (cut at 120, `NULL`, newline as a symbol), `--format json` separators and `ensure_ascii`, `--format csv` | `parity_query_output_formats_match_the_original` |
| `--expect` parsing (`1,,x,-2` gives 1, skip, skip, -2) and the exit codes 0 to 4 | `parity_expect_parsing_and_exit_codes` |
| Log entry written before the statement runs (checked from inside the connector at the write request) | `e2e_5_...` |
| `apply` needs `--confirmed`, re-proves local, system database refused, probe fails with exit 4, links make it non-local, rollback prints `ROLLED BACK (stage)` and exits 1, MySQL DDL warning, `restore` needs a backup | `e2e_5`, `e2e_6`, `system_databases_and_links_and_missing_proof_are_refused`, `exit_codes_...`, `restore_refuses_...` |
| `--allow-full-count` lifts the COUNT rule and nothing else | `e2e_3_...` |

Differences (each with its test; Python did none of these):

- D-A3-08. A second `apply` of the same plan is refused (an exclusive `<id>.applied` claim file; two concurrent applies get exactly one claim). The claim is given back only when the connector confirmed a rollback (`verify` stage) or the backup failed before any write; any other failure (a killed driver, MySQL's implicit DDL commit, `ok` without `committed`) keeps it and says so, because the database may have changed. The user makes a new plan. Question: should a failed `execute` with no DDL release the claim too?
- D-A3-09. The plan carries a `seal`: SHA-256 over id, profile, engine, `expect`, `ddl`, backup mode and statements. Python's `sha256` (statements only, kept for compatibility) let `expect` be edited to `[null]` and the count check disappear. A plan without a seal, which includes every plan the Python runner recorded, is refused with "make a new plan". The seal is tamper evidence against accidental or agent edits, not a MAC: someone who can write the plans directory can recompute it, exactly as with the Python hash. Question: sign with a key kept outside the plans directory?
- D-A3-10. `apply` also refuses a plan whose engine differs from the profile's, runs the connect probe only after the tamper check (a tampered plan causes no connection), and treats a result as committed only if it says `ok` AND `committed` with one count per statement.
- D-A3-11. The command log: user text cannot forge structure (a line starting `## `, a code fence or, after the first line, `- ` gets a tab; control characters become U+FFFD); the log path may not be a symbolic link (`O_NOFOLLOW`) or a non-file; an entry above 32 MiB is refused; every entry is redacted (password, host, user) including the reason and the statements. A log that cannot be written stops the command before anything runs (Python let the exception propagate, which had the same effect, but nothing tested it). `plan` removes its plan file when its entry cannot be logged.
- D-A3-12. Times are UTC (`...+00:00`, log file date and plan id from UTC); Python used local time. The standard library has no local-zone lookup and the crate forbids `unsafe`. Question: add a time crate for local time?
- D-A3-13. `--expect` entries Python crashes on (`--5`, a number that does not fit 64 bits) are refused with a message; a non-numeric entry still means "skip" as in Python (a typo such as `2x` silently skips that count; Question: refuse it?). Plan SQL and script SQL above 16 MiB are refused.
- D-A3-14. `script` creates its file exclusively (Python checked `exists()` and then wrote, which races and follows a symbolic link) and removes it if its log entry cannot be written. `--format json` writes the `masked` object with sorted keys; Python keeps column order.
- D-A3-15. Not in this task: reading the SQL from `--sql -` or a file (bounded read), the CLI flags, the profile-to-`Target` wiring beyond `Target::from_profile` (T011), `session`, `secrets migrate` log entries (`MigrateReport::log_line` is for the CLI to log), the container-exec route (T014).

### T012: connector isolation (parity list and decisions)

`dbrun.py` ran the driver in a separate process too (`driver_process`: `uv run ... _exec`, request on stdin, last stdout line parsed, a timeout of `req.timeout * statements + 180` s, `driver process failed: <last 5 lines of stderr>` as a connect failure). Parity items and their tests (`rust/crates/ai-gent-dbrun/tests/isolate.rs`, a test executable without libtest because the child is that same executable; names contain `isolate`):

| Behavior of `dbrun.py` | Test |
|---|---|
| Request on stdin, one JSON line out | `isolate_a_round_trip_carries_the_password_on_stdin_only` |
| A child that dies or prints no result line: a connect-stage failure with a bounded, redacted excerpt | `isolate_an_abort_...`, `isolate_garbage_output_...` |
| A timeout kills the driver process; wall limit scales with the statement count | `isolate_a_hang_is_killed_at_the_deadline_and_the_child_is_gone` (limit scaling: `process` unit test) |
| `_exec` reads the request from stdin and prints `execute()`'s result | `isolate_the_exec_reader_...` |

Differences:

- D-A3-16. `serve_exec` (the `_exec` reader) bounds its own read at the 16 MiB request limit plus the newline: it stops reading an endless stdin (the old reader was `json.load(sys.stdin)`, unbounded). Malformed, non-UTF-8 and empty input each get one `request`-stage line that names no value.
- D-A3-17. The child's environment is rebuilt, not inherited: `PATH`, system and temp variables, locale, `SSL_CERT_FILE` and `SSL_CERT_DIR`, and what the caller adds. `PGPASSWORD`, `MYSQL_PWD`, `ORACLE_*`, tokens and the rest are dropped. `dbrun.py` passed its whole environment on (and added `LD_LIBRARY_PATH` for Oracle thick mode, which T070 must add through `with_env`). `HOME` is not inherited either (no `~/.pgpass` or `~/.my.cnf` read by a driver: a profile is the only source of credentials). Question: does any driver need `HOME`?
- D-A3-18. A panic that unwinds is caught inside the child and becomes an `execute`-stage line with a fixed message (the panic text may hold a credential and is never shown); an abort ends the child and the parent reports it. During a `write` request a child that gave no result is an `execute` failure that says the commit is unconfirmed (the process connector said `connect`), and `apply` keeps the plan claimed (D-A3-08).
- D-A3-19. The release profile still reads `panic = "unwind"` in `rust/Cargo.toml` while plan decision 5 says `panic = "abort"` stays; with this change both work (unwind is caught in the child, abort is contained by the process boundary). Left to the owner and T091.
- D-A3-20. Kill is by child handle on every platform (no pattern kill). A grandchild that inherited the pipes can outlive the kill (the process connector's recorded limitation); the readers are bounded so the runner does not wait for it.
- Not in this task: the `ai-gent db _exec` subcommand itself (T011 wires `serve_exec` and the real connectors; `IsolatedConnector::current_exe` gives it `db _exec`).

### Review round on T006-T012 (fixes, intended differences and recorded questions)

- D-A3-21 (intended, owner decision 2026-11-14). A non-numeric `--expect` entry is REFUSED, naming its position and never its text; `skip` (or an empty entry) means no count check; only ASCII digits count (Python read any non-number as skip). An `expect` list whose length differs from the statements is refused at verify (the T005 contract), and old unsealed Python plans stay refused (D-A3-09). Pinned by `parity_expect_parsing_and_exit_codes`. This replaces the open question in D-A3-13.
- D-A3-22. `apply` labels outcomes by what is known: `ROLLED BACK` only for a verify failure the connector confirmed and not on MySQL/MariaDB with DDL in the plan (implicit commit: claim and backup kept, `OUTCOME UNKNOWN`); `NOT APPLIED` for a connect-stage failure (nothing ran: claim and backup given back); `OUTCOME UNKNOWN` for an `ok` result without a commit report, a commit with a different number of counts, or a failure after the write began. A committed result from a child that was killed for not exiting is kept as `COMMITTED` with a warning (`ExecResult.warning`, never on the wire); any other result of a killed child stays a failure. A spawn failure is a plain connect failure and is not relabelled "not confirmed committed".
- D-A3-23. A plan file whose inner `id` differs from its file name is refused (a copied plan could bypass the claim). The restore refuses a missing, non-regular or empty backup, and the container exec opens the backup before starting the tool and fails if it cannot be streamed whole.
- D-A3-24. `apply` writes its start entry after taking the claim (a refused second apply leaves no `started` entry; a log failure gives the claim back). The log also replaces Unicode format and bidi characters (U+200B..U+200F, U+202A..U+202E, U+2066..U+2069, U+FEFF). Of the two symlink guards of the log append, the `symlink_metadata` check is tested (directory, live and dangling link); `O_NOFOLLOW` only closes the swap between check and open and cannot be tested deterministically.
- Q-A3-01 (recorded, not decided). `--format json` (`py_json`) differs from Python's `json.dumps` for `\b` and `\f` (Python writes `\b` and `\f`, Rust `\u0008` and `\u000c`) and for float exponent formatting (`1e-07` against `1e-7`).
- Q-A3-02 (recorded, not decided). `rust/Cargo.toml` release profile says `panic = "unwind"`, plan decision 5 says `panic = "abort"` stays (see D-A3-19).
