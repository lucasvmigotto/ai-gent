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
| D-011 | PostgreSQL: DDL, `EXPLAIN` and `SHOW` row counts (T031, FR-022) | psycopg `cursor.rowcount`: `-1` for DDL, the rows received for `EXPLAIN` and `SHOW` | `0` for every statement whose `CommandComplete` tag carries no number (the client crate keeps only the number); a `SELECT` and a DML keep the server's number, as psycopg | `connector::postgres` tests `ddl_explain_and_show_count_zero_the_contract_value`, `rowcount_is_the_servers_count_whatever_the_statement_text`; `real_server_counts_come_from_the_server` (needs a server) |
| D-012 | PostgreSQL TLS (T032, FR-015..FR-017) | libpq `sslmode=prefer` through psycopg: TLS when offered, plain text otherwise, certificate not verified | always TLS, certificate verified (system roots or `tls_ca_file`, name from `tls_server_name` or the host); a server that answers `N` to the SSL request is a `connect` failure, never a plain session; `unverified` only through `TlsSettings::authorize`; the request's host must be the host the policy was checked for | `tests/connector_postgres.rs` (fake TLS server with a throwaway CA) |
| D-013 | PostgreSQL port | a non-numeric `port` raises in the connect; an empty one is 5432 | a port that is not 1-65535 is a `connect` failure `invalid port` (the spike fell back to 5432 on a parse error, which could connect to the wrong service); an empty or missing one is 5432 | `bad_connection_facts_are_refused_without_echoing_them` |
| D-014 | PostgreSQL silent server, and an abandoned request | none: a server that accepts and then never answers hangs the runner (`connect_timeout` covers the TCP connect only) | the connector waits `connect_timeout` (10 s) plus up to 2 s for the connection to be established and fails at stage `connect` (exit 4 class, no `seconds`) when it is not; after that it abandons the request at `grace (5 s) + timeout x (statements + 3)` with a stage `execute` failure. An abandoned request never commits: the worker checks a shared flag before `BEGIN`, each statement and `COMMIT` and rolls back; when `COMMIT` was already sent the message is `commit outcome unknown` (check the data). The abandoned thread may hold its socket until the process ends, and the server's `idle_in_transaction_session_timeout` (60 s) closes an open transaction | `a_server_that_accepts_and_goes_silent_is_a_connect_failure_at_the_connect_deadline`, `an_abandoned_write_never_commits`, `a_commit_that_does_not_answer_in_time_is_reported_as_unknown` |
| D-015 | PostgreSQL session settings (review of T030) | none: the server's `DateStyle` and `bytea_output` apply | the startup options pin `DateStyle=ISO` and `bytea_output=hex`, so the value normalisation of timestamps and `bytea` cannot be defeated by a server or role setting (a statement may still `SET` them for its own session) | `a_verified_connection_with_the_ca_reaches_the_server_and_sends_the_session_options`, `read_mode_begins_read_only_checks_after_every_statement_and_never_commits` |
| D-016 | PostgreSQL read mode after transaction control (review of T030) | the session default alone: `COMMIT` then `BEGIN READ WRITE` or `SET default_transaction_read_only = off` make the next statement writable | after every statement `SHOW transaction_read_only` must be `on`; otherwise rollback and an `execute` failure before the next statement runs (one extra round trip per statement). The statement that flipped it already ran: it is transaction control or a `SET`, never a write, because the server refused writes until then | `read_mode_stops_when_a_statement_made_the_session_writable`; ignored `real_server_read_mode_refuses_a_write_with_25006_and_a_second_statement` |

## Questions for the owner (the Python behavior is what runs today)

| ID | Item | Python | Rust today | Why it may matter |
|---|---|---|---|---|
| Q-001 | `expect` longer than `statements` | the extra entries are ignored and the write commits | same | a plan whose expected counts outnumber its statements passes verification silently; failing closed would be a `verify` error. Pinned as Python by fixture `19-verify-edges` and `request::verify_counts` |
| Q-002 | An empty connection field | `c.get("service") or c.get("database")` treats `""` as unset | `core::fields` refuses an empty value; the profile loaders (T010) should turn a blank field into "absent" | a profile with `SERVICE=` would otherwise be refused at run time |
| Q-003 | Limits chosen here (a builder proposal, see below) | n/a | timeout 0.001 to 3600 s, `max_rows` at most 1,000,000, request line at most 16 MiB (FR-011 says "capped" and gives no number) | the numbers are mine; the owner may want others |
| Q-004 | A timeout that is out of the range of a JSON number (`1e999`) | Python reads it as `inf` and fails in the connect | the JSON parser rejects the line, so it is a `request`-stage error, not `connect` (FR-011 says `connect`) | same outcome (a refusal with no secret), different stage name; fixing it needs a hand-written number scanner |
| Q-005 | Redaction strictness | see D-003 | a one-character password or user is redacted wherever it occurs, which can make an error unreadable; host and user also match in any ASCII case, so a user `a`, `user`, `select` or `sa`, or a host `db`, turns "A table named a is a problem" into "<user> table named <user> is <user> problem" | safe over readable on a credential path; say if a minimum length is wanted |
| Q-006 | Host names in the field allowlist | any text | ASCII only (an internationalized name is given as punycode); IPv6 without brackets | a profile with a Unicode host is refused, with the field named |
| Q-007 | PostgreSQL: a `SELECT`'s `rowcount` | psycopg: the number of rows the server sent (`SELECT 3` gives `3`) | the same (the server's number); the contract's `-1` for a statement that returns rows comes from SQLite and is not applied, because `CommandComplete` cannot tell `SELECT 3` from `UPDATE ... RETURNING` with 3 rows without reading the statement text | a fixture for PostgreSQL would need to say which of the two the contract wants |
| Q-008 | PostgreSQL: a host that is empty or a unix socket | no host means libpq's default socket; a path starting with `/` is a socket and uses no TLS | an empty host is a `connect` failure `missing conn.host`; a socket path (Unix only) uses no TLS, since it never leaves the machine. Caveat: the socket is not authenticated, so a process that can create a socket in a world-writable directory (such as `/tmp`) can impersonate the server and receive the password if the server asks for a cleartext one; the profile's host is the user's choice and a socket under a directory only the user can write is safe | say if a socket host should also be refused, or limited to directories not writable by others |
| Q-009 | PostgreSQL without TLS | psycopg connects to a server with TLS off (`prefer`) | unreachable, so a plain local `postgres` container fails to connect with `the server must offer TLS`: there is no `disable` mode in the profile model (`verify` or `unverified`), and `unverified` still encrypts. A plain `postgres:17` container has TLS off, so the engines script (T034) must start it with `ssl=on` and a self-signed certificate and set `tls_mode = unverified` on its proven-local profile | decide whether a proven-local loopback profile may have a `disable` mode |
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
